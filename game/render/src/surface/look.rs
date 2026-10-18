//! Look frames (LLP 1127.000 D1, D3, D4): the current tick drawn once,
//! offscreen, with the renderer's probe armed, and read back. A canvas that
//! draws (web, Apple, a rendering Linux host) draws its look frame with its own
//! renderer. A device-less one, the GPU-less proof host, makes a device on its
//! first look frame where the machine has an adapter
//! (`exact_gpu::fixture::device`) and keeps a renderer for it beside its
//! headless state; without an adapter it answers that there is no picture.
//!
//! Only under the agent's clock, and only in a development build: a build
//! baked with `EXACT_UPDATE_TRUST=production` refuses every look request by
//! name. A look frame runs no simulation tick: it draws the tick the agent's
//! clock has reached, and the world hash does not move. It is observational
//! for hooks too: they see the held frame with no time passing, and the hook
//! clock keeps the last live frame's time. Its readback is the frame's colour
//! target, the particle counts, the probe's query results and, for a culled
//! frame, the indirect arguments and the compacted lists; `look.readbackBytes`
//! reports its size.
use super::*;
use crate::look::{Ask, View};
use crate::renderer::probe::{Facts, Readback, Record};

pub(super) struct Look<const ASSETS: bool> {
    /// The device-less surface's device, made on first use.
    #[cfg(not(target_arch = "wasm32"))]
    gpu: Option<Result<exact_gpu::Gpu, String>>,
    /// The device-less surface's look renderer and format, between look frames.
    stash: Option<(crate::renderer::RendererWithAssets<ASSETS>, Feed)>,
    stash_format: Option<wgpu::TextureFormat>,
    /// A readback still mapping (on the web, until the page's next turn).
    pending: Option<Pending>,
    /// The line each tell last told: a settled frame tells once.
    logged: Vec<(&'static str, String)>,
    /// Tell lines the next `logs` reply carries. They stay out of the world's
    /// journal, which a save carries: presentation never enters a save.
    unread: Vec<String>,
    /// The canvas's size in points: its last frame's, or the last request that said it.
    pub(super) points: (f32, f32),
    /// The last live frame: a look frame draws it held.
    pub(super) held: Option<Frame>,
}
impl<const ASSETS: bool> Default for Look<ASSETS> {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            gpu: None,
            stash: None,
            stash_format: None,
            pending: None,
            logged: Vec::new(),
            unread: Vec::new(),
            points: (0., 0.),
            held: None,
        }
    }
}

struct Pending {
    ask: Ask,
    tick: u64,
    hash: u64,
    device: wgpu::Device,
    readback: Readback,
    facts: Facts,
    records: Vec<Record>,
    labels: Vec<String>,
    view_proj: glam::Mat4,
    camera: glam::Vec3,
    /// The frame's size in points, for the picture's reply.
    points: (f32, f32),
    /// Drawn at the canvas's whole resolution: only such a frame is judged.
    whole: bool,
}

/// Why a look frame was not drawn.
enum Refusal {
    /// No device on this host: the proof's "no picture on this host".
    NoPicture(String),
    Refused(String),
}
impl From<String> for Refusal {
    fn from(e: String) -> Self {
        Refusal::Refused(e)
    }
}

#[derive(Data)]
struct Size {
    width: f32,
    height: f32,
    scale: f32,
    now: f64,
    /// Only a canvas that draws answers (the driver's tells at `clock settle`
    /// and `screenshot`): a device-less one drew no frame to tell about.
    drawing: bool,
    /// The fraction of the canvas's pixels to draw, in (0, 1]: a contact
    /// sheet's frames need not be read back whole.
    resolution: f32,
}
impl Default for Size {
    fn default() -> Self {
        Self {
            width: 0.,
            height: 0.,
            scale: 1.,
            now: f64::NAN,
            drawing: false,
            resolution: 1.,
        }
    }
}

