//! Weatherlight's living sky, loaded after the first pixel (LLP 1009).
//!
//! `weather(cloudCover, precipitation, daylight, localHour, wind, animated)`
//! consumes percentages, millimetres, a 0–1 daylight value, local hours,
//! kilometres per hour, and an animation toggle. The host draws Contract
//! children over this surface normally; the sky never changes their pixels.

#![deny(missing_docs)]

use exact_gpu::json::number;
use exact_gpu::wgpu;
use exact_gpu::{Frame, Registry, Surface, SurfaceError, Value};

/// Shader interfaces reflected from WGSL during the build.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

use shaders::weather::{entry, module, Uniforms, GROUP_0, U};

/// A sky painted from forecast conditions and the host's seekable clock.
#[derive(Default)]
pub struct WeatherSurface {
    cloud: f32,
    rain: f32,
    daylight: f32,
    hour: f32,
    wind: f32,
    animated: bool,
    animation_ms: Option<f64>,
    clock_anchor: Option<f64>,
    gpu: Option<Gpu>,
}

struct Gpu {
    format: wgpu::TextureFormat,
    generation: u32,
    pipeline: wgpu::RenderPipeline,
    /// Written in place each frame (`FrameUniform`); a bind group per slot.
    uniforms: exact_gpu::FrameUniform,
    bind_groups: Vec<wgpu::BindGroup>,
}

fn input(value: &Value, name: &str, maximum: f64) -> Result<f32, SurfaceError> {
    let value = number(value, name)?;
    if !value.is_finite() {
        return Err(SurfaceError(format!(
            "weather.{name}: expected a finite number"
        )));
    }
    Ok(value.clamp(0.0, maximum) as f32)
}

impl Surface for WeatherSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        let [cloud, rain, daylight, hour, wind, animated] = inputs else {
            return Err(SurfaceError(format!(
                "weather: expected 6 inputs, got {}",
                inputs.len()
            )));
        };
        let Value::Bool(animated) = animated else {
            return Err(SurfaceError("weather.animated: expected a boolean".into()));
        };
        // Validate the complete input before replacing the current scene.
        let cloud = input(cloud, "cloudCover", 100.0)? / 100.0;
        let rain = input(rain, "precipitation", 100.0)?;
        let daylight = input(daylight, "daylight", 1.0)?;
        let hour = input(hour, "localHour", 24.0)?;
        let wind = input(wind, "wind", 200.0)?;
        self.cloud = cloud;
        self.rain = rain;
        self.daylight = daylight;
        self.hour = hour;
        self.wind = wind;
        if self.animated != *animated {
            // A toggle keeps the last picture and starts a fresh clock
            // interval, excluding time spent paused from future motion.
            self.clock_anchor = None;
        }
        self.animated = *animated;
        Ok(())
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
        if self.gpu.as_ref().map(|g| (g.format, g.generation))
            != Some((format, frame.shader_generation))
        {
            self.gpu = Some(build(device, format, frame.shader_generation));
        }
        let gpu = self.gpu.as_mut().unwrap();
        let (width, height) = frame.pixels();
        let animation_ms = self.animation_ms.get_or_insert_with(|| {
            if self.animated {
                frame.now_ms.rem_euclid(3_600_000.0)
            } else {
                0.0
            }
        });
        if self.animated {
            if let Some(anchor) = self.clock_anchor {
                *animation_ms = (*animation_ms + frame.now_ms - anchor).rem_euclid(3_600_000.0);
            }
            self.clock_anchor = Some(frame.now_ms);
        }
        let uniforms = Uniforms {
            // Reduce the epoch clock before f32 conversion so even today's
            // millisecond timestamps retain smooth, sub-frame motion.
            time: (*animation_ms * 0.001) as f32,
            width: width.max(1) as f32,
            height: height.max(1) as f32,
            cloud: self.cloud,
            rain: self.rain,
            daylight: self.daylight,
            hour: self.hour,
            wind: self.wind,
        };
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weather sky"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, &gpu.bind_groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        self.animated
    }
}

