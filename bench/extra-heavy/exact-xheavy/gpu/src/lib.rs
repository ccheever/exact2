//! The shader row's surface (SPEC 3, LLP 1009): `wave(photo, duoA, duoB, freeze)`.
//!
//! The photo is an app asset the surface asks for (`assets` → `asset`). Its JPEG is decoded
//! off the main thread, downsampled to at most the box's pixel size, and uploaded once per name
//! for every row that shows it (a small LRU of textures shared by all instances). Each frame
//! samples it through the wave displacement and mixes in the duotone (`shaders/wave.wgsl`).
//! t is the host's frame clock in seconds since launch; under `freeze` it is 1.25 and the
//! surface draws once.

#![deny(missing_docs)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc;

use exact_gpu::json::text;
use exact_gpu::wgpu;
use exact_gpu::{AssetChanges, AssetError, Frame, Registry, Surface, SurfaceError, Value};

mod fit;

/// Shader interfaces reflected from WGSL during the build.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

use shaders::wave::{entry, module, Uniforms, GROUP_0, PHOTO, PHOTO_SAMPLER, U};

/// Longest side a decoded photo keeps: the widest box (600 pt) at 2× is 1,200 px.
const MAX_SIDE: u32 = 1200;
/// Decoded photos kept on the GPU, shared by every row.
const KEEP: usize = 40;