impl<G: Game, P: Executor, const ASSETS: bool, H: crate::Hooks> WorldSurface<G, P, ASSETS, H> {
    /// Answer a look request; `None` when `request` is not one.
    pub(super) fn look(&mut self, request: &str) -> Option<String> {
        let tick = |s: &Self| s.sim.as_ref().map_or(0, |s| s.world().tick());
        // Every canvas request says its size; a look frame takes the last one.
        if request.contains("\"width\"") {
            let size: Size = exact_game::json::from_str(request).unwrap_or_default();
            if size.width > 0.
                && size.height > 0.
                && size.width.is_finite()
                && size.height.is_finite()
            {
                self.look.points = (size.width, size.height);
            }
        }
        let ask = match crate::look::ask(request) {
            Ok(Some(ask)) => ask,
            Ok(None) => return None,
            Err(e) => {
                return Some(format!(
                    "{{\"tick\":{},\"error\":{}}}",
                    tick(self),
                    crate::look::quote(&e)
                ))
            }
        };
        Some(match self.answer_look(request, ask) {
            Ok(reply) => reply,
            Err(Refusal::NoPicture(why)) => format!(
                "{{\"tick\":{},\"unavailable\":{}}}",
                tick(self),
                crate::look::quote(&format!("no picture on this host: {why}"))
            ),
            Err(Refusal::Refused(e)) => format!(
                "{{\"tick\":{},\"error\":{}}}",
                tick(self),
                crate::look::quote(&e)
            ),
        })
    }

    fn answer_look(&mut self, request: &str, ask: Ask) -> Result<String, Refusal> {
        if !inspects(option_env!("EXACT_UPDATE_TRUST")) {
            return Err(Refusal::Refused(PRODUCTION.into()));
        }
        if !self.seekable {
            return Err("a look frame is drawn only under the agent's clock"
                .to_string()
                .into());
        }
        let size: Size = exact_game::json::from_str(request).unwrap_or_default();
        let sim = self
            .sim
            .as_mut()
            .ok_or("the world has not been bound".to_string())?;
        // As every agent request, the world catches up to the host's clock. The
        // canvas's size is the look frame's own (`look.points`); the input's
        // viewport, which a save carries, is left as it is.
        if size.now.is_finite() {
            sim.advance_with(
                size.now,
                Clock::Seekable,
                observer(
                    &mut self.render,
                    &mut self.placed,
                    &mut self.perf,
                    &mut self.trace,
                    &mut self.error,
                    &mut self.hook_poses,
                    false,
                    0,
                ),
            );
        }
        let (tick, hash) = (sim.world().tick(), sim.world().hash());
        // A readback still mapping answers the same question at the same tick.
        let pending = match self.look.pending.take() {
            Some(p) if p.ask == ask && p.tick == tick && p.hash == hash => p,
            _ => {
                let points = self.look.points;
                if points.0 <= 0. || points.1 <= 0. {
                    return Err(
                        "a look frame needs the canvas's size: give width and height"
                            .to_string()
                            .into(),
                    );
                }
                if size.drawing && !(self.device && self.render.is_some()) {
                    return Err(Refusal::NoPicture("the canvas draws no frames".into()));
                }
                let resolution = size.resolution.clamp(0.05, 1.);
                let scale = size.scale * resolution;
                let mut started = self.start_look(ask, points, scale, tick, hash)?;
                started.whole = resolution >= 1.;
                started
            }
        };
        if !pending.readback.ready(&pending.device)? {
            self.look.pending = Some(pending);
            return Ok(format!("{{\"tick\":{tick},\"pending\":\"readback\"}}"));
        }
        self.finish_look(pending)
    }