fn build(device: &wgpu::Device, format: wgpu::TextureFormat, generation: u32) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let uniforms = exact_gpu::FrameUniform::new(device, Uniforms::SIZE, "weather uniforms");
    let bind_groups = (0..uniforms.slots())
        .map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("weather"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: U.binding,
                    resource: uniforms.buffer(slot).as_entire_binding(),
                }],
            })
        })
        .collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("weather"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("weather"),
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
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    Gpu {
        format,
        generation,
        pipeline,
        uniforms,
        bind_groups,
    }
}

fn weather() -> Box<dyn Surface> {
    Box::new(WeatherSurface::default())
}

/// The optional module's single surface and reflected shader interfaces.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("weather", 6, weather)],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use exact_gpu::fixture;

    #[test]
    fn forecast_changes_the_sky_and_pause_stops_the_clock() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
        exact_gpu::shaders::load_dir(&dir, &REGISTRY).unwrap();
        let gpu = match fixture::device() {
            Ok(gpu) => gpu,
            Err(error) => {
                eprintln!("{error}; weather GPU readback skipped");
                return;
            }
        };
        let mut sky = WeatherSurface::default();
        let inputs = |cloud, rain, daylight, hour, animated| {
            [
                Value::Number(cloud),
                Value::Number(rain),
                Value::Number(daylight),
                Value::Number(hour),
                Value::Number(18.0),
                Value::Bool(animated),
            ]
        };
        let mut frame = Frame {
            width: 960.0,
            height: 600.0,
            scale: 1.0,
            now_ms: 1_789_000_000_000.0,
            children_generation: 0,
            seekable: false,
            period_ms: 0.0,
            shader_generation: exact_gpu::shaders::shader_generation(),
            headroom: 1.0,
        };
        sky.bind(&inputs(45.0, 0.0, 1.0, 14.0, true), None).unwrap();
        let (day, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(animated);
        day.save("weather-day");
        frame.now_ms += 5_000.0;
        let (moving, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(animated);
        assert!(
            (0..600).step_by(20).any(|y| (0..960)
                .step_by(20)
                .any(|x| day.at(x, y) != moving.at(x, y))),
            "clouds must move even when the host clock is an epoch timestamp"
        );
        sky.bind(&inputs(45.0, 0.0, 1.0, 14.0, false), None)
            .unwrap();
        frame.now_ms += 100.0;
        let (paused_moving, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(!animated);
        assert_eq!(
            moving.data, paused_moving.data,
            "pausing preserves the last frame"
        );
        frame.now_ms += 30_000.0;
        let (still_paused, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(!animated);
        assert_eq!(paused_moving.data, still_paused.data);
        sky.bind(&inputs(45.0, 0.0, 1.0, 14.0, true), None).unwrap();
        frame.now_ms += 30_000.0;
        let (resumed, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(animated);
        assert_eq!(
            paused_moving.data, resumed.data,
            "resuming excludes paused time"
        );
        frame.now_ms += 5_000.0;
        let (moving_again, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(animated);
        assert_ne!(
            resumed.data, moving_again.data,
            "resumed clouds move on the next clock"
        );
        sky.bind(&inputs(45.0, 0.0, 0.0, 23.0, true), None).unwrap();
        let (night, _) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        night.save("weather-night");
        assert!(day.at(20, 200)[1] > night.at(20, 200)[1]);
        sky.bind(&inputs(45.0, 0.0, 1.0, 18.5, true), None).unwrap();
        let (dusk, _) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        dusk.save("weather-dusk");
        sky.bind(&inputs(95.0, 4.0, 1.0, 14.0, false), None)
            .unwrap();
        let (paused, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(!animated);
        paused.save("weather-rain");
        frame.now_ms += 30_000.0;
        let (later, animated) = fixture::render(&gpu, &mut sky, &frame).unwrap();
        assert!(!animated);
        for y in (0..600).step_by(20) {
            for x in (0..960).step_by(20) {
                assert_eq!(paused.at(x, y), later.at(x, y));
            }
        }
    }
}
