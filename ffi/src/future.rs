//! Registre de futures côté packer — webgpu.h « Asynchronous-Operations ».
//!
//! wgpu-core 29 n'expose aucune infrastructure de futures : chaque point
//! d'entrée async enregistre une future ici et la complète quand wgc résout
//! l'opération. La livraison du callback suit le `WGPUCallbackMode` choisi par
//! le consommateur :
//! - `AllowSpontaneous` : tirée dès la complétion (pendant le poll qui la
//!   découvre, ou avant le retour pour les opérations synchrones) ;
//! - `AllowProcessEvents` : payload gardé, callback livré au prochain
//!   `wgpuInstanceProcessEvents` (ou `wgpuInstanceWaitAny`) ;
//! - `WaitAnyOnly` : callback livré uniquement par un `wgpuInstanceWaitAny`
//!   couvrant la future.
//!
//! L'appartenance d'une future à son instance est le pointeur de l'`Arc<Context>`
//! partagé par tous les objets d'une même instance : aucune plomberie entre
//! objets n'est nécessaire, et `wgpuInstanceWaitAny` rejette les futures d'une
//! autre instance.

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

/// Callback C prêt à être tiré, avec tout ce qu'il capture (message, handles).
pub(crate) type Payload = Box<dyn FnOnce() + Send>;

/// Timeline d'origine d'une opération. Une attente temporisée ne peut pas
/// mélanger plusieurs queues (webgpu.h « Mixed Sources »).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// Opération CPU (pipelines async, popErrorScope, perte de device,
    /// requestAdapter/requestDevice) : complète sans dépendre d'une queue.
    Cpu,
    /// Opération liée à la timeline d'une queue (map async, work done).
    Queue(id::QueueId),
}

enum FutureState {
    Pending,
    Ready(Payload),
}

struct FutureEntry {
    /// `Arc::as_ptr(context)` de l'instance propriétaire.
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
    // 0 reste la sentinelle « null » (WGPU_FUTURE_INIT), jamais attribuée.
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

thread_local! {
    /// Ids complétés pendant le poll interne d'un `wgpuInstanceWaitAny` sur ce
    /// thread : les futures AllowSpontaneous y sont tirées immédiatement ; le
    /// scope permet à la boucle d'attente de poser leur flag `completed`.
    static WAIT_SCOPE: RefCell<Option<Vec<u64>>> = const { RefCell::new(None) };
}

fn notify_wait_scope(future_id: u64) {
    WAIT_SCOPE.with(|scope| {
        if let Some(ids) = scope.borrow_mut().as_mut() {
            ids.push(future_id);
        }
    });
}

/// Exécute `poll` avec un scope d'attente actif sur ce thread et retourne les
/// ids de futures AllowSpontaneous complétées (déjà tirées) pendant ce poll.
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

/// webgpu.h : le mode d'un CallbackInfo « Has no default » — 0 (posé par le
/// macro INIT) ou toute valeur hors 1/2/3 est un bug consommateur.
pub(crate) fn validate_callback_mode(mode: native::WGPUCallbackMode) {
    match mode {
        native::WGPUCallbackMode_WaitAnyOnly
        | native::WGPUCallbackMode_AllowProcessEvents
        | native::WGPUCallbackMode_AllowSpontaneous => (),
        _ => panic!("invalid callback mode: {mode}"),
    }
}

/// Enregistre une future en attente et retourne son id (jamais 0).
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

/// Complète une future. `AllowSpontaneous` : payload tiré immédiatement (« as
/// soon as possible ») et future consommée. Autres modes : payload gardé pour
/// `wgpuInstanceProcessEvents` / `wgpuInstanceWaitAny`.
pub(crate) fn complete(future_id: u64, payload: Payload) {
    let to_fire: Option<Payload> = {
        let mut map = registry().lock();
        let is_spontaneous = match map.get(&future_id) {
            // Purge d'instance : annulation silencieuse, payload droppé.
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
        // Hors verrou du registre : le callback peut ré-entrer dans l'API.
        payload();
    }
}

/// ProcessEvents : livre toutes les futures complètes de mode
/// `AllowProcessEvents` appartenant à cette instance.
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

/// Valide une liste d'attente : ids non nuls, sans doublon, connus et possédés
/// par l'instance. Une future déjà livrée est absente du registre → invalide.
/// Retourne la source de chaque future (règle « Mixed Sources »).
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

/// Retire et retourne les payloads des futures listées qui sont complètes
/// (pending ou déjà consommées → ignorées).
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

/// Destruction de l'instance : annule silencieusement les callbacks non livrés
/// (webgpu.h : les callbacks peuvent être annulés à la destruction). Les
/// gardes `OwnedHandle` des payloads droppés libèrent leurs handles.
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

/// Garde un handle C passé avec ownership au callback ; libère l'Arc si le
/// callback n'est jamais tiré (annulation à la destruction de l'instance).
pub(crate) struct OwnedHandle<T> {
    handle: Option<*mut T>,
}
unsafe impl<T> Send for OwnedHandle<T> {}

impl<T> OwnedHandle<T> {
    pub(crate) fn new(handle: *mut T) -> Self {
        OwnedHandle { handle: Some(handle) }
    }

    /// Consomme la garde : le handle appartient désormais au callback.
    pub(crate) fn take(&mut self) -> *mut T {
        self.handle.take().unwrap_or(std::ptr::null_mut())
    }
}

impl<T> Drop for OwnedHandle<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Annulation sans livraison : le handle n'a jamais été transféré
            // au callback, l'Arc redevient la propriété du packer.
            drop(unsafe { Arc::from_raw(handle) });
        }
    }
}
