//! Windows full-window canvas composition. Only pixels and an HWND cross the
//! module ABI; the device, game target, and swapchain stay in this artifact.
use crate::*;

pub(crate) struct WindowFrame {
    hwnd: usize,
    target: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    scene: wgpu::Texture,
    below: wgpu::Texture,
    above: wgpu::Texture,
    background: Background,
    overlay: Overlay,
    pending_ui: bool,
    group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

const SHADER: &str = r#"
@group(0) @binding(0) var below: texture_2d<f32>;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var above: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
    return vec4f(p[i], 0., 1.);
}
@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let xy = vec2i(p.xy);
    let b = textureLoad(below, xy, 0);
    let s = textureLoad(scene, xy, 0);
    let a = textureLoad(above, xy, 0);
    return a + (s + b * (1. - s.a)) * (1. - a.a);
}
"#;

impl WindowFrame {
    // SAFETY: the host keeps its HWND alive until the module is detached.
    unsafe fn new(
        gpu: &Gpu,
        hwnd: usize,
        hinstance: usize,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        use wgpu::rwh::{
            RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
        };
        let handle = std::num::NonZeroIsize::new(hwnd as isize).ok_or("null window")?;
        let mut raw = Win32WindowHandle::new(handle);
        raw.hinstance = std::num::NonZeroIsize::new(hinstance as isize);
        let target = unsafe {
            gpu.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(
                        RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                    ),
                    raw_window_handle: RawWindowHandle::Win32(raw),
                })
        }
        .map_err(|e| e.to_string())?;
        let mut config = target
            .get_default_config(&gpu.adapter, width, height)
            .ok_or("adapter cannot present to this window")?;
        config.format = target
            .get_capabilities(&gpu.adapter)
            .formats
            .into_iter()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .ok_or("window has no unorm SDR format")?;
        config.present_mode = if std::env::var_os("EXACT_AGENT_WINDOW_FRAMES").is_some()
            && std::env::var_os("EXACT_AGENT_WINDOW_UNPACED").is_some()
        {
            wgpu::PresentMode::AutoNoVsync
        } else {
            wgpu::PresentMode::AutoVsync
        };
        config.desired_maximum_frame_latency = 2;
        target.configure(&gpu.device, &config);
        let texture = |name, usage| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(name),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | usage,
                view_formats: &[],
            })
        };
        let scene = texture("window game", wgpu::TextureUsages::RENDER_ATTACHMENT);
        let below = texture(
            "window below",
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let above = texture("window above", wgpu::TextureUsages::COPY_DST);
        let entries: Vec<_> = (0..3)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("window composition"),
                entries: &entries,
            });
        let views = [&below, &scene, &above].map(|t| t.create_view(&Default::default()));
        let entries: Vec<_> = views
            .iter()
            .enumerate()
            .map(|(i, v)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(v),
            })
            .collect();
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("window composition"),
            layout: &layout,
            entries: &entries,
        });
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("window composition"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("window composition"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    targets: &[Some(config.format.into())],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        Ok(Self {
            hwnd,
            target,
            config,
            scene,
            below,
            above,
            background: Background::default(),
            overlay: Overlay::default(),
            pending_ui: true,
            group,
            pipeline,
        })
    }
}

#[derive(Default)]
struct Background {
    pixels: Vec<u8>,
    solid: Option<[u8; 4]>,
}

impl Background {
    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        data: &[u8],
        changed: bool,
    ) {
        if let Ok(color) = <[u8; 4]>::try_from(data) {
            if self.solid == Some(color) {
                return;
            }
            let [r, g, b, a] = color.map(|v| v as f64 / 255.);
            let view = texture.create_view(&Default::default());
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("window solid background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.solid = Some(color);
            // A later painted background must upload every pixel, including
            // bytes equal to the painted picture from before this clear.
            self.pixels.clear();
        } else if changed || self.pixels.is_empty() {
            self.solid = None;
            upload(queue, texture, &mut self.pixels, data);
        }
    }
}

// UI is usually unchanged, or changes in a few HUD rows. Upload only those
// rows; do not stage two screen-sized textures on every game frame.
fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, previous: &mut Vec<u8>, data: &[u8]) {
    upload_rows(queue, texture, previous, data, 0..texture.height());
}

#[derive(Default)]
struct Overlay {
    pixels: Vec<u8>,
    rows: std::ops::Range<u32>,
}

impl Overlay {
    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        data: &[u8],
        rows: std::ops::Range<u32>,
    ) {
        let affected = if self.rows.is_empty() {
            rows.clone()
        } else if rows.is_empty() {
            self.rows.clone()
        } else {
            self.rows.start.min(rows.start)..self.rows.end.max(rows.end)
        };
        upload_rows(queue, texture, &mut self.pixels, data, affected);
        self.rows = rows;
    }
}

