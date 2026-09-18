use crate::{
    perf::{Perf, Stamp},
    Feed,
};
use exact_game::{Clock, Game, Sim, World};
use exact_gpu::{
    wgpu, AssetError, Frame, InputEvent, Lifecycle, Restore, Surface, SurfaceError, Value,
};

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
    /// Snapshot newly constructed definitions before a carry replaces dynamic state.
    fn before_restore(&mut self, _world: &World, _mode: Restore) {}
    /// Overlay fresh definitions only for a development carry.
    fn after_restore(&mut self, _world: &World, _mode: Restore) {}
    /// Called synchronously on key/pointer down, within the browser's gesture.
    fn unlock(&mut self) {}
}
impl Presentation for () {}

const RETIRED_BUDGET: u64 = 64 * 1024 * 1024;

/// One simulation and its lazily created GPU renderer, for an exact canvas.
/// Model pipelines prepare during delivery; primitive pipelines prepare at first render.
pub struct WorldSurface<G: Game, P: Presentation = (), const ASSETS: bool = false> {
    sim: Option<Sim<G>>,
    pending_restore: Option<(Vec<u8>, Restore)>,
    refusal: Option<SurfaceError>,
    presentation: P,
    render: Option<(crate::renderer::RendererWithAssets<ASSETS>, Feed)>,
    format: Option<wgpu::TextureFormat>,
    device: bool,
    perf: Perf,
    ready_work: Option<crate::world::assets::Work>,
    trace: Option<crate::trace::Trace>,
    error: Option<SurfaceError>,
    dirty: bool,
    assets_dirty: bool,
    model_digests: std::collections::BTreeMap<String, u64>,
    reported: bool,
    generation: u64,
    seekable: bool,
    audio_requested: bool,
    hidden: bool,
    interrupted: bool,
}
impl<G: Game, P: Presentation, const ASSETS: bool> Default for WorldSurface<G, P, ASSETS> {
    fn default() -> Self {
        Self {
            sim: None,
            pending_restore: None,
            refusal: None,
            presentation: P::default(),
            render: None,
            format: None,
            device: false,
            perf: Perf::default(),
            ready_work: None,
            trace: None,
            error: None,
            dirty: true,
            assets_dirty: false,
            model_digests: Default::default(),
            reported: false,
            generation: 0,
            seekable: true,
            audio_requested: false,
            hidden: false,
            interrupted: false,
        }
    }
}
impl<G: Game, P: Presentation, const ASSETS: bool> WorldSurface<G, P, ASSETS> {
    fn finish_restore(&mut self) {
        if let Some(sim) = self.sim.as_mut().filter(|s| !s.is_loading()) {
            if let Some((bytes, mode)) = self.pending_restore.take() {
                let definitions = (ASSETS && mode == Restore::Carry)
                    .then(|| exact_game::animation::Definitions::capture(sim.world()));
                self.presentation.before_restore(sim.world(), mode);
                if let Err(error) = sim.restore_bound(&bytes) {
                    self.refusal = Some(SurfaceError(format!("restore refused: {error}")));
                } else {
                    if let Some(definitions) = definitions {
                        definitions.apply(sim.world());
                    }
                    self.presentation.after_restore(sim.world(), mode);
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
fn observer<'a, const ASSETS: bool>(
    render: &'a mut Option<(crate::renderer::RendererWithAssets<ASSETS>, Feed)>,
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
impl<G: Game, P: Presentation, const ASSETS: bool> Surface for WorldSurface<G, P, ASSETS> {
    fn clock(&mut self, seekable: bool) {
        self.seekable = seekable;
        self.presentation.clock(seekable);
    }
    fn lifecycle(&mut self, event: Lifecycle) {
        match event {
            Lifecycle::Hidden => self.hidden = true,
            Lifecycle::Visible => self.hidden = false,
            Lifecycle::Interrupted => self.interrupted = true,
            Lifecycle::Resumed => self.interrupted = false,
            _ => return,
        }
        self.presentation.suspend(self.hidden || self.interrupted);
    }
    fn bind(&mut self, values: &[Value], at_ms: Option<f64>) -> Result<(), SurfaceError> {
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
        if !ASSETS {
            let sim = self.sim.as_ref().unwrap();
            if let Some((_, sprite)) = sim.world().query::<&exact_game::Sprite>().iter().next() {
                return Err(SurfaceError(format!(
                    "Sprite `{}` requires game.assets: true",
                    sprite.texture
                )));
            }
            let name = G::ASSETS.first().map(|n| (*n).to_owned()).or_else(|| {
                sim.world()
                    .query::<&exact_game::Mesh>()
                    .iter()
                    .find_map(|(_, mesh)| {
                        if let exact_game::Mesh::Asset(name) = mesh {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
            });
            if let Some(name) = name {
                self.sim = None;
                return Err(SurfaceError(format!(
                    "asset `{name}`: this module has no model support; declare game.assets"
                )));
            }
        }
        self.dirty = true;
        Ok(())
    }
    fn assets(&mut self) -> Vec<String> {
        let names = self.sim.as_mut().map_or_else(Vec::new, Sim::take_assets);
        if ASSETS {
            names
        } else {
            for name in names {
                self.sim
                    .as_mut()
                    .unwrap()
                    .asset_failed(&name, "module has no model support");
            }
            Vec::new()
        }
    }
    fn retired_assets(&mut self) -> Vec<String> {
        let retired = self
            .sim
            .as_mut()
            .map_or_else(Vec::new, Sim::take_retired_assets);
        if !retired.is_empty() {
            // A reference disappearing (including a restore) retires host flights,
            // not device residency. A later arrival must still compare its digest.
            for name in &retired {
                self.model_digests.remove(name);
            }
            if let Some((renderer, feed)) = &mut self.render {
                for name in &retired {
                    renderer.quads.retire_texture(name);
                    if let Some(model) = renderer.models.loaded.get_mut(name) {
                        model.active = false;
                    }
                }
                renderer.models.revision += 1;
                feed.reset();
            }
            self.assets_dirty = true;
            self.dirty = true;
        }
        retired
    }
    fn asset(&mut self, name: &str, bytes: Result<&[u8], AssetError>) {
        if ASSETS {
            if let Some(sim) = &mut self.sim {
                match bytes {
                    Ok(bytes) => {
                        if sim.asset(name, Some(bytes)).is_ok() && name.ends_with(".model") {
                            if let Some((_, model)) =
                                sim.presentation_models().find(|(n, _)| *n == name)
                            {
                                self.model_digests
                                    .insert(name.into(), crate::models::model_digest(model));
                            }
                        }
                    }
                    Err(AssetError::Missing) => sim.asset_failed(name, "missing file"),
                    Err(AssetError::Failed(reason)) => sim.asset_failed(name, &reason),
                }
            }
        }
        self.assets_dirty = true;
        self.finish_restore();
        self.dirty = true;
    }
    fn prepare_assets(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if !ASSETS {
            return;
        }
        if !self.assets_dirty && self.format == Some(format) {
            return;
        }
        let Some(sim) = &mut self.sim else { return };
        let textures = sim.take_textures();
        if self.render.is_none()
            && sim.presentation_models().next().is_none()
            && textures.is_empty()
        {
            self.assets_dirty = false;
            return;
        }
        if self.format != Some(format) {
            self.ready_work = None;
            self.render = Some((
                crate::renderer::RendererWithAssets::<ASSETS>::new(device, queue, format),
                Feed::default(),
            ));
            self.format = Some(format);
        }
        let (renderer, feed) = self.render.as_mut().unwrap();
        let live = sim.presentation_assets().map(str::to_owned).collect();
        let replaced = sim.presentation_models().any(|(name, _)| {
            self.model_digests
                .get(name)
                .is_some_and(|&digest| renderer.model_changed(name, digest))
        });
        if replaced || renderer.retired_bytes(&live) > RETIRED_BUDGET {
            renderer.compact_assets(format, &live);
            feed.reset();
        }
        let prepared: Vec<_> = sim
            .presentation_models()
            .filter(|(name, _)| self.model_digests.contains_key(*name))
            .map(|(name, model)| {
                (
                    name.to_owned(),
                    renderer
                        .prepare_model_digest(name, model, self.model_digests[name])
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
        self.sim.as_ref().and_then(|sim| sim.save().ok())
    }
    fn restore(&mut self, bytes: &[u8], mode: Restore) -> Result<(), String> {
        if self.sim.as_ref().is_some_and(Sim::is_loading) {
            self.pending_restore = Some((bytes.to_vec(), mode));
            return Ok(());
        }
        let sim = self.sim.as_mut().ok_or("world has not been bound")?;
        let definitions = (ASSETS && mode == Restore::Carry)
            .then(|| exact_game::animation::Definitions::capture(sim.world()));
        self.presentation.before_restore(sim.world(), mode);
        sim.restore_bound(bytes).map_err(|e| e.to_string())?;
        if let Some(definitions) = definitions {
            definitions.apply(sim.world());
        }
        self.presentation.after_restore(sim.world(), mode);
        self.dirty = true;
        self.error = None;
        self.reported = false;
        self.perf = Perf::default();
        Ok(())
    }
    fn device_ready(&mut self) {
        if let Some(sim) = &mut self.sim {
            sim.defer_assets(true);
            if !self.device {
                sim.invalidate_device_assets();
            }
        }
        self.device = true;
    }
    fn device_lost(&mut self) {
        self.device = false;
        self.render = None;
        self.ready_work = None;
        self.format = None;
        if let Some(sim) = &mut self.sim {
            sim.invalidate_device_assets();
        }
        self.assets_dirty = true;
        self.dirty = true;
    }
    fn take_error(&mut self) -> Option<SurfaceError> {
        if let Some(refusal) = self.refusal.take() {
            return Some(refusal);
        }
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
            return sim.assets_pending();
        }
        if ASSETS && !sim.device_assets_ready() {
            return sim.assets_pending();
        }
        sim.viewport(frame.width, frame.height);
        if self.format != Some(format) {
            self.ready_work = None;
            self.render = Some((
                crate::renderer::RendererWithAssets::<ASSETS>::new(device, queue, format),
                Feed::default(),
            ));
            self.format = Some(format);
            self.dirty = true;
        }
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
        let input = feed.frame_pixels(sim.world(), sim.alpha(), (frame.width, frame.height));
        self.perf.stats = renderer.draw_assets(target, frame.pixels(), &input);
        self.ready_work
            .get_or_insert_with(|| renderer.residency_work());
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
                perf: bool,
            }
            if let Ok(r) = exact_game::json::from_str::<PerfRequest>(request) {
                if r.perf_reset {
                    self.perf.reset();
                } else if r.perf {
                    self.perf.arm();
                }
            }
            reply.truncate(reply.len() - 2);
            reply.push_str(if self.device {
                ",\"device\":true"
            } else {
                ",\"device\":false"
            });
            self.perf.append(&mut reply);
            let work = self
                .render
                .as_ref()
                .map_or_else(Default::default, |(r, _)| r.residency_work());
            let mut reasons: Vec<String> = sim.asset_failures().map(str::to_owned).collect();
            if let Some(error) = &self.error {
                reasons.push(format!("render error: {}", error.0));
            }
            if !self.device {
                reasons.push("no device".into());
            }
            if sim.is_loading() {
                reasons.push("declared content pending".into());
            }
            if !sim.device_assets_ready() {
                reasons.push("device assets pending".into());
            }
            if self.ready_work.is_none() {
                reasons.push("first draw pending".into());
            }
            let ready = reasons.is_empty();
            reply.push_str(&format!(",\"ready\":{ready},\"readyReasons\":{},\"gpu\":{{\"beforeReady\":{},\"afterReady\":{},\"bufferScope\":\"model instances and skin buffers only; excludes vertex/index, primitive pages and slots\"}}",
                exact_game::json::to_string(&reasons).unwrap(),
                self.ready_work.unwrap_or(work).json(),
                self.ready_work.map_or_else(Default::default, |before| work.since(before)).json()));
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
        surface.bind(&[], None).unwrap();
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
            surface.lifecycle(Lifecycle::Interrupted);
            surface.lifecycle(Lifecycle::Visible);
            assert!(surface.presentation.suspended);
            surface.lifecycle(Lifecycle::Resumed);
            assert!(!surface.presentation.suspended);
            assert_eq!(surface.carry(), saved_after_input);
        }
        assert_ne!(saved, saved_after_input); // only the actual input changes saved state
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod residency_tests {
    use super::*;
    struct Fox;
    impl Game for Fox {
        type Args = ();
        const ID: &'static str = "residency-fox";
        const ASSETS: &'static [&'static str] = &["fox.model"];
        fn setup(w: &mut World, _: &()) {
            w.spawn((
                exact_game::Transform::default(),
                exact_game::Mesh::asset("fox.model"),
            ));
        }
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }

    struct Cosmetic;
    impl Game for Cosmetic {
        type Args = ();
        const ID: &'static str = "residency-cosmetic";
        fn setup(w: &mut World, _: &()) {
            w.spawn_named(
                "hero",
                (
                    exact_game::Transform::default(),
                    exact_game::Mesh::asset("hero.model"),
                ),
            );
            w.spawn((
                exact_game::Transform::at(2.8, 2., 3.8)
                    .looking_at(exact_game::Vec3::ZERO, exact_game::Vec3::Y),
                exact_game::Camera::default(),
            ));
        }
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    fn frame() -> Frame {
        Frame {
            width: 32.,
            height: 32.,
            scale: 1.,
            now_ms: 0.,
            seekable: true,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
        }
    }
    fn model() -> exact_game::asset::Model {
        exact_game::bin::from_slice(include_bytes!(
            "../../games/asset-fixture/assets/crate.model"
        ))
        .unwrap()
    }
    #[test]
    fn retired_model_stays_hidden_until_changed_dependency_closure_is_prepared() {
        let Ok(gpu) = exact_gpu::fixture::device() else {
            return;
        };
        let mut s = WorldSurface::<Cosmetic, (), true>::default();
        s.device_ready();
        s.bind(&[], None).unwrap();
        let mut model = model();
        let texture = include_bytes!("../../games/asset-fixture/assets/crate/0-srgb-straight.tex");
        s.assets();
        s.asset("hero.model", Ok(&exact_game::bin::to_vec(&model)));
        s.asset(&model.textures[0], Ok(texture));
        let (before, _) = exact_gpu::fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert!(!s.render.as_ref().unwrap().0.models.records.is_empty());
        *s.sim
            .as_ref()
            .unwrap()
            .world()
            .get_mut::<exact_game::Mesh>("hero")
            .unwrap() = exact_game::Mesh::asset("away.model");
        s.assets();
        s.retired_assets();
        *s.sim
            .as_ref()
            .unwrap()
            .world()
            .get_mut::<exact_game::Mesh>("hero")
            .unwrap() = exact_game::Mesh::asset("hero.model");
        s.assets();
        s.retired_assets();
        exact_gpu::fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert!(
            s.render.as_ref().unwrap().0.models.records.is_empty(),
            "retired geometry leaked before redelivery"
        );
        model.materials[0].base_color = [1., 0., 0., 1.];
        s.asset("hero.model", Ok(&exact_game::bin::to_vec(&model)));
        exact_gpu::fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert!(
            s.render.as_ref().unwrap().0.models.records.is_empty(),
            "model leaked before its texture arrived"
        );
        s.asset(&model.textures[0], Ok(texture));
        let (after, _) = exact_gpu::fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert!(
            before != after,
            "first drawable frame must use changed material bytes"
        );
        assert_eq!(
            s.render.as_ref().unwrap().0.models.materials.len(),
            model.materials.len(),
            "replacement reclaims old materials"
        );
    }

    #[test]
    fn twenty_unique_models_bound_retirement_and_hash_only_at_delivery() {
        let Ok(gpu) = exact_gpu::fixture::device() else {
            return;
        };
        let mut s = WorldSurface::<Cosmetic, (), true>::default();
        s.device_ready();
        s.bind(&[], None).unwrap();
        let mut model = model();
        let texture = exact_game::asset::TextureData {
            width: 1024,
            height: 1024,
            mips: (0..11)
                .map(|level| vec![255; (1024usize >> level).max(1).pow(2) * 4])
                .collect(),
            ..Default::default()
        };
        let bytes = exact_game::bin::to_vec(&texture);
        let hashes_before = crate::models::model_hash_count();
        for i in 0..20 {
            let name = format!("unique-{i}.model");
            model.textures[0] = format!("unique-{i}.tex");
            *s.sim
                .as_ref()
                .unwrap()
                .world()
                .get_mut::<exact_game::Mesh>("hero")
                .unwrap() = exact_game::Mesh::asset(&name);
            s.assets();
            s.retired_assets();
            s.asset(&name, Ok(&exact_game::bin::to_vec(&model)));
            s.asset(&model.textures[0], Ok(&bytes));
            s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
            let live = s
                .sim
                .as_ref()
                .unwrap()
                .presentation_assets()
                .map(str::to_owned)
                .collect();
            assert!(s.render.as_ref().unwrap().0.retired_bytes(&live) <= RETIRED_BUDGET);
            assert_eq!(crate::models::model_hash_count(), hashes_before + i + 1);
            // Dirty texture redelivery must never hash any resident model again.
            s.asset(&model.textures[0], Ok(&bytes));
            s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
            assert_eq!(crate::models::model_hash_count(), hashes_before + i + 1);
        }
        assert!(s.render.as_ref().unwrap().0.models.loaded.len() < 20);
        let json = s.agent(r#"{"op":"state"}"#).unwrap();
        assert!(json.contains(r#""modelSkinBufferReallocations":"#));
        assert!(!json.contains(r#""bufferReallocations":"#));
    }
    #[test]
    fn ready_reasons_name_declaration_and_render_failures() {
        let mut s = WorldSurface::<Fox, (), true>::default();
        s.device_ready();
        s.bind(&[], None).unwrap();
        s.asset("fox.model", Err(AssetError::Missing));
        let state = s.agent(r#"{"op":"state"}"#).unwrap();
        assert!(
            state
                .split("\"readyReasons\":")
                .nth(1)
                .unwrap()
                .split(']')
                .next()
                .unwrap()
                .contains("fox.model"),
            "{state}"
        );
        s.error = Some(SurfaceError("named render failure".into()));
        let state = s.agent(r#"{"op":"state"}"#).unwrap();
        assert!(
            state
                .split("\"readyReasons\":")
                .nth(1)
                .unwrap()
                .split(']')
                .next()
                .unwrap()
                .contains("named render failure"),
            "{state}"
        );
        assert!(state.contains("\"ready\":false"));
    }
    #[test]
    fn fox_restore_and_paranoid_save_keep_assets_pipelines_and_palette_capacity() {
        let Ok(gpu) = exact_gpu::fixture::device() else {
            return;
        };
        let mut surface = WorldSurface::<Fox, (), true>::default();
        surface.device_ready();
        surface.bind(&[], None).unwrap();
        surface.asset(
            "fox.model",
            Ok(include_bytes!(
                "../../games/skinned-fixture/assets/fox.model"
            )),
        );
        surface.asset(
            "fox/0-srgb-straight.tex",
            Ok(include_bytes!(
                "../../games/skinned-fixture/assets/fox/0-srgb-straight.tex"
            )),
        );
        let mut frame = Frame {
            width: 16.,
            height: 16.,
            scale: 1.,
            now_ms: 0.,
            seekable: true,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
        };
        exact_gpu::fixture::render(&gpu, &mut surface, &frame).unwrap();
        let before = surface.render.as_ref().unwrap().0.residency_work().json();
        let saved = surface.carry().unwrap();
        for mode in [Restore::Open, Restore::Carry] {
            surface.restore(&saved, mode).unwrap();
            exact_gpu::fixture::render(&gpu, &mut surface, &frame).unwrap();
            assert_eq!(
                before,
                surface.render.as_ref().unwrap().0.residency_work().json()
            );
            assert_eq!(surface.carry().unwrap(), saved);
        }
        let mut texture: exact_game::asset::TextureData = exact_game::bin::from_slice(
            include_bytes!("../../games/skinned-fixture/assets/fox/0-srgb-straight.tex"),
        )
        .unwrap();
        texture.mips[0][0] ^= 127;
        let changed = exact_game::bin::to_vec(&texture);
        surface.asset("fox/0-srgb-straight.tex", Ok(&changed));
        surface.restore(&saved, Restore::Carry).unwrap();
        exact_gpu::fixture::render(&gpu, &mut surface, &frame).unwrap();
        let work = surface.render.as_ref().unwrap().0.residency_work();
        let delta = work.since(surface.ready_work.unwrap());
        assert_eq!(delta.texture_uploads, 1);
        assert_eq!(delta.mesh_uploads, 0);
        assert_eq!(delta.pipeline_creations, 0);
        assert_eq!(delta.buffer_reallocations, 0);
        surface.asset("fox/0-srgb-straight.tex", Ok(&changed));
        exact_gpu::fixture::render(&gpu, &mut surface, &frame).unwrap();
        assert_eq!(
            work.json(),
            surface.render.as_ref().unwrap().0.residency_work().json()
        );
        let before = work.json();
        surface.sim = surface
            .sim
            .take()
            .map(|s| s.paranoid(exact_game::Paranoid::Save));
        for tick in 1..=4 {
            frame.now_ms = tick as f64 * 1000. / 60.;
            exact_gpu::fixture::render(&gpu, &mut surface, &frame).unwrap();
            assert_eq!(
                before,
                surface.render.as_ref().unwrap().0.residency_work().json()
            );
        }
        assert!(surface.render.as_ref().unwrap().0.models.skinning.is_some());
        let mut model = surface
            .sim
            .as_ref()
            .unwrap()
            .presentation_models()
            .next()
            .unwrap()
            .1
            .clone();
        let old = &surface.render.as_ref().unwrap().0;
        let sizes = (old.meshes.len(), old.models.materials.len());
        let skins: Vec<_> = old.models.loaded["fox.model"]
            .nodes
            .iter()
            .map(|n| n.3)
            .collect();
        for _ in 0..3 {
            model.meshes[0].positions[0] += 0.01;
            surface.asset("fox.model", Ok(&exact_game::bin::to_vec(&model)));
            exact_gpu::fixture::render(&gpu, &mut surface, &frame).unwrap();
            let renderer = &surface.render.as_ref().unwrap().0;
            assert_eq!(
                (renderer.meshes.len(), renderer.models.materials.len()),
                sizes
            );
            assert_eq!(
                renderer.models.loaded["fox.model"]
                    .nodes
                    .iter()
                    .map(|n| n.3)
                    .collect::<Vec<_>>(),
                skins,
                "same-name replacement reclaims skin templates too"
            );
        }
    }
}
