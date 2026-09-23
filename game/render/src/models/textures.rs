//! Baked texture upload: RGBA8 or a GPU block format, exactly as delivered.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#compressed-textures-2026-09-23
use crate::RenderError;
use exact_game::asset::{Filter, TextureData, TextureFamily, TextureFormat, Wrap};
use exact_gpu::wgpu;
use std::collections::BTreeMap;

pub(crate) struct Texture {
    pub(crate) bytes: u64,
    pub(crate) format: TextureFormat,
    pub(crate) active: bool,
    pub digest: u64,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: [u32; 2],
    pub(crate) sprite_bind: Option<wgpu::BindGroup>,
}

/// The device format a record uploads as; sRGB records decode on sampling.
pub(crate) fn device_format(data: &TextureData) -> wgpu::TextureFormat {
    use wgpu::{AstcBlock, AstcChannel, TextureFormat as F};
    match (data.format, data.srgb) {
        (TextureFormat::Rgba8, false) => F::Rgba8Unorm,
        (TextureFormat::Rgba8, true) => F::Rgba8UnormSrgb,
        (TextureFormat::Bc4, _) => F::Bc4RUnorm,
        (TextureFormat::Bc5, _) => F::Bc5RgUnorm,
        (TextureFormat::Bc7, false) => F::Bc7RgbaUnorm,
        (TextureFormat::Bc7, true) => F::Bc7RgbaUnormSrgb,
        (TextureFormat::Astc4x4, srgb) => F::Astc {
            block: AstcBlock::B4x4,
            channel: if srgb {
                AstcChannel::UnormSrgb
            } else {
                AstcChannel::Unorm
            },
        },
    }
}

pub(super) fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &TextureData,
    samplers: &mut BTreeMap<([Wrap; 2], [Filter; 3]), wgpu::Sampler>,
) -> Result<Texture, RenderError> {
    let format = device_format(data);
    // Refuse by name before wgpu would: a family the device did not enable,
    // or an edge beyond the granted limit, is a delivery error, not a crash.
    let (needs, feature) = match data.format.family() {
        TextureFamily::Rgba8 => (wgpu::Features::empty(), ""),
        TextureFamily::Bc => (
            wgpu::Features::TEXTURE_COMPRESSION_BC,
            "texture-compression-bc",
        ),
        TextureFamily::Astc => (
            wgpu::Features::TEXTURE_COMPRESSION_ASTC,
            "texture-compression-astc",
        ),
    };
    if !device.features().contains(needs) {
        return Err(RenderError::scene(format!(
            "{} texture needs device feature {feature}",
            data.format.name()
        )));
    }
    let limit = device.limits().max_texture_dimension_2d;
    if data.width.max(data.height) > limit {
        return Err(RenderError::scene(format!(
            "{}x{} texture exceeds the device's {limit}",
            data.width, data.height
        )));
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("game baked texture"),
        size: wgpu::Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: data.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let (edge, block_bytes) = data.format.block();
    for (level, mip) in data.mips.iter().enumerate() {
        let (w, h) = ((data.width >> level).max(1), (data.height >> level).max(1));
        // A level smaller than one block still copies whole blocks: its
        // physical size rounds up (WebGPU's copy rule for block formats).
        let (blocks_x, blocks_y) = (w.div_ceil(edge), h.div_ceil(edge));
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(blocks_x * block_bytes as u32),
                rows_per_image: Some(blocks_y),
            },
            wgpu::Extent3d {
                width: blocks_x * edge,
                height: blocks_y * edge,
                depth_or_array_layers: 1,
            },
        );
    }

    let sampler = samplers.entry((data.wrap, data.filter)).or_insert_with(|| {
        let wrap = |w| match w {
            Wrap::Repeat => wgpu::AddressMode::Repeat,
            Wrap::Clamp => wgpu::AddressMode::ClampToEdge,
            Wrap::Mirror => wgpu::AddressMode::MirrorRepeat,
        };
        let filters = data.filter.map(|f| {
            if f == Filter::Nearest {
                wgpu::FilterMode::Nearest
            } else {
                wgpu::FilterMode::Linear
            }
        });
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("game shared sampler"),
            address_mode_u: wrap(data.wrap[0]),
            address_mode_v: wrap(data.wrap[1]),
            mag_filter: filters[0],
            min_filter: filters[1],
            mipmap_filter: if data.filter[2] == Filter::Nearest {
                wgpu::MipmapFilterMode::Nearest
            } else {
                wgpu::MipmapFilterMode::Linear
            },
            anisotropy_clamp: if data.filter.iter().all(|f| *f == Filter::Linear) {
                4
            } else {
                1
            },
            ..Default::default()
        })
    });
    Ok(Texture {
        active: true,
        size: [data.width, data.height],
        // Device memory is the delivered level bytes, blocks included.
        bytes: data.mips.iter().map(|mip| mip.len() as u64).sum(),
        format: data.format,
        digest: 0,
        view: texture.create_view(&Default::default()),
        sampler: sampler.clone(),
        sprite_bind: None,
    })
}

/// Active delivered textures (the 1×1 defaults excluded) for `state.world.gpu`:
/// device bytes and a count per format, e.g. `{"bytes":1398128,"Bc7":1}`.
pub(crate) fn summary(textures: &BTreeMap<String, Texture>) -> String {
    let mut bytes = 0;
    let mut counts = [0u32; TextureFormat::ALL.len()];
    for texture in textures
        .iter()
        .filter(|(name, t)| t.active && !name.starts_with('\0'))
        .map(|(_, t)| t)
    {
        bytes += texture.bytes;
        counts[texture.format as usize] += 1;
    }
    let mut out = format!("{{\"bytes\":{bytes}");
    for (format, n) in TextureFormat::ALL.iter().zip(counts) {
        if n > 0 {
            out.push_str(&format!(",\"{}\":{n}", format.name()));
        }
    }
    out + "}"
}