fn upload_rows(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    previous: &mut Vec<u8>,
    data: &[u8],
    rows: std::ops::Range<u32>,
) {
    let stride = texture.width() as usize * 4;
    let first = previous.len() != data.len();
    if first {
        previous.resize(data.len(), 0);
    }
    let mut row = if first { 0 } else { rows.start as usize };
    let height = if first {
        texture.height() as usize
    } else {
        rows.end as usize
    };
    while row < height {
        if !first
            && previous[row * stride..(row + 1) * stride] == data[row * stride..(row + 1) * stride]
        {
            row += 1;
            continue;
        }
        let start = row;
        row += 1;
        while row < height
            && (first
                || previous[row * stride..(row + 1) * stride]
                    != data[row * stride..(row + 1) * stride])
        {
            row += 1;
        }
        // A changing counter can touch a few pixels in very wide rows.
        // Keep the source stride, but stage only the changed columns.
        let (mut left, mut right) = (texture.width() as usize, 0usize);
        for y in start..row {
            let old = &previous[y * stride..(y + 1) * stride];
            let new = &data[y * stride..(y + 1) * stride];
            if first {
                left = 0;
                right = texture.width() as usize;
                break;
            }
            for (x, (a, b)) in old.chunks_exact(4).zip(new.chunks_exact(4)).enumerate() {
                if a != b {
                    left = left.min(x);
                    right = right.max(x + 1);
                }
            }
        }
        let offset = start * stride + left * 4;
        let end = (row - 1) * stride + right * 4;
        previous[start * stride..row * stride].copy_from_slice(&data[start * stride..row * stride]);
        let mut dest = texture.as_image_copy();
        dest.origin.x = left as u32;
        dest.origin.y = start as u32;
        queue.write_texture(
            dest,
            &data[offset..end],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: (right - left) as u32,
                height: (row - start) as u32,
                depth_or_array_layers: 1,
            },
        );
    }
}

