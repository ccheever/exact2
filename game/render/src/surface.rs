use crate::{
    perf::{Perf, Stamp},
    Feed, Renderer,
};
use exact_game::{Clock, Game, Sim, World};
use exact_gpu::{wgpu, Frame, InputEvent, Lifecycle, Surface, SurfaceError, Value};

/// Optional presentation executor. The default `()` links no executor code.
/// Implementations must use a discarding output whenever `seekable` is true.
pub trait Presentation: Default {
    /// This executor needs the host's audio session when using live time.
    fn wants_audio(&self) -> bool {
        false
    }
    /// Clock ownership reaches the executor before any input.
    fn clock(&mut self, _seekable: bool) {}
    /// Aggregate visibility and interruption state, outside simulation state.
    fn suspend(&mut self, _suspended: bool) {}
    /// Called once after the simulation advances, including zero-size frames.
    fn sync(&mut self, _world: &World, _generation: u64, _playing: bool, _seekable: bool) {}
    /// Called synchronously on key/pointer down, within the browser's gesture.
    fn unlock(&mut self) {}
}
impl Presentation for () {}

/// One simulation and its lazily created GPU renderer, for an exact canvas.
/// Model pipelines prepare during delivery; primitive pipelines prepare at first render.
pub struct WorldSurface<G: Game, P: Presentation = ()> {
    sim: Option<Sim<G>>,
    pending_restore: Option<Vec<u8>>,
    presentation: P,
    render: Option<(Renderer, Feed)>,
    format: Option<wgpu::TextureFormat>,
    device: bool,
    perf: Perf,
    trace: Option<crate::trace::Trace>,
    error: Option<SurfaceError>,
    dirty: bool,
    assets_dirty: bool,
    reported: bool,
    generation: u64,
    seekable: bool,
    audio_requested: bool,
    hidden: bool,
    interrupted: bool,
}
impl<G: Game, P: Presentation> Default for WorldSurface<G, P> {
    fn default() -> Self {
        Self {
            sim: None,
            pending_restore: None,
            presentation: P::default(),
            render: None,
            format: None,
            device: false,
            perf: Perf::default(),
            trace: None,
            error: None,
            dirty: true,
            assets_dirty: false,
            reported: false,
            generation: 0,
            seekable: true,
            audio_requested: false,
            hidden: false,
            interrupted: false,
        }
    }
}
impl<G: Game, P: Presentation> WorldSurface<G, P> {
    fn finish_restore(&mut self) {
        if let Some(sim) = self.sim.as_mut().filter(|s| !s.is_loading()) {
            if let Some(bytes) = self.pending_restore.take() {
                if let Err(error) = sim.restore_bound(&bytes) {
                    self.error = Some(SurfaceError(error.to_string()));
                    self.reported = false;
                }
            }
        }
    }
    /// Sticky capacity refusal; also drained through the GPU Surface error seam.
    pub fn error(&self) -> Option<&SurfaceError> {
        self.error.as_ref()
    }
    /// Read the simulation after a successful bind.
    pub fn sim(&self) -> Option<&Sim<G>> {
        self.sim.as_ref()
    }
}
fn observer<'a>(
    render: &'a mut Option<(Renderer, Feed)>,
    perf: &'a mut Perf,
    trace: &'a mut Option<crate::trace::Trace>,
    error: &'a mut Option<SurfaceError>,
    measure: bool,
    ticks: u32,
) -> impl FnMut(&World, u32) + 'a {
    let mut start = (measure && (1..=240).contains(&ticks)).then(Stamp::now);
    move |world, left| {
        if let Some(start) = &start {
            if left < 240 {
                let ms = start.elapsed();
                perf.tick.push(ms);
                if let Some(trace) = trace {
                    trace.times[0] += ms;
                }
            }
        }
        if left < 2 && error.is_none() {
            if let Some((renderer, feed)) = render {
                if let Some(trace) = trace {
                    trace.feed(world);
                }
                let upload = measure.then(Stamp::now);
                if let Err(e) = feed.feed(world, renderer) {
                    *error = Some(SurfaceError(e.to_string()));
                }
                if let Some(upload) = upload {
                    let ms = upload.elapsed();
                    perf.feed.push(ms);
                    if let Some(trace) = trace {
                        trace.times[1] += ms;
                    }
                }
            }
        }
        start = (measure && left > 0 && left <= 240).then(Stamp::now);
    }
}
impl<G: Game, P: Presentation> Surface for WorldSurface<G, P> {
    fn clock(&mut self, seekable: bool) {
        self.seekable = seekable;
        self.presentation.clock(seekable);
    }
    fn lifecycle(&mut self, event: Lifecycle) {
        match event {
            Lifecycle::Hidden => self.hidden = true,
            Lifecycle::Visible => self.hidden = false,
            Lifecycle::AudioInterrupted => self.interrupted = true,
            Lifecycle::AudioResumed => self.interrupted = false,
            _ => return,
        }
        self.presentation.suspend(self.hidden || self.interrupted);
    }
    fn bind(&mut self, values: &[Value]) -> Result<(), SurfaceError> {
        self.bind_at(values, None)
    }
    fn bind_at(&mut self, values: &[Value], at_ms: Option<f64>) -> Result<(), SurfaceError> {
        if at_ms.is_some_and(|at| !at.is_finite()) {
            return Err(SurfaceError("bind clock must be finite".into()));
        }
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        if let Some(sim) = &mut self.sim {
            let generation = sim.generation();
            sim.bind_with(
                values,
                at_ms,
                observer(
                    &mut self.render,
                    &mut self.perf,
                    &mut self.trace,
                    &mut self.error,
                    false,
                    0,
                ),
            )
            .map_err(SurfaceError)?;
            if generation != sim.generation() {
                if let Some((_, feed)) = &mut self.render {
                    feed.reset();
                }
                self.perf = Perf::default();
            }
        } else {
            let mut sim = Sim::from_values(values).map_err(SurfaceError)?;
            sim.defer_assets(self.device);
            if let Some(at) = at_ms {
                sim.advance(at, Clock::Seekable);
            }
            self.sim = Some(sim);
        }
        self.dirty = true;
        Ok(())
    }
    fn assets(&mut self) -> Vec<String> {
        self.sim.as_mut().map_or_else(Vec::new, Sim::take_assets)
    }
    fn asset(&mut self, name: &str, bytes: Option<&[u8]>) {
        if let Some(sim) = &mut self.sim {
            let _ = sim.asset(name, bytes);
        }
        self.assets_dirty = true;
        self.finish_restore();
        self.dirty = true;
    }
    fn asset_failed(&mut self, name: &str, reason: &str) {
        if let Some(sim) = &mut self.sim {
            sim.asset_failed(name, reason);
        }
        self.dirty = true;
    }
    fn prepare_assets(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if !self.assets_dirty && self.format == Some(format) {
            return;
        }
        let Some(sim) = &mut self.sim else { return };
        let textures = sim.take_textures();
        if sim.presentation_models().next().is_none() && textures.is_empty() {
            return;
        }
        if self.format != Some(format) {
            self.render = Some((Renderer::new(device, queue, format), Feed::default()));
            self.format = Some(format);
        }
        let (renderer, _) = self.render.as_mut().unwrap();
        let prepared: Vec<_> = sim
            .presentation_models()
            .map(|(name, model)| {
                (
                    name.to_owned(),
                    renderer
                        .prepare_model(name, model)
                        .map_err(|e| e.to_string()),
                )
            })
            .collect();
        for (name, result) in prepared {
            sim.asset_prepared(&name, result);
        }
        for (name, texture) in textures {
            let result = renderer
                .add_texture(&name, &texture)
                .map_err(|e| e.to_string());
            sim.asset_prepared(&name, result);
        }
        self.finish_restore();
        self.assets_dirty = false;
        self.dirty = true;
    }
    fn carry(&mut self) -> Option<Vec<u8>> {
        self.pending_restore
            .clone()
            .or_else(|| self.sim.as_ref().filter(|s| !s.is_loading()).map(Sim::save))
    }
    fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.sim.as_ref().is_some_and(Sim::is_loading) {
            self.pending_restore = Some(bytes.to_vec());
            return Ok(());
        }
        self.sim
            .as_mut()
            .ok_or("world has not been bound")?
            .restore_bound(bytes)
            .map_err(|e| e.to_string())?;
        self.dirty = true;
        self.error = None;
        self.reported = false;
        self.perf = Perf::default();
        Ok(())
    }
    fn device_ready(&mut self) {
        self.device = true;
        if let Some(sim) = &mut self.sim {
            sim.defer_assets(true);
        }
    }
    fn device_lost(&mut self) {
        self.device = false;
        self.render = None;
        self.format = None;
        self.dirty = true;
    }
    fn take_error(&mut self) -> Option<SurfaceError> {
        if self.reported {
            return None;
        }
        self.reported = self.error.is_some();
        self.error.clone()
    }
    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
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
                        &mut self.perf,
                        &mut self.trace,
                        &mut self.error,
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
            return true;
        }
        sim.viewport(frame.width, frame.height);
        if self.format != Some(format) {
            self.render = Some((Renderer::new(device, queue, format), Feed::default()));
            self.format = Some(format);
            self.dirty = true;
        }
        // Seed setup/current state before running ticks; no origin streak on frame one.
        if self.dirty {
            if let Some(trace) = &mut self.trace {
                trace.feed(sim.world());
            }
            let (renderer, feed) = self.render.as_mut().unwrap();
            if let Err(e) = feed.feed(sim.world(), renderer) {
                self.error = Some(SurfaceError(e.to_string()));
                return false;
            }
        }
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
                &mut self.perf,
                &mut self.trace,
                &mut self.error,
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
        let input = feed.frame(sim.world(), sim.alpha(), frame.width / frame.height);
        self.perf.stats = renderer.draw(target, frame.pixels(), &input);
        if let Some(start) = start {
            let ms = start.elapsed();
            self.perf.encode.push(ms);
            if let Some(trace) = &mut self.trace {
                trace.times[2] = ms;
            }
        }
        if let Some(trace) = &mut self.trace {
            trace.frame(
                frame.now_ms,
                sim.alpha(),
                ticks,
                feed.trace_camera(sim.alpha()),
                trace.times,
            );
            trace.times = [0.; 3];
        }
        let wants = !G::paused(sim.args()) || ticks != 0;
        self.dirty = false;
        wants
    }
    fn wants_input(&self) -> bool {
        true
    }
    fn input(&mut self, event: &InputEvent) {
        self.reported = false;
        use exact_game::{InputEvent as E, PointerPhase as P};
        use exact_gpu::PointerPhase as Q;
        let Some(sim) = &mut self.sim else {
            return;
        };
        if matches!(
            event,
            InputEvent::Key { down: true, .. } | InputEvent::Pointer { phase: Q::Down, .. }
        ) {
            self.presentation.unlock();
        }
        let e = match event {
            InputEvent::Key {
                code, down, at_ms, ..
            } => E::Key {
                code: code.clone(),
                down: *down,
                at_ms: *at_ms,
            },
            InputEvent::Pointer {
                id,
                phase,
                x,
                y,
                at_ms,
                ..
            } => E::Pointer {
                id: u64::from(*id),
                phase: match phase {
                    Q::Down => P::Down,
                    Q::Move => P::Move,
                    Q::Up => P::Up,
                    Q::Cancel => P::Cancel,
                },
                x: *x,
                y: *y,
                at_ms: *at_ms,
            },
            InputEvent::Wheel { dx, dy, at_ms, .. } => E::Wheel {
                dx: *dx,
                dy: *dy,
                at_ms: *at_ms,
            },
            InputEvent::Blur { at_ms } => E::Blur { at_ms: *at_ms },
        };
        sim.input(e);
    }
    fn published(&mut self) -> Option<String> {
        self.sim.as_mut().and_then(Sim::take_published)
    }
    fn messages(&mut self) -> Vec<String> {
        let mut messages = self.sim.as_mut().map_or_else(Vec::new, Sim::take_messages);
        if !self.seekable && !self.audio_requested && self.presentation.wants_audio() {
            messages.push("exact:audio".into());
            self.audio_requested = true;
        }
        messages
    }
    fn agent(&mut self, request: &str) -> Option<String> {
        self.reported = false;
        let sim = self.sim.as_mut()?;
        match crate::trace::request(request) {
            Ok(Some(crate::trace::Request::Arm { entity, frames })) => {
                return Some(
                    match crate::trace::Trace::new(sim.world(), &entity, frames) {
                        Ok(trace) => {
                            self.trace = Some(trace);
                            "{\"trace\":\"armed\"}".into()
                        }
                        Err(e) => {
                            format!("{{\"error\":{}}}", exact_game::json::to_string(&e).unwrap())
                        }
                    },
                );
            }
            Ok(Some(crate::trace::Request::Read)) => {
                return Some(
                    self.trace
                        .take()
                        .map_or_else(|| "{\"error\":\"trace is not armed\"}".into(), |t| t.read()),
                );
            }
            Ok(Some(crate::trace::Request::Stop)) => {
                self.trace = None;
                return Some("{\"trace\":\"stopped\"}".into());
            }
            Err(e) => {
                return Some(format!(
                    "{{\"error\":{}}}",
                    exact_game::json::to_string(&e.to_string()).unwrap()
                ))
            }
            Ok(None) => {}
        }
        let mut reply = sim.agent_with(
            request,
            observer(
                &mut self.render,
                &mut self.perf,
                &mut self.trace,
                &mut self.error,
                false,
                0,
            ),
        );
        // Engine world-state replies have this fixed suffix. Parse the reply using
        // its own Data decoder to distinguish state from tree/error/entity replies.
        let state = world_state(&reply);
        if state {
            #[derive(Default, exact_game::Data)]
            struct PerfRequest {
                perf_reset: bool,
            }
            if exact_game::json::from_str::<PerfRequest>(request).is_ok_and(|r| r.perf_reset) {
                self.perf.reset();
            }
            reply.truncate(reply.len() - 2);
            reply.push_str(if self.device {
                ",\"device\":true"
            } else {
                ",\"device\":false"
            });
            self.perf.append(&mut reply);
            reply.push_str("}}");
        }
        if let Some(error) = &self.error {
            reply.pop();
            reply.push_str(",\"renderError\":");
            reply.push_str(&exact_game::json::to_string(&error.0).unwrap());
            reply.push('}');
        }
        Some(reply)
    }
}
fn world_state(reply: &str) -> bool {
    use exact_game::Reader;
    let mut reader = exact_game::json::Decoder::new(reply);
    let parse = || -> Result<bool, exact_game::DataError> {
        reader.begin_struct()?;
        while let Some(field) = reader.field()? {
            if field == "world" {
                reader.begin_struct()?;
                while let Some(field) = reader.field()? {
                    if field == "hash" {
                        return Ok(true);
                    }
                    reader.skip()?;
                }
            } else {
                reader.skip()?;
            }
        }
        Ok(false)
    };
    // The input is produced by Sim, so malformed replies are never spliced.
    let mut parse = parse;
    parse().unwrap_or(false)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "surface_tests.rs"]
