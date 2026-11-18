//! Packer-side future registry — webgpu.h "Asynchronous-Operations".
//!
//! wgpu-core 30 exposes no future infrastructure: every async entry
//! point registers a future here and completes it when wgc resolves
//! the operation. Callback delivery follows the `WGPUCallbackMode` chosen by
//! the consumer:
//! - `AllowSpontaneous`: fired as soon as it completes (during the poll that
//!   discovers it, or before the return for synchronous operations);
//! - `AllowProcessEvents`: payload kept, callback delivered at the next
//!   `wgpuInstanceProcessEvents` (or `wgpuInstanceWaitAny`);
//! - `WaitAnyOnly`: callback delivered only by a `wgpuInstanceWaitAny`
//!   covering the future.
//!
//! A future's membership in its instance is the pointer of the `Arc<Context>`
//! shared by all objects of the same instance: no plumbing between
//! objects is needed, and `wgpuInstanceWaitAny` rejects futures of another
//! instance.

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
};

use parking_lot::Mutex;
use wgc::id as id;

use crate::native;
use crate::objects::Context;

/// C callback ready to be fired, with everything it captures (message, handles).
pub(crate) type Payload = Box<dyn FnOnce() + Send>;

/// Origin timeline of an operation. A timed wait cannot
/// mix several queues (webgpu.h "Mixed Sources").
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// CPU operation (async pipelines, popErrorScope, device loss,
    /// requestAdapter/requestDevice): completes without depending on a queue.
    Cpu,
    /// Operation tied to the timeline of a queue (map async, work done).
    Queue(id::QueueId),
}

enum FutureState {
    Pending,
    Ready(Payload),
}

struct FutureEntry {
    /// `Arc::as_ptr(context)` of the owning instance.
    owner: usize,
    mode: native::WGPUCallbackMode,
    source: Source,
    state: FutureState,
}

fn registry() -> &'static Mutex<HashMap<u64, FutureEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, FutureEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_id() -> u64 {
    // 0 remains the "null" sentinel (WGPU_FUTURE_INIT), never assigned.
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

thread_local! {
    /// Ids completed during the internal poll of a `wgpuInstanceWaitAny` on this
    /// thread: the AllowSpontaneous futures are fired there immediately; the
    /// scope lets the wait loop set their `completed` flag.
    static WAIT_SCOPE: RefCell<Option<Vec<u64>>> = const { RefCell::new(None) };
}

fn notify_wait_scope(future_id: u64) {
    WAIT_SCOPE.with(|scope| {
        if let Some(ids) = scope.borrow_mut().as_mut() {
            ids.push(future_id);
        }
    });
}

/// Runs `poll` with a wait scope active on this thread and returns the
/// ids of AllowSpontaneous futures completed (already fired) during this poll.
pub(crate) fn run_in_wait_scope<R>(poll: impl FnOnce() -> R) -> (R, Vec<u64>) {
    WAIT_SCOPE.with(|scope| {
        let outer = scope.borrow_mut().replace(Vec::new());
        let result = poll();
        let completed = scope.borrow_mut().take().unwrap_or_default();
        *scope.borrow_mut() = outer;
        (result, completed)
    })
}

pub(crate) fn owner_of(context: &Arc<Context>) -> usize {
    Arc::as_ptr(context) as usize
}

/// webgpu.h: the mode of a CallbackInfo "Has no default" — 0 (set by the
/// INIT macros) is accepted as `WaitAnyOnly` with a warning; any other
/// value outside the enum is a consumer bug.
pub(crate) fn validate_callback_mode(mode: native::WGPUCallbackMode) {
    match mode {
        native::WGPUCallbackMode_WaitAnyOnly
        | native::WGPUCallbackMode_AllowProcessEvents
        | native::WGPUCallbackMode_AllowSpontaneous => (),
        _ => panic!("invalid callback mode: {mode}"),
    }
}

/// Registers a pending future and returns its id (never 0).
/// Normalizes `.mode` in place: 0 — what every `WGPU_*_CALLBACK_INFO_INIT`
/// macro produces — becomes `WaitAnyOnly`, the header's lowest mode, with a
/// warning. A consumer that forgot the mode loses nothing but spontaneity;
/// before this, the call aborted the process.
pub(crate) fn normalize_callback_mode(mode: &mut native::WGPUCallbackMode) {
    if *mode as u32 == 0 {
        log::warn!(
            "callback mode 0 (set by the INIT macros) is treated as WaitAnyOnly; \
             set an explicit WGPUCallbackMode"
        );
        *mode = native::WGPUCallbackMode_WaitAnyOnly;
    }
}

/// Validate + normalize, value form (modes read through a shared
/// reference, e.g. inside a descriptor).
pub(crate) fn effective_callback_mode(
    mut mode: native::WGPUCallbackMode,
) -> native::WGPUCallbackMode {
    normalize_callback_mode(&mut mode);
    validate_callback_mode(mode);
    mode
}

