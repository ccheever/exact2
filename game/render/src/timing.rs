use exact_gpu::wgpu;

/// Number of optional GPU timestamp pairs reserved for a frame.
pub const GPU_PASS_COUNT: u32 = 17;
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
];

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
