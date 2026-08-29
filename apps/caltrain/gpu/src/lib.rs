//! The Caltrain line map: a canvas surface (LLP 1009).
//!
//! Inputs, in `canvas surface=map(line, selectedId, board, nowMs)` order:
//! the stations in line order (records `[id, name, zone, distance]`), the
//! selected station's id, the northbound board (records `[id, train,
//! service, headsign, at]`), and the clock. It draws the line, one dot per
//! station with the selected one highlighted, and the next northbound
//! train as a dot sliding toward the selected station as its countdown
//! runs — every frame from a vertex buffer built on `bind`, so a change of
//! inputs is a new picture and nothing else is.

#![deny(missing_docs)]

pub mod aurora;
pub use aurora::AuroraSurface;

use exact_gpu::json::{list, number, text};
use exact_gpu::wgpu;
use exact_gpu::{Frame, Registry, Surface, SurfaceError, Value};

/// The shader, validated at build (`build.rs`).
pub const MAP_WGSL: &str = include_str!("../shaders/map.wgsl");

/// The line-map surface.
#[derive(Default)]
pub struct MapSurface {
    stations: Vec<String>,
    selected: Option<usize>,
    /// Progress of the next train toward the selected station, 0–1.
    train: Option<f32>,
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

impl MapSurface {
    /// A fresh surface.
    pub fn new() -> MapSurface {
        MapSurface::default()
    }

    /// The vertices for the current inputs at a size in points.
    pub fn vertices(&self, width: f32, height: f32) -> Vec<[f32; 6]> {
        let mut out = Vec::new();
        let n = self.stations.len();
        if n == 0 {
            return out;
        }
        let x = 0.5 * width;
        let top = 16.0;
        let bottom = height - 16.0;
        let y_of = |i: usize| top + (bottom - top) * (i as f32 / (n.max(2) - 1) as f32);
        let ndc = |px: f32, py: f32| [(px / width) * 2.0 - 1.0, 1.0 - (py / height) * 2.0];
        let mut quad = |cx: f32, cy: f32, w: f32, h: f32, color: [f32; 4]| {
            let a = ndc(cx - w / 2.0, cy - h / 2.0);
            let b = ndc(cx + w / 2.0, cy - h / 2.0);
            let c = ndc(cx + w / 2.0, cy + h / 2.0);
            let d = ndc(cx - w / 2.0, cy + h / 2.0);
            for p in [a, b, c, a, c, d] {
                out.push([p[0], p[1], color[0], color[1], color[2], color[3]]);
            }
        };
        let line = [0.75, 0.75, 0.75, 1.0];
        quad(x, (top + bottom) / 2.0, 3.0, bottom - top, line);
        for i in 0..n {
            let selected = self.selected == Some(i);
            let color = if selected {
                [0.75, 0.22, 0.17, 1.0]
            } else {
                [0.4, 0.4, 0.4, 1.0]
            };
            let size = if selected { 14.0 } else { 7.0 };
            quad(x, y_of(i), size, size, color);
        }
        if let (Some(sel), Some(progress)) = (self.selected, self.train) {
            // Northbound: the train comes from the next station south (the
            // higher index) toward the selected one.
            let from = (sel + 1).min(n - 1);
            let y = y_of(from) + (y_of(sel) - y_of(from)) * progress;
            quad(x + 18.0, y, 10.0, 10.0, [0.16, 0.5, 0.9, 1.0]);
        }
        out
    }
}

impl Surface for MapSurface {
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        if inputs.len() != 4 {
            return Err(SurfaceError(format!(
                "map: expected 4 inputs, got {}",
                inputs.len()
            )));
        }
        let stations = list(&inputs[0], "line")?;
        self.stations = stations
            .iter()
            .map(|s| {
                list(s, "station")
                    .and_then(|f| text(f.first().unwrap_or(&Value::Unit), "station.id"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let selected = text(&inputs[1], "selectedId")?;
        self.selected = self.stations.iter().position(|id| *id == selected);
        let board = list(&inputs[2], "board")?;
        let now = number(&inputs[3], "nowMs")?;
        self.train = board.first().and_then(|d| {
            let fields = list(d, "departure").ok()?;
            let at = number(fields.get(4)?, "at").ok()?;
            let remaining_min = ((at - now) / 60_000.0).max(0.0);
            // A train is "coming" over the last twenty minutes.
            Some((1.0 - (remaining_min / 20.0).min(1.0)) as f32)
        });
        Ok(())
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        if self.pipeline.as_ref().map(|(f, _)| *f) != Some(format) {
            self.pipeline = Some((format, pipeline(device, format)));
        }
        let (_, pipeline) = self.pipeline.as_ref().unwrap();
        let vertices = self.vertices(frame.width, frame.height);
        let bytes: Vec<u8> = vertices
            .iter()
            .flat_map(|v| v.iter().flat_map(|f| f.to_le_bytes()))
            .collect();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map vertices"),
            size: (bytes.len().max(4) as u64 + 3) & !3,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !bytes.is_empty() {
            queue.write_buffer(&buffer, 0, &bytes);
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.97,
                            g: 0.97,
                            b: 0.97,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        queue.submit([encoder.finish()]);
        false
    }
}

fn pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("map"),
        source: wgpu::ShaderSource::Wgsl(MAP_WGSL.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("map"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 24,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn map() -> Box<dyn Surface> {
    Box::new(MapSurface::new())
}

fn aurora() -> Box<dyn Surface> {
    Box::new(AuroraSurface::new())
}

/// The module's surfaces: name, arity, factory.
pub static REGISTRY: Registry = Registry(&[("map", 4, map), ("aurora", 1, aurora)]);

exact_gpu::module!(REGISTRY);
