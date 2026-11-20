#[allow(clippy::missing_safety_doc)]

mod instance;
mod adapter;
mod device;
mod buffer;
mod texture;
mod sampler;
mod shader;
mod bind_group;
mod pipeline;
mod query;
mod command;
mod render;
mod compute;
mod queue;
mod surface;
mod labels;
mod ray_tracing;

pub use instance::*;
pub use adapter::*;
pub use device::*;
pub use buffer::*;
pub use texture::*;
pub use sampler::*;
pub use shader::*;
pub use bind_group::*;
pub use pipeline::*;
pub use query::*;
pub use command::*;
pub use render::*;
pub use compute::*;
pub use queue::*;
pub use surface::*;
pub use labels::*;
pub use ray_tracing::*;

use std::mem;
use crate::{conv, native};

fn return_features(native: &mut native::WGPUSupportedFeatures, features: wgt::Features) {
    let temp = conv::features_to_native(features);
    let mut temp = temp.into_boxed_slice();

    native.featureCount = temp.len();
    native.features = temp.as_mut_ptr();

    mem::forget(temp);
}