    fn start_look(
        &mut self,
        ask: Ask,
        points: (f32, f32),
        scale: f32,
        tick: u64,
        hash: u64,
    ) -> Result<Pending, Refusal> {
        let live = self.device && self.render.is_some();
        let (device, queue, format) = if live {
            let (renderer, _) = self.render.as_ref().unwrap();
            let format = self.format.ok_or("no format".to_string())?;
            if format.block_copy_size(None) != Some(4) {
                return Err(Refusal::NoPicture(format!(
                    "the canvas draws {format:?}, and a look frame reads back 8-bit colour"
                )));
            }
            (renderer.device.clone(), renderer.queue.clone(), format)
        } else {
            #[cfg(target_arch = "wasm32")]
            return Err(Refusal::NoPicture("the canvas has no device".into()));
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Preparing models and textures would change the headless
                // world's asset bookkeeping, so they draw where the canvas does.
                if ASSETS {
                    return Err(Refusal::NoPicture(
                        "a model-capable module draws its look frames on a drawing host (web, macOS)".into(),
                    ));
                }
                let format = if self.srgb_target() {
                    wgpu::TextureFormat::Rgba8UnormSrgb
                } else {
                    wgpu::TextureFormat::Rgba8Unorm
                };
                let gpu = self.look.gpu.get_or_insert_with(exact_gpu::fixture::device);
                let gpu = gpu.as_ref().map_err(|e| Refusal::NoPicture(e.clone()))?;
                (gpu.device.clone(), gpu.queue.clone(), format)
            }
        };
        if !live {
            // The device-less surface draws with its look renderer; its own
            // headless state comes back untouched below.
            std::mem::swap(&mut self.render, &mut self.look.stash);
            std::mem::swap(&mut self.format, &mut self.look.stash_format);
            if let Some((_, feed)) = &mut self.render {
                feed.reset();
            }
        }
        let saved = (self.dirty, self.device, self.ready_work, self.assets_dirty);
        if !live {
            self.dirty = true;
            self.assets_dirty = true;
        }
        let drawn = self.draw_look(&device, &queue, format, points, scale, &ask);
        if !live {
            std::mem::swap(&mut self.render, &mut self.look.stash);
            std::mem::swap(&mut self.format, &mut self.look.stash_format);
            (self.dirty, self.device, self.ready_work, self.assets_dirty) = saved;
        }
        let (readback, facts, records, labels, view_proj, camera) = drawn?;
        Ok(Pending {
            ask,
            tick,
            hash,
            device,
            readback,
            facts,
            records,
            labels,
            view_proj,
            camera,
            points,
            whole: true,
        })
    }

    #[allow(clippy::type_complexity)]
    fn draw_look(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        points: (f32, f32),
        scale: f32,
        ask: &Ask,
    ) -> Result<
        (
            Readback,
            Facts,
            Vec<Record>,
            Vec<String>,
            glam::Mat4,
            glam::Vec3,
        ),
        Refusal,
    > {
        if !self.storage_fits(device) {
            return Err(self.error.clone().map_or_else(String::new, |e| e.0).into());
        }
        self.prepare_assets(device, queue, format);
        let sim = self
            .sim
            .as_mut()
            .ok_or("the world has not been bound".to_string())?;
        if sim.is_loading() {
            return Err("declared assets are still loading; `clock settle` first"
                .to_string()
                .into());
        }
        if self.format != Some(format) || self.render.is_none() {
            self.render = Some((
                crate::renderer::RendererWithAssets::<ASSETS>::configured(
                    device,
                    queue,
                    format,
                    G::SAMPLES,
                    G::DEPTH_PREPASS,
                )
                .with_matte_models(G::MATTE_MODELS),
                Feed::default(),
            ));
            self.format = Some(format);
            self.dirty = true;
        }
        let (renderer, feed) = self.render.as_mut().unwrap();
        if ASSETS {
            for (name, model) in &mut renderer.models.loaded {
                let active = sim.model_prepared(name);
                if model.active != active {
                    model.active = active;
                    renderer.models.revision += 1;
                }
            }
        }
        if self.dirty {
            feed.feed(sim.world(), renderer)
                .map_err(|e| e.to_string())?;
        }
        // The last live frame, held, at the look frame's size. A device-less
        // surface has drawn none: its look renderer is its own.
        let held = self.look.held.unwrap_or(Frame {
            width: 0.,
            height: 0.,
            scale: 1.,
            now_ms: 0.,
            seekable: true,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
            headroom: 1.,
        });
        let frame = Frame {
            width: points.0,
            height: points.1,
            scale: if scale.is_finite() && scale > 0. {
                scale
            } else {
                1.
            },
            seekable: true,
            ..held
        };
        self.hook_poses.track(self.hooks.entities(), sim.world());
        self.hook_poses.sync(sim.world());
        let hook_time = if H::ENABLED {
            self.hook_clock.held(
                &frame,
                sim.world().presentation_generation(),
                G::paused(sim.args()),
            )
        } else {
            crate::HookTime::default()
        };
        let view = match ask {
            Ask::Picture(view) => *view,
            _ => View::Camera,
        };
        let mut input = feed.frame_pixels(sim.world(), sim.alpha(), points);
        input.headroom = frame.headroom;
        let models = renderer.model_boxes();
        view.apply(
            &mut input,
            sim.world(),
            glam::Vec2::new(points.0, points.1),
            &models,
        );
        let (view_proj, camera) = (input.proj * input.view, input.camera_position);
        let (width, height) = frame.pixels();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("game look frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.arm_probe();
        let encoded = renderer.encode_hooked(
            &mut encoder,
            &target,
            (width, height),
            &input,
            &mut self.hooks,
            Some(sim.world()),
            hook_time,
            &self.hook_poses,
        );
        if let Err(e) = encoded {
            renderer.probe = None;
            return Err(e.to_string().into());
        }
        let records = renderer
            .probe
            .as_ref()
            .map(|p| p.records.take())
            .unwrap_or_default();
        let facts = renderer.probe_facts();
        let labels = renderer.probe_labels(&feed.shape_names());
        let readback = renderer.probe_copies(&mut encoder, &texture, &facts);
        queue.submit([encoder.finish()]);
        renderer.submitted();
        readback.map();
        Ok((readback, facts, records, labels, view_proj, camera))
    }

    fn finish_look(&mut self, p: Pending) -> Result<String, Refusal> {
        let bytes = p.readback.bytes;
        let read = p.readback.read()?;
        self.perf.look.0 += 1;
        self.perf.look.1 += bytes;
        let sim = self
            .sim
            .as_ref()
            .ok_or("the world has not been bound".to_string())?;
        let w = sim.world();
        let stats = crate::tells::FrameStats::of(&read.rgba);
        let drawn = crate::look::drawn(&p.records, &p.facts, &read);
        let judged = crate::tells::judge(&stats, &drawn);
        // `logs` gets each tell of a whole frame once per change; a reduced
        // frame (a contact sheet's) is a picture, and tells nothing.
        for tell in judged.tells.iter().filter(|_| p.whole) {
            if !self
                .look
                .logged
                .iter()
                .any(|(n, l)| *n == tell.name && *l == tell.line)
            {
                self.look
                    .unread
                    .push(format!("tick={} {}", p.tick, tell.line));
                self.look.logged.retain(|(n, _)| *n != tell.name);
                self.look.logged.push((tell.name, tell.line.clone()));
            }
        }
        let head = format!("{{\"tick\":{},\"hash\":\"0x{:016x}\"", p.tick, p.hash);
        let mut look = crate::look::tells_json(&stats, &drawn, &judged, bytes);
        if !p.whole {
            look.insert_str(look.len() - 1, ",\"reduced\":true");
        }
        Ok(match &p.ask {
            Ask::Tells => format!("{head},\"look\":{look}}}"),
            Ask::Picture(_) => format!(
                "{head},\"look\":{look},\"w\":{},\"h\":{},\"width\":{},\"height\":{},\"data\":\"{}\"}}",
                p.points.0,
                p.points.1,
                read.width,
                read.height,
                crate::look::base64(&read.rgba)
            ),
            Ask::Draws => format!(
                "{head},\"draws\":{}}}",
                crate::look::draws_json(
                    &p.records,
                    &p.facts,
                    &read,
                    &crate::look::Labels { batches: p.labels }
                )
            ),
            Ask::Visible(name) => {
                let entity = w.resolve(name).ok_or_else(|| {
                    format!("no entity named `{name}`; `tree world` lists names")
                })?;
                let (word, reason) = crate::look::visible(
                    w,
                    entity,
                    |model| !sim.model_prepared(model),
                    p.camera,
                    p.view_proj,
                    &p.facts,
                    &read,
                );
                format!(
                    "{head},\"entity\":{{\"id\":{},\"name\":{}}},\"visible\":{},\"reason\":{}}}",
                    entity.index(),
                    crate::look::quote(name),
                    crate::look::quote(&word),
                    crate::look::quote(&reason)
                )
            }
        })
    }
}