mod tests;

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[derive(Default)]
    struct Hook {
        seekable: bool,
        suspended: bool,
        unlocked: bool,
    }
    impl Presentation for Hook {
        fn wants_audio(&self) -> bool {
            true
        }
        fn clock(&mut self, seekable: bool) {
            self.seekable = seekable;
        }
        fn suspend(&mut self, suspended: bool) {
            self.suspended = suspended;
        }
        fn unlock(&mut self) {
            self.unlocked = !self.seekable && !self.suspended;
        }
    }
    struct GameTest;
    impl Game for GameTest {
        type Args = ();
        const NAME: &'static str = "lifecycle";
        const ID: &'static str = "test.lifecycle";
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    #[test]
    fn lifecycle_only_changes_presentation_and_clock_precedes_first_input() {
        let mut surface = WorldSurface::<GameTest, Hook>::default();
        surface.clock(false);
        surface.bind(&[]).unwrap();
        assert_eq!(surface.messages(), ["exact:audio"]);
        assert!(surface.messages().is_empty());
        let saved = surface.carry();
        surface.input(&InputEvent::Key {
            code: "KeyW".into(),
            key: "w".into(),
            down: true,
            repeat: false,
            at_ms: 0.,
        });
        assert!(surface.presentation.unlocked);
        let saved_after_input = surface.carry();
        for seekable in [false, true] {
            surface.clock(seekable);
            surface.lifecycle(Lifecycle::Hidden);
            surface.lifecycle(Lifecycle::AudioInterrupted);
            surface.lifecycle(Lifecycle::Visible);
            assert!(surface.presentation.suspended);
            surface.lifecycle(Lifecycle::AudioResumed);
            assert!(!surface.presentation.suspended);
            assert_eq!(surface.carry(), saved_after_input);
        }
        assert_ne!(saved, saved_after_input); // only the actual input changes saved state
    }
}