pub(crate) fn register(
    context: &Arc<Context>,
    mode: native::WGPUCallbackMode,
    source: Source,
) -> u64 {
    let future_id = next_id();
    registry().lock().insert(
        future_id,
        FutureEntry {
            owner: owner_of(context),
            mode,
            source,
            state: FutureState::Pending,
        },
    );
    future_id
}

/// Completes a future. `AllowSpontaneous`: payload fired immediately ("as
/// soon as possible") and future consumed. Other modes: payload kept for
/// `wgpuInstanceProcessEvents` / `wgpuInstanceWaitAny`.
pub(crate) fn complete(future_id: u64, payload: Payload) {
    let to_fire: Option<Payload> = {
        let mut map = registry().lock();
        let is_spontaneous = match map.get(&future_id) {
            // Instance purge: silent cancellation, payload dropped.
            None => return,
            Some(entry) => entry.mode == native::WGPUCallbackMode_AllowSpontaneous,
        };
        if is_spontaneous {
            notify_wait_scope(future_id);
            map.remove(&future_id);
            Some(payload)
        } else {
            if let Some(entry) = map.get_mut(&future_id) {
                entry.state = FutureState::Ready(payload);
            }
            None
        }
    };
    if let Some(payload) = to_fire {
        // Outside the registry lock: the callback may re-enter the API.
        payload();
    }
}

/// ProcessEvents: delivers all completed futures of mode
/// `AllowProcessEvents` belonging to this instance.
pub(crate) fn sweep_process_events(context: &Arc<Context>) {
    let owner = owner_of(context);
    let mut payloads: Vec<Payload> = Vec::new();
    {
        let mut map = registry().lock();
        let ready: Vec<u64> = map
            .iter()
            .filter(|(_, entry)| {
                entry.owner == owner
                    && entry.mode == native::WGPUCallbackMode_AllowProcessEvents
                    && matches!(entry.state, FutureState::Ready(_))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ready {
            if let Some(entry) = map.remove(&id) {
                if let FutureState::Ready(payload) = entry.state {
                    payloads.push(payload);
                }
            }
        }
    }
    payloads.into_iter().for_each(|payload| payload());
}

/// Validates a wait list: non-null ids, no duplicates, known and owned
/// by the instance. An already delivered future is absent from the registry → invalid.
/// Returns the source of each future ("Mixed Sources" rule).
pub(crate) fn wait_set_sources(context: &Arc<Context>, ids: &[u64]) -> Option<Vec<Source>> {
    let owner = owner_of(context);
    let map = registry().lock();
    let mut seen = std::collections::HashSet::new();
    let mut sources = Vec::with_capacity(ids.len());
    for id in ids {
        if *id == 0 || !seen.insert(*id) {
            return None;
        }
        let entry = map.get(id)?;
        if entry.owner != owner {
            return None;
        }
        sources.push(entry.source);
    }
    Some(sources)
}

/// Removes and returns the payloads of the listed futures that are complete
/// (pending or already consumed → ignored).
pub(crate) fn take_ready_payloads(ids: &[u64]) -> Vec<(u64, Payload)> {
    let mut out = Vec::new();
    let mut map = registry().lock();
    for id in ids {
        let is_ready = map
            .get(id)
            .is_some_and(|entry| matches!(entry.state, FutureState::Ready(_)));
        if is_ready {
            if let Some(entry) = map.remove(id) {
                if let FutureState::Ready(payload) = entry.state {
                    out.push((*id, payload));
                }
            }
        }
    }
    out
}

/// Instance destruction: silently cancels the undelivered callbacks
/// (webgpu.h: callbacks may be cancelled at destruction). The
/// `OwnedHandle` guards of the dropped payloads release their handles.
pub(crate) fn purge(context: &Arc<Context>) {
    let owner = owner_of(context);
    let mut map = registry().lock();
    let owned: Vec<u64> = map
        .iter()
        .filter(|(_, entry)| entry.owner == owner)
        .map(|(id, _)| *id)
        .collect();
    for id in owned {
        map.remove(&id);
    }
}

/// Guards a C handle passed with ownership to the callback; frees the Arc if the
/// callback is never fired (cancellation at instance destruction).
pub(crate) struct OwnedHandle<T> {
    handle: Option<*mut T>,
}
unsafe impl<T> Send for OwnedHandle<T> {}

impl<T> OwnedHandle<T> {
    pub(crate) fn new(handle: *mut T) -> Self {
        OwnedHandle { handle: Some(handle) }
    }

    /// Consumes the guard: the handle now belongs to the callback.
    pub(crate) fn take(&mut self) -> *mut T {
        self.handle.take().unwrap_or(std::ptr::null_mut())
    }
}

impl<T> Drop for OwnedHandle<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Cancellation without delivery: the handle was never transferred
            // to the callback, the Arc returns to the ownership of the packer.
            drop(unsafe { Arc::from_raw(handle) });
        }
    }
}
