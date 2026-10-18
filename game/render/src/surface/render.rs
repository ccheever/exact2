//! World advancement and GPU frame ownership.
use super::*;

pub(super) fn retain_primes<const ASSETS: bool>(
    render: &mut Option<(crate::renderer::RendererWithAssets<ASSETS>, Feed)>,
    placed: &mut crate::placed::Placements,
    retain: bool,
) {
    placed.attachments.retain_primes(retain);
    if let Some((renderer, feed)) = render {
        feed.retain_primes(retain);
        if let Some(skin) = &mut renderer.models.skinning {
            skin.retain_primes(retain);
        }
    }
}

impl<G: Game, P: Executor, const ASSETS: bool, H: crate::Hooks> Default
    for WorldSurface<G, P, ASSETS, H>
{
    fn default() -> Self {
        Self {
            sim: None,
            pending_restore: None,
            placed: Default::default(),
            refusal: None,
            presentation: P::default(),
            hooks: H::default(),
            hook_clock: Default::default(),
            hook_poses: Default::default(),
            gpu_timing: None,
            encoded: None,
            render: None,
            format: None,
            payloads: Default::default(),
            device: false,
            device_assets_invalidated: false,
            storage_limit: 0,
            perf: Perf::default(),
            ready_work: None,
            trace: None,
            error: None,
            dirty: true,
            output_opaque: false,
            frame_ready: false,
            assets_dirty: false,
            asset_check: None,
            reported: false,
            generation: 0,
            seekable: true,
            audio_requested: false,
            hidden: false,
            interrupted: false,
            look: Default::default(),
        }
    }
}

