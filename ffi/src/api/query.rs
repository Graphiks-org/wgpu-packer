use std::sync::Arc;

use crate::native;

// QuerySet methods

#[no_mangle]
pub unsafe extern "C" fn wgpuQuerySetDestroy(query_set: native::WGPUQuerySet) {
    let (query_set_id, context) = {
        let query_set = query_set.as_ref().expect("query set");
        (query_set.id, &query_set.context)
    };

    // Destroy, not drop: the handle stays alive until the consumer releases
    // it, exactly like wgpuTextureDestroy (wgpu-core keeps the two apart).
    context.query_set_destroy(query_set_id);
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQuerySetGetCount(query_set: native::WGPUQuerySet) -> u32 {
    let query_set = query_set.as_ref().expect("invalid query set");
    query_set.data.query_count
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQuerySetGetType(
    query_set: native::WGPUQuerySet,
) -> native::WGPUQueryType {
    let query_set = query_set.as_ref().expect("invalid query set");
    query_set.data.query_type
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQuerySetAddRef(query_set: native::WGPUQuerySet) {
    assert!(!query_set.is_null(), "invalid query set");
    Arc::increment_strong_count(query_set);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuQuerySetRelease(query_set: native::WGPUQuerySet) {
    assert!(!query_set.is_null(), "invalid query set");
    Arc::decrement_strong_count(query_set);
}