fn hex(s: &str) -> [f32; 4] {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
    [((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0, 1.0]
}

struct Photo {
    name: String,
    view: wgpu::TextureView,
    w: u32,
    h: u32,
}

/// Decoded pixels on their way to the GPU.
type Decoded = (String, Vec<u8>, u32, u32);

struct Shared {
    key: (wgpu::TextureFormat, u32),
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

thread_local! {
    static SHARED: RefCell<Option<Shared>> = const { RefCell::new(None) };
    static PHOTOS: RefCell<VecDeque<Photo>> = const { RefCell::new(VecDeque::new()) };
    static DECODED: (mpsc::Sender<Decoded>, RefCell<mpsc::Receiver<Decoded>>) = {
        let (tx, rx) = mpsc::channel();
        (tx, RefCell::new(rx))
    };
    static DECODING: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn cached(name: &str) -> Option<(wgpu::TextureView, u32, u32)> {
    PHOTOS.with(|p| p.borrow().iter().find(|x| x.name == name).map(|x| (x.view.clone(), x.w, x.h)))
}

fn decoding(name: &str) -> bool {
    DECODING.with(|d| d.borrow().iter().any(|n| n == name))
}

/// Upload whatever the decoder threads finished.
fn upload(device: &wgpu::Device, queue: &wgpu::Queue) {
    let done: Vec<Decoded> = DECODED.with(|(_, rx)| rx.borrow().try_iter().collect());
    for (name, px, w, h) in done {
        DECODING.with(|d| d.borrow_mut().retain(|n| *n != name));
        let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("photo"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // sRGB-encoded values, sampled as they are (SPEC 3).
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            tex.as_image_copy(),
            &px,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: None },
            size,
        );
        let view = tex.create_view(&Default::default());
        PHOTOS.with(|p| {
            let mut p = p.borrow_mut();
            p.push_back(Photo { name, view, w, h });
            while p.len() > KEEP {
                p.pop_front();
            }
        });
    }
}

struct Gpu {
    key: (wgpu::TextureFormat, u32),
    uniforms: exact_gpu::FrameUniform,
    group: Option<(String, Vec<wgpu::BindGroup>)>,
}

/// One shader row.
#[derive(Default)]
pub struct Wave {
    photo: String,
    a: [f32; 4],
    b: [f32; 4],
    freeze: bool,
    asked: Option<String>,
    /// The photo this row drew, kept while the row shows it: the shared cache
    /// may evict it, and the row must not fall back to the placeholder.
    held: Option<Photo>,
    gpu: Option<Gpu>,
}

impl Surface for Wave {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        let [photo, a, b, freeze] = inputs else {
            return Err(SurfaceError(format!("wave: expected 4 inputs, got {}", inputs.len())));
        };
        self.photo = text(photo, "wave.photo")?;
        self.a = hex(&text(a, "wave.duoA")?);
        self.b = hex(&text(b, "wave.duoB")?);
        let Value::Bool(f) = freeze else { return Err(SurfaceError("wave.freeze: expected a boolean".into())) };
        self.freeze = *f;
        Ok(())
    }

    fn assets(&mut self) -> AssetChanges {
        if self.asked.as_deref() == Some(self.photo.as_str())
            || self.held.as_ref().is_some_and(|p| p.name == self.photo)
            || cached(&self.photo).is_some()
            || decoding(&self.photo)
        {
            return AssetChanges::default();
        }
        let retired = self.asked.replace(self.photo.clone()).into_iter().collect();
        AssetChanges { requests: vec![self.photo.clone()], retired }
    }

    fn asset(&mut self, name: &str, bytes: Result<&[u8], AssetError>) {
        let Ok(bytes) = bytes else { return };
        if cached(name).is_some() || decoding(name) {
            return;
        }
        DECODING.with(|d| d.borrow_mut().push(name.to_string()));
        let (name, bytes) = (name.to_string(), bytes.to_vec());
        let tx = DECODED.with(|(tx, _)| tx.clone());
        let decode = move || {
            let Ok(img) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg) else { return };
            // image's Triangle resize to ≤ MAX_SIDE then `to_rgba8`, bit for bit, 3× faster.
            let (rgba, w, h) = fit::fit_rgba8(&img, MAX_SIDE);
            let _ = tx.send((name, rgba, w, h));
        };
        // The web build (../web): wasm32-unknown-unknown has no threads, so the JPEG is
        // decoded where the asset arrives.
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(decode);
        #[cfg(target_arch = "wasm32")]
        decode();
    }

    fn preparing(&self) -> bool {
        decoding(&self.photo)
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        upload(device, queue);
        let key = (format, frame.shader_generation);
        if self.gpu.as_ref().map(|g| g.key) != Some(key) {
            self.gpu = Some(build(device, key));
        }
        if self.held.as_ref().is_none_or(|p| p.name != self.photo) {
            self.held = cached(&self.photo).map(|(view, w, h)| Photo { name: self.photo.clone(), view, w, h });
        }
        let Some((view, iw, ih)) = self.held.as_ref().map(|p| (p.view.clone(), p.w, p.h)) else {
            // Not decoded yet: the box stays the placeholder grey; ask again next frame.
            clear(encoder, target);
            return true;
        };
        let (pipeline, layout, sampler) =
            SHARED.with(|s| s.borrow().as_ref().map(|s| (s.pipeline.clone(), s.layout.clone(), s.sampler.clone())).unwrap());
        let gpu = self.gpu.as_mut().unwrap();
        if gpu.group.as_ref().map(|g| g.0.as_str()) != Some(self.photo.as_str()) {
            let group = (0..gpu.uniforms.slots())
                .map(|slot| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("wave"),
                        layout: &layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: U.binding, resource: gpu.uniforms.buffer(slot).as_entire_binding() },
                            wgpu::BindGroupEntry { binding: PHOTO.binding, resource: wgpu::BindingResource::TextureView(&view) },
                            wgpu::BindGroupEntry { binding: PHOTO_SAMPLER.binding, resource: wgpu::BindingResource::Sampler(&sampler) },
                        ],
                    })
                })
                .collect();
            gpu.group = Some((self.photo.clone(), group));
        }
        let (w, h) = (frame.width, frame.height);
        // Aspect-fill: the image's visible window, centred.
        let s = (w / iw as f32).max(h / ih as f32);
        let (fx, fy) = (w / s / iw as f32, h / s / ih as f32);
        let t = if self.freeze { 1.25 } else { (frame.now_ms / 1000.0) as f32 };
        let u = Uniforms {
            size: [w, h, frame.scale, t],
            duo_a: self.a,
            duo_b: self.b,
            cover: [fx, fy, (1.0 - fx) / 2.0, (1.0 - fy) / 2.0],
            flags: [if format.is_srgb() { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        };
        let slot = gpu.uniforms.write(queue, &u.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wave"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &gpu.group.as_ref().unwrap().1[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        // Every frame while live (SPEC 3); a frozen row is drawn once.
        !self.freeze
    }
}

/// The placeholder (#E5E5EA) while the photo decodes.
fn clear(enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    {
        let c = 0xE5 as f64 / 255.0;
        let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wave placeholder"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: c, g: c, b: 0xEA as f64 / 255.0, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
}

fn build(device: &wgpu::Device, key: (wgpu::TextureFormat, u32)) -> Gpu {
    SHARED.with(|s| {
        let mut s = s.borrow_mut();
        if s.as_ref().map(|s| s.key) != Some(key) {
            let shader = device.create_shader_module(module());
            let layout = device.create_bind_group_layout(&GROUP_0);
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("wave"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("wave"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(entry::VS),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry::FS),
                    targets: &[Some(key.0.into())],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("photo"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            *s = Some(Shared { key, pipeline, layout, sampler });
        }
    });
    let uniforms = exact_gpu::FrameUniform::new(device, Uniforms::SIZE, "wave uniforms");
    Gpu { key, uniforms, group: None }
}

fn wave() -> Box<dyn Surface> {
    Box::new(Wave::default())
}

/// The module's one surface and its reflected shader.
pub static REGISTRY: Registry = Registry { surfaces: &[("wave", 4, wave)], shaders: shaders::SHADERS };

exact_gpu::module!(REGISTRY);
