//! Shared device grants; optional diagnostics allocate only on request.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d6-inspection-and-budgets
use crate::{wgpu, Gpu};
use std::fmt;

/// The device-creation stage that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceFailureKind {
    /// No adapter can satisfy the request.
    NoAdapter,
    /// An adapter exists but cannot create the requested device.
    NoDevice,
}

impl DeviceFailureKind {
    /// Stable host-facing recovery code.
    pub fn code(self) -> &'static str {
        match self {
            Self::NoAdapter => "no-adapter",
            Self::NoDevice => "no-device",
        }
    }
}

/// A typed adapter or device creation failure.
#[derive(Debug)]
pub struct DeviceFailure {
    kind: DeviceFailureKind,
    message: String,
}

impl DeviceFailure {
    pub(crate) fn new(kind: DeviceFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Which device-creation stage failed.
    pub fn kind(&self) -> DeviceFailureKind {
        self.kind
    }
}

impl fmt::Display for DeviceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            self.kind.code().replace('-', " "),
            self.message
        )
    }
}

impl std::error::Error for DeviceFailure {}

/// Create the device from the first adapter that can present, requesting
/// supported capacities independently. Awaited by native and web loaders.
pub async fn load_gpu(
    instance: wgpu::Instance,
    compatible: Option<&wgpu::Surface<'_>>,
) -> Result<Gpu, DeviceFailure> {
    // Exact's Android shaders keep uniforms in f32. Opt this instance into
    // native f16 arithmetic on Adreno; the HAL rejects unsupported f16 uniforms
    // before Vulkan sees them (vendor/wgpu-hal/EXACT-PATCHES.md, 11).
    #[cfg(target_os = "android")]
    if let Some(vulkan) = unsafe { instance.as_hal::<wgpu::hal::api::Vulkan>() } {
        vulkan.enable_f16_arithmetic();
    }
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: compatible,
            ..Default::default()
        })
        .await
        .map_err(|e| DeviceFailure::new(DeviceFailureKind::NoAdapter, e.to_string()))?;
    let available = adapter.limits();
    let required_limits = requested_limits(available);
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("exact"),
            required_limits,
            required_features: adapter.features() & requested_features(),
            memory_hints: memory_hints(),
            ..Default::default()
        })
        .await
        .map_err(|e| DeviceFailure::new(DeviceFailureKind::NoDevice, e.to_string()))?;
    Ok(Gpu {
        instance,
        adapter,
        device,
        queue,
    })
}

/// How the device suballocates. On Android, small blocks: the default
/// (`Performance`) reserves a 128 MiB device block and a 64 MiB host block
/// for the device's own first buffers, which the phone's kernel driver
/// allocates and clears before the device is usable (~55 ms of a cold start
/// on an Adreno phone) and which then count against the app's graphics
/// memory. `MemoryUsage` starts at 8 MiB and 4 MiB blocks.
fn memory_hints() -> wgpu::MemoryHints {
    if cfg!(target_os = "android") {
        wgpu::MemoryHints::MemoryUsage
    } else {
        wgpu::MemoryHints::Performance
    }
}

/// Optional features, each granted only where the adapter has it: timestamp
/// diagnostics (no queries or buffers until a surface arms them), the two
/// block-compressed texture families a surface may choose its payloads from,
/// forced early depth tests (a game's forward shaders, where nothing discards),
/// half-precision shader arithmetic (a game's colour math, which a mobile GPU
/// runs at twice the rate in half the registers) and a first instance in
/// indirect draws (a game's culled draws then share one scene binding).
/// @ref llp/1046.003-game-engine-as-built.explainer.md#compressed-textures-2026-09-23
pub fn requested_features() -> wgpu::Features {
    wgpu::Features::TIMESTAMP_QUERY
        | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
        | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES
        | wgpu::Features::TEXTURE_COMPRESSION_BC
        | wgpu::Features::TEXTURE_COMPRESSION_ASTC
        | wgpu::Features::SHADER_EARLY_DEPTH_TEST
        | wgpu::Features::PIPELINE_CACHE
        | wgpu::Features::SHADER_F16
        | wgpu::Features::INDIRECT_FIRST_INSTANCE
}

/// Request each optional capacity independently; a low inter-stage limit must
/// not discard storage capacity offered by the same adapter.
pub fn requested_limits(available: wgpu::Limits) -> wgpu::Limits {
    let mut limits = wgpu::Limits::downlevel_defaults().using_resolution(available.clone());
    limits.max_storage_buffers_per_shader_stage = available.max_storage_buffers_per_shader_stage;
    limits.max_inter_stage_shader_variables = available.max_inter_stage_shader_variables.min(16);
    limits
}

/// wgpu's defaults without its validation of indirect draw arguments: a game
/// renderer writes them with its own cull, valid by construction, and the
/// validation adds a compute pass and copies before each render pass that
/// draws indirectly (on a Mac, 15% of replaying Castle Town's frame to Metal).
/// D3D12 keeps it: there, without it, `vertex_index` omits the base vertex,
/// which the model shaders read.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn instance_flags() -> wgpu::InstanceFlags {
    let flags = wgpu::InstanceFlags::default();
    if cfg!(windows) {
        flags
    } else {
        flags - wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL
    }
}