impl<G: Game, P: Executor, const ASSETS: bool, H: crate::Hooks> WorldSurface<G, P, ASSETS, H> {
    /// Read the simulation after a successful bind.
    pub fn sim(&self) -> Option<&Sim<G>> {
        self.sim.as_ref()
    }
    /// Read presentation diagnostics owned by this canvas's render hooks.
    pub fn render_hooks(&self) -> &H {
        &self.hooks
    }
    pub(super) fn render_frame(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        self.frame_ready = false;
        if !self.storage_fits(device) {
            return false;
        }
        self.reported = false;
        if self.error.is_some() {
            return false;
        }
        self.device = true;
        self.prepare_assets(device, queue, format);
        let Some(sim) = &mut self.sim else {
            return false;
        };
        if self.generation != sim.world().presentation_generation() {
            self.generation = sim.world().presentation_generation();
            self.dirty = true;
        }
        let drawable = frame.width.is_finite()
            && frame.height.is_finite()
            && frame.width > 0.0
            && frame.height > 0.0
            && frame.scale.is_finite()
            && frame.scale > 0.0;
        if !frame.now_ms.is_finite() {
            return true;
        }
        // The host's display period: the live scheduler looks ahead by it.
        sim.frame_period(frame.period_ms);
        if !drawable {
            if frame.seekable {
                sim.advance_with(
                    frame.now_ms,
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
            self.presentation.sync(
                sim.world(),
                sim.generation(),
                !G::paused(sim.args()),
                frame.seekable,
            );
            self.dirty = true;
            return self.error.is_none();
        }
        if sim.is_loading() {
            sim.advance(
                frame.now_ms,
                if frame.seekable {
                    Clock::Seekable
                } else {
                    Clock::Live
                },
            );
            return sim.assets_pending();
        }
        if ASSETS && !sim.device_assets_ready() {
            return sim.assets_pending();
        }
        sim.viewport(frame.width, frame.height);
        self.look.points = (frame.width, frame.height);
        if self.format != Some(format) {
            self.gpu_timing = None;
            self.hook_clock.reset();
            self.hooks.device_lost();
            self.ready_work = None;
            self.render = Some((
                crate::renderer::RendererWithAssets::<ASSETS>::configured(
                    device,
                    queue,
                    format,
                    G::SAMPLES,
                    G::DEPTH_PREPASS,
                )
                .with_matte_models(G::MATTE_MODELS),
                {
                    let mut feed = Feed::default();
                    feed.share_attachment_diagnostics(self.placed.attachments.diagnostics.clone());
                    feed
                },
            ));
            self.format = Some(format);
            self.dirty = true;
        }
        if ASSETS {
            if let Some((renderer, _)) = &mut self.render {
                for (name, model) in &mut renderer.models.loaded {
                    let active = sim.model_prepared(name);
                    if model.active != active {
                        model.active = active;
                        renderer.models.revision += 1;
                        self.dirty = true;
                    }
                }
            }
        }
        // Seed setup/current state before running ticks; no origin streak on frame one.
        if self.dirty {
            if let Some(trace) = &mut self.trace {
                trace.feed(sim.world());
            }
            if let Err(e) = if ASSETS {
                self.placed.feed(sim.world())
            } else {
                self.placed.feed_primitive(sim.world())
            } {
                self.error = Some(SurfaceError(e.to_string()));
                return false;
            }
            let (renderer, feed) = self.render.as_mut().unwrap();
            if let Err(e) = feed.feed(sim.world(), renderer) {
                self.error = Some(SurfaceError(e.to_string()));
                return false;
            }
        }
        // The dirty feed seeds histories; the final observer feed may replace it.
        retain_primes(&mut self.render, &mut self.placed, true);
        self.perf.pixels = frame.pixels();
        self.perf.frame(frame.now_ms, frame.seekable);
        let due = sim.ticks_due(frame.now_ms, Clock::Live);
        let ticks = sim.advance_with(
            frame.now_ms,
            if frame.seekable {
                Clock::Seekable
            } else {
                Clock::Live
            },
            observer(
                &mut self.render,
                &mut self.placed,
                &mut self.perf,
                &mut self.trace,
                &mut self.error,
                &mut self.hook_poses,
                !frame.seekable,
                due,
            ),
        );
        self.presentation.sync(
            sim.world(),
            sim.generation(),
            !G::paused(sim.args()),
            frame.seekable,
        );
        self.perf.ticks.push(ticks as f64);
        if self.error.is_some() {
            return false;
        }
        let (renderer, feed) = self.render.as_mut().unwrap();
        let start = (!frame.seekable).then(Stamp::now);
        self.hook_poses.track(self.hooks.entities(), sim.world());
        self.hook_poses.sync(sim.world());
        let hook_time = if H::ENABLED {
            self.hook_clock.frame(
                frame,
                sim.world().presentation_generation(),
                G::paused(sim.args()),
            )
        } else {
            crate::HookTime::default()
        };
        if !self.perf.armed() {
            self.gpu_timing = None;
        }
        if feed.mouse_look() {
            feed.unshown_motion(sim.unshown_motion());
        }
        let mut input = feed.frame_pixels(sim.world(), sim.alpha(), (frame.width, frame.height));
        // Every pixel the renderer stores has alpha one: each clear of the
        // output stores one, opaque draws write one, and every blend composes
        // alpha OVER, which keeps a destination alpha of one at one (tests
        // `every_output_pixel_stores_alpha_one`). Hooks can write any alpha.
        self.output_opaque = !H::ENABLED;
        self.perf.lights(input.lights.len(), input.lights_dropped);
        input.headroom = frame.headroom;
        // Armed perf adds asynchronous pass timings and per-view culling counts.
        if self.perf.armed() {
            renderer.count_culled(true);
            if self.gpu_timing.is_none() {
                self.gpu_timing = crate::hooks::gpu_timing::GpuTiming::new(device, queue);
                if H::ENABLED && self.gpu_timing.is_some() {
                    renderer
                        .hook_metrics
                        .get_or_insert_with(Default::default)
                        .engine
                        .buffers += crate::hooks::gpu_timing::GpuTiming::BUFFERS;
                }
            }
            if H::ENABLED {
                renderer
                    .hook_metrics
                    .get_or_insert_with(Default::default)
                    .arm(false);
            }
            if let Some(timing) = &mut self.gpu_timing {
                timing.poll(device, queue);
                input.timestamps = timing.query();
            }
        }
        self.placed
            .frame(&input, exact_game::Vec2::new(frame.width, frame.height));
        #[cfg(not(target_arch = "wasm32"))]
        renderer
            .quads
            .children(&renderer.device, &renderer.queue, &self.placed);
        let direct_frames = renderer.direct_frames;
        self.perf.stats = match renderer.encode_hooked(
            encoder,
            target,
            frame.pixels(),
            &input,
            &mut self.hooks,
            Some(sim.world()),
            hook_time,
            &self.hook_poses,
        ) {
            Ok(stats) => stats,
            Err(error) => {
                self.error = Some(SurfaceError(error.to_string()));
                return false;
            }
        };
        self.hook_poses.track(self.hooks.entities(), sim.world());
        if !self.hooks.needs().contains(crate::Needs::PENDING)
            && self.hooks.error().is_none()
            && self.ready_work.is_none()
        {
            eprintln!(
                "game renderer first frame encoded: {}x{} pipelines={} direct={} f16={}",
                frame.pixels().0,
                frame.pixels().1,
                renderer.residency_work().pipeline_creations,
                self.perf.stats.direct_frames > direct_frames,
                renderer.pipelines.half
            );
            self.ready_work = Some(renderer.residency_work());
        }
        self.frame_ready =
            !self.hooks.needs().contains(crate::Needs::PENDING) && self.hooks.error().is_none();
        if let Some(start) = start {
            let ms = start.elapsed();
            self.perf.encode.push(ms);
            if let Some(trace) = &mut self.trace {
                trace.times[2] = ms;
            }
        }
        if let Some(trace) = &mut self.trace {
            trace.projection[..16]
                .copy_from_slice(&(input.proj * input.view).to_cols_array().map(f64::from));
            let (width, height) = frame.pixels();
            trace.projection[16] = f64::from(width);
            trace.projection[17] = f64::from(height);
            trace.frame(
                frame.now_ms,
                sim.alpha(),
                ticks,
                feed.trace_camera(sim.alpha()),
                trace.times,
            );
            trace.times = [0.; 3];
        }
        // Timed once submitted (`submitted`): the module submits the frame.
        self.encoded = Some(renderer.timed.get());
        if self.perf.armed() {
            self.perf.culled = renderer.culled();
            self.perf.culled_triangles = renderer.culled_triangles();
        }
        let wants = !G::paused(sim.args()) || ticks != 0;
        retain_primes(&mut self.render, &mut self.placed, false);
        self.dirty = false;
        wants
            || self.hooks.needs().contains(crate::Needs::ANIMATE)
            || self.hooks.needs().contains(crate::Needs::PENDING)
    }

    /// A display frame the module does not render: run the ticks due by
    /// `frame` and feed them, as the next `render_frame` would. At 256,000
    /// moving cubes on a Pixel 5 a tick and its upload take 8 ms, so a frame
    /// that waited for the GPU and then ran two missed its display frame too.
    /// Only for a plain live frame with nothing pending first: a frame to
    /// seed, assets, hooks.
    pub(super) fn advance_frame(&mut self, frame: &Frame) {
        if H::ENABLED
            || self.dirty
            || self.error.is_some()
            || frame.seekable
            || self.render.is_none()
        {
            return;
        }
        let Some(sim) = &mut self.sim else {
            return;
        };
        if !frame.now_ms.is_finite()
            || sim.is_loading()
            || (ASSETS && !sim.device_assets_ready())
            || self.generation != sim.world().presentation_generation()
        {
            return;
        }
        let due = sim.ticks_due(frame.now_ms, Clock::Live);
        if due == 0 {
            return;
        }
        sim.frame_period(frame.period_ms);
        sim.advance_with(
            frame.now_ms,
            Clock::Live,
            observer(
                &mut self.render,
                &mut self.placed,
                &mut self.perf,
                &mut self.trace,
                &mut self.error,
                &mut self.hook_poses,
                true,
                due,
            ),
        );
        self.presentation
            .sync(sim.world(), sim.generation(), !G::paused(sim.args()), false);
    }

    /// The module submitted the frame `render_frame` encoded: map its
    /// readbacks and start its pass timings.
    pub(super) fn frame_submitted(&mut self) {
        let Some(timed) = self.encoded.take() else {
            return;
        };
        let Some((renderer, _)) = &mut self.render else {
            return;
        };
        renderer.submitted();
        if let Some(timing) = &mut self.gpu_timing {
            timing.submitted(&renderer.queue, timed);
        }
    }
}