/// Why a production build answers no look request.
const PRODUCTION: &str = "look frames are a development capability: this production build \
     (EXACT_UPDATE_TRUST=production) refuses `screenshot … from`, `state world look`, \
     `state world draws` and `state world:<entity> visible`";

/// Whether a build baked with this trust answers look requests: every build
/// but a production one (`scripts/app.mjs` bakes development trust unless told).
fn inspects(trust: Option<&str>) -> bool {
    trust != Some("production")
}

impl<const ASSETS: bool> Look<ASSETS> {
    /// Add the unread tell lines to a world's `logs` reply (`{…,"lines":[…]}`).
    pub(super) fn tell_into(&mut self, request: &str, reply: &mut String) {
        if self.unread.is_empty() || !request.contains("\"logs\"") || !reply.ends_with("]}") {
            return;
        }
        let at = reply.len() - 2;
        let mut lines = String::new();
        for line in self.unread.drain(..) {
            lines.push(',');
            lines.push_str(&crate::look::quote(&line));
        }
        if reply[..at].ends_with('[') {
            lines.remove(0);
        }
        reply.insert_str(at, &lines);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_production_build_refuses_look_frames_by_name() {
        assert!(super::inspects(None));
        assert!(super::inspects(Some("development")));
        assert!(!super::inspects(Some("production")));
        assert!(super::PRODUCTION.contains("production build"));
    }
}
