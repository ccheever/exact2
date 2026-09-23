//! Shared device grants; optional diagnostics allocate only on request.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d6-inspection-and-budgets
use crate::{wgpu, Gpu};

/// Create the device from the first adapter that can present, requesting
/// supported capacities independently. Awaited by native and web loaders.
pub async fn load_gpu(
    instance: wgpu::Instance,
    compatible: Option<&wgpu::Surface<'_>>,
) -> Result<Gpu, String> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: compatible,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no adapter: {e}"))?;
    let available = adapter.limits();
    let required_limits = requested_limits(available);
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("exact"),
            required_limits,
            // Optional diagnostics: no queries or buffers until a surface arms them.
            required_features: adapter.features()
                & (wgpu::Features::TIMESTAMP_QUERY
                    | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
                    | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no device: {e}"))?;
    Ok(Gpu {
        instance,
        adapter,
        device,
        queue,
    })
}

/// Request each optional capacity independently; a low inter-stage limit must
/// not discard storage capacity offered by the same adapter.
pub fn requested_limits(available: wgpu::Limits) -> wgpu::Limits {
    let mut limits = wgpu::Limits::downlevel_defaults().using_resolution(available.clone());
    limits.max_storage_buffers_per_shader_stage = available.max_storage_buffers_per_shader_stage;
    limits.max_inter_stage_shader_variables = available.max_inter_stage_shader_variables.min(16);
    limits
}
