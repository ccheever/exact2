use exact_gpu::wgpu;

/// Number of optional GPU timestamp pairs reserved for a frame.
pub const GPU_PASS_COUNT: u32 = 29;
/// Timestamp slots: inactive passes leave their pair untouched.
pub const GPU_PASS_NAMES: [&str; GPU_PASS_COUNT as usize] = [
    "shadow 0",
    "shadow 1",
    "shadow 2",
    "forward + sky",
    "bloom bright",
    "bloom down 1",
    "bloom down 2",
    "bloom down 3",
    "bloom down 4",
    "bloom down 5",
    "bloom up 0",
    "bloom up 1",
    "bloom up 2",
    "bloom up 3",
    "bloom up 4",
    "tonemap",
    "skin palettes",
    "hook compute",
    "hook opaque (inside forward)",
    "hook background (inside forward)",
    "hook surface + translucent",
    "hook post",
    "opaque depth resolve",
    "final depth resolve",
    "cull",
    "ssao",
    "local shadows",
    "environment prefilter",
    "translucent + particles (inside forward)",
];
/// Timestamp pair of the frustum-culling compute pass.
pub(crate) const CULL: u32 = 24;
pub(crate) const SSAO: u32 = 25;
pub(crate) const LOCAL_SHADOWS: u32 = 26;
pub(crate) const ENVIRONMENT: u32 = 27;
pub(crate) const TRANSLUCENT: u32 = 28;

/// One pair spanning several passes: the first begins it, the last ends it.
pub(crate) fn span(
    set: Option<&wgpu::QuerySet>,
    pass: u32,
    first: bool,
    last: bool,
) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
    set.filter(|_| first || last)
        .map(|query_set| wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: first.then_some(pass * 2),
            end_of_pass_write_index: last.then_some(pass * 2 + 1),
        })
}

pub(crate) fn writes(
    set: Option<&wgpu::QuerySet>,
    pass: u32,
) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
    set.map(|query_set| wgpu::RenderPassTimestampWrites {
        query_set,
        beginning_of_pass_write_index: Some(pass * 2),
        end_of_pass_write_index: Some(pass * 2 + 1),
    })
}

pub(crate) fn encoder_stamp(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    set: Option<&wgpu::QuerySet>,
    pair: u32,
    end: bool,
) -> bool {
    if device
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
    {
        if let Some(set) = set {
            encoder.write_timestamp(set, pair * 2 + u32::from(end));
            return true;
        }
    }
    false
}
pub(crate) fn pass_stamp(
    device: &wgpu::Device,
    pass: &mut wgpu::RenderPass<'_>,
    set: Option<&wgpu::QuerySet>,
    pair: u32,
    end: bool,
) -> bool {
    if device
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES)
    {
        if let Some(set) = set {
            pass.write_timestamp(set, pair * 2 + u32::from(end));
            return true;
        }
    }
    false
}
