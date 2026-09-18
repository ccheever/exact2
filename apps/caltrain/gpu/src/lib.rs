//! The Caltrain line map: a canvas surface (LLP 1009).
//!
//! Inputs, in `canvas surface=map(line, selectedId, board, nowMs)` order:
//! the stations in line order (records `[id, name, zone, distance]`), the
//! selected station's id, the northbound board (records `[id, train,
//! service, headsign, at]`), and the clock. It draws the line and the next
//! northbound train as a dot sliding toward the selected station as its
//! countdown runs — from a vertex buffer built on `bind`, so a change of
//! inputs is a new picture and nothing else is. The stations themselves —
//! a dot and a name each — are the canvas's children (LLP 1014): laid out
//! by the kernel, composited over the surface on every host. The two agree
//! on where a station is by construction: the children are a column with
//! 16 points of padding and 14-point rows spread `space-between`, so the
//! first row's centre is 23 points from the top and the last 23 from the
//! bottom, and the train interpolates between the same centres.

#![deny(missing_docs)]

pub mod aurora;
pub mod glass;
pub mod stack;
pub use aurora::AuroraSurface;
pub use glass::GlassSurface;
pub use stack::StackSurface;

use exact_gpu::json::{list, number, text};
use exact_gpu::wgpu;
use exact_gpu::{Frame, Registry, Surface, SurfaceError, Value};

/// The shaders under `shaders/`, reflected at build (`build.rs`,
/// `exact-gpu-reflect`): each one's entry points, bindings, and layouts as
/// Rust, and its interface digest. The WGSL is the declaration authority — a
/// binding number, an offset, or an entry point's name is never restated
/// here by hand, and a shader edit that moves one is a build error, not a
/// wrong picture. The text itself is not here: it travels as an asset and
/// is registered at run time (LLP 1030 D8, `exact_gpu::shaders`).
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

pub use shaders::map::Vertex;

/// Where this crate's shaders live in the source tree: what a fixture
/// registers before it renders (`exact_gpu::shaders::load_dir`).
#[cfg(not(target_arch = "wasm32"))]
pub fn shader_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders")
}

/// The line-map surface.
#[derive(Default)]
pub struct MapSurface {
    stations: Vec<String>,
    selected: Option<usize>,
    /// Progress of the next train toward the selected station, 0–1.
    train: Option<f32>,
    /// The pipeline, keyed by the target's format and the shader generation
    /// (LLP 1030 D8: a registered edit is a new pipeline at the next frame).
    pipeline: Option<(wgpu::TextureFormat, u32, wgpu::RenderPipeline)>,
}

impl MapSurface {
    /// A fresh surface.
    pub fn new() -> MapSurface {
        MapSurface::default()
    }

    /// The vertices for the current inputs at a size in points.
    pub fn vertices(&self, width: f32, height: f32) -> Vec<Vertex> {
        let mut out = Vec::new();
        let n = self.stations.len();
        if n == 0 {
            return out;
        }
        let x = 0.5 * width;
        // The children's first and last row centres (see the module doc).
        let top = 23.0;
        let bottom = height - 23.0;
        let y_of = |i: usize| top + (bottom - top) * (i as f32 / (n.max(2) - 1) as f32);
        let ndc = |px: f32, py: f32| [(px / width) * 2.0 - 1.0, 1.0 - (py / height) * 2.0];
        let mut quad = |cx: f32, cy: f32, w: f32, h: f32, color: [f32; 4]| {
            let a = ndc(cx - w / 2.0, cy - h / 2.0);
            let b = ndc(cx + w / 2.0, cy - h / 2.0);
            let c = ndc(cx + w / 2.0, cy + h / 2.0);
            let d = ndc(cx - w / 2.0, cy + h / 2.0);
            for position in [a, b, c, a, c, d] {
                out.push(Vertex { position, color });
            }
        };
        let line = [0.75, 0.75, 0.75, 1.0];
        quad(x, (top + bottom) / 2.0, 3.0, bottom - top, line);
        if let (Some(sel), Some(progress)) = (self.selected, self.train) {
            // Northbound: the train comes from the next station south (the
            // higher index) toward the selected one.
            let from = (sel + 1).min(n - 1);
            let y = y_of(from) + (y_of(sel) - y_of(from)) * progress;
            // Left of the line: the names are on its right.
            quad(x - 18.0, y, 10.0, 10.0, [0.16, 0.5, 0.9, 1.0]);
        }
        out
    }
}

impl Surface for MapSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
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
        self.train = board
            .iter()
            .find(|departure| {
                list(departure, "departure")
                    .ok()
                    .and_then(|fields| number(fields.get(4)?, "at").ok())
                    .is_some_and(|at| at >= now)
            })
            .and_then(|d| {
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
        let key = (format, frame.shader_generation);
        if self.pipeline.as_ref().map(|(f, g, _)| (*f, *g)) != Some(key) {
            self.pipeline = Some((format, frame.shader_generation, pipeline(device, format)));
        }
        let (_, _, pipeline) = self.pipeline.as_ref().unwrap();
        let vertices = self.vertices(frame.width, frame.height);
        let bytes: Vec<u8> = vertices.iter().flat_map(|v| v.bytes()).collect();
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
    use shaders::map::{entry, module};
    let shader = device.create_shader_module(module());
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("map"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(entry::VS),
            buffers: &[Some(Vertex::LAYOUT)],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(entry::FS),
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

fn glass() -> Box<dyn Surface> {
    Box::new(GlassSurface::new())
}

fn stack() -> Box<dyn Surface> {
    Box::new(StackSurface::new())
}

/// The module's surfaces — name, arity, factory — and the shaders they
/// bind against, at the interfaces the build reflected.
pub static REGISTRY: Registry = Registry {
    surfaces: &[
        ("map", 4, map),
        ("aurora", 1, aurora),
        ("glass", 2, glass),
        ("stack", 4, stack),
    ],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

#[cfg(test)]
mod tests {
    use super::*;

    fn departure(id: &str, at: f64) -> Value {
        Value::list(vec![
            Value::str(id),
            Value::Number(1.0),
            Value::str("Local"),
            Value::str("San Francisco"),
            Value::Number(at),
        ])
    }

    #[test]
    fn map_selects_the_first_not_yet_departed_train() {
        let mut map = MapSurface::new();
        let station = |id: &str| {
            Value::list(vec![
                Value::str(id),
                Value::str(id),
                Value::Number(1.0),
                Value::Number(0.0),
            ])
        };
        map.bind(
            &[
                Value::list(vec![station("a"), station("b")]),
                Value::str("a"),
                Value::list(vec![
                    departure("past", 1_000.0),
                    departure("next", 1_800_000.0),
                ]),
                Value::Number(600_000.0),
            ],
            None,
        )
        .unwrap();
        assert_eq!(
            map.train,
            Some(0.0),
            "the future train is twenty minutes away"
        );
    }
}