impl Module {
    /// Present a full-window canvas between the host's premultiplied RGBA layers.
    /// `below` may be four bytes naming a uniform color instead of a full layer.
    ///
    /// # Safety
    /// `hwnd` is a live HWND until `detach_window` or destruction of this module.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn present_window(
        &mut self,
        id: u32,
        hwnd: usize,
        hinstance: usize,
        frame: &Frame,
        below: &[u8],
        above: &[u8],
        changed: bool,
        above_rows: std::ops::Range<u32>,
    ) -> Result<(), String> {
        self.check_device();
        if let Some(why) = shaders::missing(self.registry.shaders) {
            return Err(why);
        }
        self.flush();
        let gpu = self.gpu.as_ref().ok_or("no device")?;
        let mut inst = if id == 0 {
            None
        } else {
            let inst = self.instances.get_mut(&id).ok_or("no such canvas")?;
            if !inst.bound {
                return Err("canvas is not bound".into());
            }
            Some(inst)
        };
        let (width, height) = frame.pixels();
        let len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("window layer extent overflow")?;
        if (below.len() != len && below.len() != 4) || above.len() != len {
            return Err("window layer extent mismatch".into());
        }
        if above_rows.start > above_rows.end || above_rows.end > height {
            return Err("window overlay row extent mismatch".into());
        }
        if self
            .window
            .as_ref()
            .is_none_or(|w| w.hwnd != hwnd || w.config.width != width || w.config.height != height)
        {
            // Drop the previous swapchain before creating a replacement for the HWND.
            self.window = None;
            self.window = Some(unsafe { WindowFrame::new(gpu, hwnd, hinstance, width, height) }?);
        }
        let window = self.window.as_mut().unwrap();
        // A timeout or occlusion can happen after the host accepted new UI.
        // Keep its upload pending even when the next host frame is cached.
        window.pending_ui |= changed;
        use wgpu::CurrentSurfaceTexture as Current;
        let drawable = match window.target.get_current_texture() {
            Current::Success(t) | Current::Suboptimal(t) => t,
            Current::Timeout | Current::Occluded => return Ok(()),
            Current::Lost | Current::Outdated => {
                self.window = None;
                return Ok(());
            }
            other => return Err(format!("window surface: {other:?}")),
        };
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        window.background.prepare(
            &gpu.queue,
            &mut encoder,
            &window.below,
            below,
            window.pending_ui,
        );
        if window.pending_ui {
            window
                .overlay
                .prepare(&gpu.queue, &window.above, above, above_rows);
        }
        window.pending_ui = false;
        if let Some(inst) = inst.as_mut() {
            let format = wgpu::TextureFormat::Rgba8Unorm;
            inst.surface.prepare_assets(&gpu.device, &gpu.queue, format);
            let frame = Frame {
                seekable: self.seekable,
                period_ms: self.period_ms,
                children_generation: inst.children_generation,
                shader_generation: shaders::shader_generation(),
                headroom: 1.,
                ..*frame
            };
            inst.surface.render(
                &frame,
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &window.scene.create_view(&Default::default()),
                format,
            );
        } else {
            // Keep presenting menus through the same swapchain. Switching this
            // HWND back to GDI can leave the last GPU image covering the menu.
            let view = window.scene.create_view(&Default::default());
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        {
            let view = drawable.texture.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("window composition"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&window.pipeline);
            pass.set_bind_group(0, &window.group, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(inst) = inst {
            inst.surface.submitted();
            inst.drain();
            if let Some(SurfaceError(e)) = inst.surface.take_error() {
                return Err(e);
            }
            inst.dirty = false;
            inst.seen.store(true, Ordering::Release);
        }
        gpu.queue.present(drawable);
        Ok(())
    }

    /// Release the native window before its HWND is destroyed or CPU presentation resumes.
    pub fn detach_window(&mut self) {
        self.window = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_overlay_uploads_clear_moved_and_removed_ink() {
        let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
            return;
        };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 3,
                height: 8,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_formats: &[],
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        });
        let mut overlay = Overlay::default();
        for rows in [0..0, 1..3, 1..3, 5..7, 0..0, 3..4, 0..8, 0..0] {
            let mut data = vec![0; 3 * 8 * 4];
            for y in rows.clone() {
                data[y as usize * 12 + 4..y as usize * 12 + 8].copy_from_slice(&[17, 31, 71, 83]);
            }
            overlay.prepare(&gpu.queue, &texture, &data, rows);
            gpu.queue.submit([]);
            assert_eq!(fixture::read(&gpu, &texture).unwrap().data, data);
            assert_eq!(overlay.pixels, data);
        }
    }

    #[test]
    fn solid_backgrounds_match_uploads_and_restore_previously_painted_bytes() {
        let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
            return;
        };
        for (width, height) in [(3, 4), (1, 1)] {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                view_formats: &[],
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
            });
            let mut background = Background::default();
            let painted: Vec<u8> = (0..width * height)
                .flat_map(|i| [i as u8, 79, 191, 255])
                .collect();
            for (bytes, changed) in [
                (&[0, 0, 0, 0][..], false),
                (&painted[..], true),
                (&[31, 72, 109, 255][..], true),
                (&[31, 72, 109, 255][..], false),
                (&[17, 23, 29, 83][..], true),
                (&painted[..], true),
                (&painted[..], false),
            ] {
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                background.prepare(&gpu.queue, &mut encoder, &texture, bytes, changed);
                gpu.queue.submit([encoder.finish()]);
                let expected = if bytes.len() == 4 {
                    bytes.repeat((width * height) as usize)
                } else {
                    bytes.to_vec()
                };
                assert_eq!(fixture::read(&gpu, &texture).unwrap().data, expected);
                if bytes.len() == 4 {
                    assert!(
                        background.pixels.is_empty(),
                        "a solid layer keeps no CPU image copy"
                    );
                }
            }
        }
    }
    #[test]
    fn hud_upload_preserves_unchanged_rows_and_clears_changed_rows() {
        let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
            return;
        };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 3,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_formats: &[],
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        });
        let mut previous = Vec::new();
        let mut pixels = vec![0; 48];
        for row in [None, Some(0), Some(3), Some(1), None] {
            if let Some(row) = row {
                pixels[row * 12..(row + 1) * 12].fill(100 + row as u8);
            } else {
                pixels.fill(0);
            }
            upload(&gpu.queue, &texture, &mut previous, &pixels);
            gpu.queue.submit([]);
            assert_eq!(fixture::read(&gpu, &texture).unwrap().data, pixels);
        }
        // Adjacent rows change at different columns, with unchanged pixels
        // between them and at both edges of the texture.
        for changes in [[(0, 2), (1, 1)], [(1, 0), (2, 2)], [(0, 2), (3, 0)]] {
            for (row, column) in changes {
                let at = (row * 3 + column) * 4;
                pixels[at..at + 4].fill(77);
            }
            upload(&gpu.queue, &texture, &mut previous, &pixels);
            gpu.queue.submit([]);
            assert_eq!(fixture::read(&gpu, &texture).unwrap().data, pixels);
        }
        pixels.fill(0);
        upload(&gpu.queue, &texture, &mut previous, &pixels);
        gpu.queue.submit([]);
        assert_eq!(fixture::read(&gpu, &texture).unwrap().data, pixels);
    }
}
