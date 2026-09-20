use clod_view::{Renderer, wgpu};
/// Explicit diagnostic readback after submission; never feeds rendering.
pub fn selections(
    renderer: &Renderer,
    pages: usize,
    max_triangles: u32,
    lists: bool,
) -> Result<[clod_view::select::Selection; 2], String> {
    let buffers = renderer.selection_readback(lists)?;
    for b in &buffers {
        b.slice(..).map_async(wgpu::MapMode::Read, |r| {
            if let Err(e) = r {
                eprintln!("selection map: {e}");
            }
        });
    }
    renderer
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    let mut result = std::array::from_fn(|_| clod_view::select::Selection::default());
    for (buffer, selection) in buffers.iter().zip(&mut result) {
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let words: &[u32] = bytemuck::cast_slice(&mapped);
        let stats = &words[pages * 8..pages * 8 + 4];
        selection.clusters = stats[0] as u64;
        selection.triangles = stats[1] as u64;
        selection.padded_triangles =
            selection.clusters * max_triangles as u64 - selection.triangles;
        selection.candidates = stats[2] as u64;
        selection.overflow = stats[3] as u64;
        if lists {
            let pairs: &[[u32; 2]] = bytemuck::cast_slice(&words[pages * 8 + 4..]);
            selection.pages = (0..pages)
                .map(|page| {
                    let offset = words[page * 8 + 4] as usize;
                    let count = words[page * 8 + 1] as usize;
                    pairs[offset..offset + count].to_vec()
                })
                .collect();
        }
        drop(mapped);
        buffer.unmap();
    }
    Ok(result)
}

pub fn shadow(renderer: &Renderer) -> Result<Vec<u8>, String> {
    let buffer = renderer.shadow_readback();
    buffer.slice(..).map_async(wgpu::MapMode::Read, |r| {
        if let Err(e) = r {
            eprintln!("shadow map: {e}");
        }
    });
    renderer
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let bytes = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(bytes)
}
