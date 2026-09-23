mod args;
mod render;
mod textures;

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
    fn after_restore(&mut self, _world: &mut World, _mode: Restore) {}
    /// Specialized entity inspection supplied only by the linked executor.
    fn inspect(_world: &World, _entity: exact_game::Entity, pose: bool) -> Result<String, String> {
        if pose {
            Err("pose inspection requires game.assets: true".into())
        } else {
            Ok(String::new())
        }
    }
    /// Called synchronously on key/pointer down, within the browser's gesture.
    fn unlock(&mut self) {}
}
impl Presentation for () {}

use crate::renderer::RETIRED_BUDGET;

/// One simulation and its lazily created GPU renderer, for an exact canvas.
/// Model pipelines prepare during delivery; primitive pipelines prepare at first render.
pub struct WorldSurface<
    G: Game,
    P: Presentation = (),
    const ASSETS: bool = false,
    H: crate::Hooks = (),
> {
    sim: Option<Sim<G>>,
    pending_restore: Option<(Vec<u8>, Restore)>,
    placed: crate::placed::Placements,
    refusal: Option<SurfaceError>,
    presentation: P,
    hooks: H,
    hook_clock: crate::hooks::HookClock,
    hook_poses: crate::hooks::Poses,
    hook_gpu_timing: Option<crate::hooks::gpu_timing::GpuTiming>,
    render: Option<(crate::renderer::RendererWithAssets<ASSETS>, Feed)>,
    format: Option<wgpu::TextureFormat>,
    payloads: textures::Payloads,
    device: bool,
    device_assets_invalidated: bool,
    storage_limit: u32,
    perf: Perf,
    ready_work: Option<crate::world::assets::Work>,
    trace: Option<crate::trace::Trace>,
    error: Option<SurfaceError>,
    dirty: bool,
    assets_dirty: bool,
    asset_check: Option<(exact_game::WorldId, u64, u64)>,
    reported: bool,
    generation: u64,
    seekable: bool,
    audio_requested: bool,
    hidden: bool,
    interrupted: bool,
}
impl<G: Game, P: Presentation, const ASSETS: bool, H: crate::Hooks> Default
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
            hook_gpu_timing: None,
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
            assets_dirty: false,
            asset_check: None,
            reported: false,
            generation: 0,
            seekable: true,
            audio_requested: false,
            hidden: false,
            interrupted: false,
        }
    }
}
impl<G: Game, P: Presentation, const ASSETS: bool, H: crate::Hooks> WorldSurface<G, P, ASSETS, H> {
    fn check_primitive_assets(&mut self) -> Result<(), SurfaceError> {
        if ASSETS {
            return Ok(());
        }
        let Some(sim) = &self.sim else { return Ok(()) };
        let world = sim.world();
        let revision = (
            world.id(),
            world.revision::<exact_game::Mesh>(),
            world.revision::<exact_game::Sprite>(),
        );
        if self.asset_check.as_ref() == Some(&revision) {
            return Ok(());
        }
        if let Some((_, sprite)) = world.query::<&exact_game::Sprite>().iter().next() {
            return Err(SurfaceError(format!(
                "Sprite `{}` requires game.assets: true",
                sprite.texture
            )));
        }
        let missing = |name: &str| {
            SurfaceError(format!(
                "asset `{name}`: this module has no model support; declare game.assets"
            ))
        };
        if let Some(name) = G::ASSETS
            .iter()
            .copied()
            .find(|name| Some(*name) != G::LEVEL.map(|level| level.name))
        {
            return Err(missing(name));
        }
        for (_, mesh) in world.query::<&exact_game::Mesh>().iter() {
            if let exact_game::Mesh::Asset(name) = mesh {
                return Err(missing(name));
            }
        }
        self.asset_check = Some(revision);
        Ok(())
    }
    fn storage_fits(&mut self, device: &wgpu::Device) -> bool {
        if self.storage_limit == 0 {
            self.storage_limit = device.limits().max_storage_buffers_per_shader_stage;
        }
        let needed = if ASSETS {
            crate::STORAGE_BINDINGS
        } else {
            crate::SCENE_STORAGE_BINDINGS
        };
        if self.storage_limit < needed {
            self.error = Some(SurfaceError(format!(
                "renderer needs {needed} vertex storage buffers; device grants {}",
                self.storage_limit
            )));
            return false;
        }
        true
    }
    fn commit_restore(&mut self, bytes: &[u8], mode: Restore) -> Result<(), String> {
        let sim = self.sim.as_mut().ok_or("world has not been bound")?;
        self.presentation.before_restore(sim.world(), mode);
        sim.restore_bound(bytes).map_err(|e| e.to_string())?;
        self.presentation.after_restore(sim.world_mut(), mode);
        self.placed
            .attachments
            .diagnostics
            .borrow_mut()
            .restored(sim.world());
        self.dirty = true;
        self.error = None;
        self.reported = false;
        self.perf = Perf::default();
        Ok(())
    }
    fn finish_restore(&mut self) {
        if self.sim.as_ref().is_some_and(|s| !s.is_loading()) {
            if let Some((bytes, mode)) = self.pending_restore.take() {
                if let Err(error) = self.commit_restore(&bytes, mode) {
                    self.refusal = Some(SurfaceError(format!(
                        "restore refused: {}",
                        error.strip_prefix("restore refused: ").unwrap_or(&error)
                    )));
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
    /// Read presentation diagnostics owned by this canvas's render hooks.
    pub fn render_hooks(&self) -> &H {
        &self.hooks
    }
}
#[allow(clippy::too_many_arguments)]
fn observer<'a, const ASSETS: bool>(
    render: &'a mut Option<(crate::renderer::RendererWithAssets<ASSETS>, Feed)>,
    placed: &'a mut crate::placed::Placements,
    perf: &'a mut Perf,
    trace: &'a mut Option<crate::trace::Trace>,
    error: &'a mut Option<SurfaceError>,
    hook_poses: &'a mut crate::hooks::Poses,
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
            hook_poses.sync(world);
            if let Err(e) = if ASSETS {
                placed.feed(world)
            } else {
                placed.feed_primitive(world)
            } {
                *error = Some(SurfaceError(e.to_string()));
            }
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
impl<G: Game, P: Presentation, const ASSETS: bool, H: crate::Hooks> Surface
    for WorldSurface<G, P, ASSETS, H>
{
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
        self.hook_clock.reset();
    }
    fn arguments(&self) -> Vec<(&'static str, Value)> {
        self.surface_arguments()
    }
    fn bind(&mut self, values: &[Value], at_ms: Option<f64>) -> Result<(), SurfaceError> {
        self.bind_arguments(values, at_ms)
    }
    fn assets(&mut self) -> exact_gpu::AssetChanges {
        if !ASSETS {
            if self.error.is_none() {
                self.error = self.check_primitive_assets().err();
            }
            if self.error.is_some() || G::LEVEL.is_none() {
                return exact_gpu::AssetChanges::default();
            }
        }
        let requests = self.sim.as_mut().map_or_else(Vec::new, Sim::take_assets);
        let retired = self
            .sim
            .as_mut()
            .map_or_else(Vec::new, Sim::take_retired_assets);
        if ASSETS && !retired.is_empty() {
            // A reference disappearing (including a restore) retires host flights,
            // not device residency. A later arrival must still compare its digest.
            for name in &retired {
                self.placed.attachments.model_digests.remove(name);
            }
            if let Some((renderer, _feed)) = &mut self.render {
                for name in &retired {
                    renderer.retire_texture(name);
                    if let Some(model) = renderer.models.loaded.get_mut(name) {
                        model.active = false;
                    }
                }
                renderer.models.revision += 1;
            }
            self.assets_dirty = true;
            self.dirty = true;
        }
        if !ASSETS {
            return exact_gpu::AssetChanges { requests, retired };
        }
        exact_gpu::AssetChanges {
            retired: self.payloads.retire(retired),
            requests: self.payloads.request(requests),
        }
    }
    fn asset(&mut self, file: &str, bytes: Result<&[u8], AssetError>) {
        if ASSETS || G::LEVEL.is_some() {
            let name = if ASSETS {
                self.payloads.authored(file).to_owned()
            } else {
                file.to_owned()
            };
            let name = name.as_str();
            if let Some(sim) = &mut self.sim {
                match bytes {
                    Ok(bytes) => {
                        let accepted = sim
                            .deliver_asset(
                                name,
                                exact_game::asset::Content::decode::<ASSETS>(name, bytes),
                            )
                            .is_ok();
                        if ASSETS && accepted && name.ends_with(".model") {
                            if let Some((_, model)) =
                                sim.presentation_models().find(|(n, _)| *n == name)
                            {
                                let digest = crate::models::model_digest(model);
                                self.placed
                                    .attachments
                                    .model_digests
                                    .insert(name.into(), digest);
                            }
                        }
                    }
                    // A game without a bake's family payloads still has its RGBA8 file.
                    Err(AssetError::Missing) if ASSETS && self.payloads.missing(file) => {}
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
        if !self.storage_fits(device) {
            return;
        }
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
            self.hook_gpu_timing = None;
            self.hook_clock.reset();
            self.hooks.device_lost();
            self.ready_work = None;
            self.render = Some((
                crate::renderer::RendererWithAssets::<ASSETS>::new(device, queue, format),
                {
                    let mut feed = Feed::default();
                    feed.share_attachment_diagnostics(self.placed.attachments.diagnostics.clone());
                    feed
                },
            ));
            self.format = Some(format);
        }
        let (renderer, _feed) = self.render.as_mut().unwrap();
        let live = sim.presentation_assets().map(str::to_owned).collect();
        // Accept deliveries before the sole compaction: live identical residents
        // may still be marked retired until preparation reactivates them.
        let prepared: Vec<_> = sim
            .presentation_models()
            .map(|(name, model)| {
                let digest = *self
                    .placed
                    .attachments
                    .model_digests
                    .entry(name.to_owned())
                    .or_insert_with(|| crate::models::model_digest(model));
                (
                    name.to_owned(),
                    renderer
                        .prepare_model_digest(name, model, digest)
                        .map_err(|e| e.to_string()),
                )
            })
            .collect();
        // Resident geometry is independent of draw readiness: a texture still in
        // flight hides the model but must not evict this pass's accepted bytes.
        let touched = prepared
            .iter()
            .filter(|(_, result)| result.is_ok())
            .map(|(name, _)| name.clone())
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
        // Closure acceptance can reactivate an identical resident model without
        // uploading it. Publish that activity before compaction considers it.
        for (name, model) in &mut renderer.models.loaded {
            let active = sim.model_prepared(name);
            if model.active != active {
                model.active = active;
                renderer.models.revision += 1;
            }
        }
        if renderer.retired_bytes(&live) > RETIRED_BUDGET {
            renderer.compact_assets(&live, &touched);
        }
        self.finish_restore();
        self.assets_dirty = false;
        self.dirty = true;
    }
    fn carry(&mut self) -> Result<Option<Vec<u8>>, SurfaceError> {
        self.sim
            .as_ref()
            .map(|sim| sim.save().map_err(|e| SurfaceError(e.to_string())))
            .transpose()
    }
    fn restore(&mut self, bytes: &[u8], mode: Restore) -> Result<(), String> {
        if self.sim.as_ref().is_some_and(Sim::is_loading) {
            self.pending_restore = Some((bytes.to_vec(), mode));
            return Ok(());
        }
        self.commit_restore(bytes, mode)
    }

    fn preparing(&self) -> bool {
        self.hooks.needs().contains(crate::Needs::PENDING)
    }
    fn device_ready(&mut self, features: wgpu::Features) {
        if ASSETS {
            let family_changed = self.payloads.choose(features);
            if let Some(sim) = &mut self.sim {
                sim.defer_assets(true);
                // Loss already invalidated residency. Bytes delivered during adapter
                // backoff belong to the replacement and must survive its attachment,
                // unless the replacement fetches another texture family.
                if family_changed {
                    sim.take_textures();
                }
                if family_changed || (!self.device && !self.device_assets_invalidated) {
                    sim.invalidate_device_assets();
                }
            }
        }
        self.device_assets_invalidated = false;
        self.device = true;
    }
    fn device_lost(&mut self) {
        self.hooks.device_lost();
        self.hook_clock.reset();
        self.hook_gpu_timing = None;
        for child in &mut self.placed.children {
            child.texture = None;
        }
        self.device = false;
        self.render = None;
        self.ready_work = None;
        self.format = None;
        self.storage_limit = 0;
        if ASSETS && !self.device_assets_invalidated {
            if let Some(sim) = &mut self.sim {
                sim.invalidate_device_assets();
                self.device_assets_invalidated = true;
            }
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
        self.render_frame(frame, device, queue, target, format)
    }
    fn children_mode(&self) -> exact_gpu::ChildrenMode {
        if self.sim.as_ref().is_some_and(|sim| {
            sim.world()
                .query::<&exact_game::Placed>()
                .iter()
                .next()
                .is_some()
        }) {
            exact_gpu::ChildrenMode::Each
        } else {
            exact_gpu::ChildrenMode::Overlay
        }
    }
    fn child(
        &mut self,
        index: usize,
        name: &str,
        texture: Option<&wgpu::TextureView>,
        frame: [f32; 4],
    ) {
        self.placed.child(index, name, texture, frame);
        self.dirty = true;
    }
    fn placement(&self, index: usize) -> Option<exact_gpu::Placement> {
        self.placed.placement(index)
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
            InputEvent::Key { down: true, .. }
                | InputEvent::Pointer { phase: Q::Down, .. }
                | InputEvent::Control { phase: Q::Down, .. }
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
            InputEvent::Control {
                name,
                id,
                phase,
                x,
                y,
                at_ms,
            } => E::Control {
                name: name.clone(),
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
        if let Err(error) = sim.validate_input(&e) {
            self.refusal = Some(SurfaceError(error));
            return;
        }
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
        let mut reply = sim.agent_with_inspector(
            request,
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
            P::inspect,
        );
        if H::ENABLED {
            #[derive(Default, exact_game::Data)]
            struct Operation {
                op: String,
            }
            if exact_game::json::from_str::<Operation>(request).is_ok_and(|q| q.op == "tree")
                && reply.ends_with('}')
            {
                reply.pop();
                reply.push_str(&format!(
                    ",\"renderHooks\":{}}}",
                    crate::hooks::metrics::stages(self.hooks.needs())
                ));
            }
        }
        if self.render.is_none() {
            if let Err(error) = if ASSETS {
                self.placed.feed(sim.world())
            } else {
                self.placed.feed_primitive(sim.world())
            } {
                self.error = Some(SurfaceError(error.to_string()));
            }
            #[derive(Default, exact_game::Data)]
            struct Size {
                width: f32,
                height: f32,
            }
            if let Ok(size) = exact_game::json::from_str::<Size>(request) {
                if self.error.is_none() && size.width > 0. && size.height > 0. {
                    self.placed
                        .headless(exact_game::Vec2::new(size.width, size.height), sim.alpha());
                }
            }
        }
        self.placed.status(sim.world(), request, &mut reply);
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
                    if let Some(timing) = &mut self.hook_gpu_timing {
                        timing.reset();
                    }
                } else if r.perf {
                    self.perf.arm();
                }
                if r.perf || r.perf_reset {
                    if let Some(metrics) = self
                        .render
                        .as_mut()
                        .and_then(|(r, _)| r.hook_metrics.as_mut())
                    {
                        metrics.arm(r.perf_reset);
                    }
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
            let mut reasons: Vec<String> = if ASSETS || G::LEVEL.is_some() {
                sim.asset_failures().map(str::to_owned).collect()
            } else {
                Vec::new()
            };
            if let Some(error) = &self.error {
                reasons.push(format!("render error: {}", error.0));
            }
            if !self.device {
                reasons.push("no device".into());
            }
            if sim.is_loading() {
                reasons.push("declared content pending".into());
            }
            if ASSETS && !sim.device_assets_ready() {
                reasons.push("device assets pending".into());
            }
            if self.ready_work.is_none() {
                reasons.push("first draw pending".into());
            }
            if H::ENABLED && self.hooks.needs().contains(crate::Needs::PENDING) {
                reasons.push(self.hooks.pending_reason().to_string());
            }
            if let Some(error) = self.hooks.error() {
                reasons.push(format!("render hook: {error}"));
            }
            let ready = reasons.is_empty();
            // Which payload family this device fetches, and what is resident.
            let textures = if ASSETS {
                let resident = self
                    .render
                    .as_ref()
                    .map_or_else(|| "null".into(), |(r, _)| r.texture_summary());
                format!(
                    ",\"textureFamily\":\"{}\",\"textures\":{resident}",
                    self.payloads.family.label()
                )
            } else {
                String::new()
            };
            reply.push_str(&format!(",\"ready\":{ready},\"readyReasons\":{},\"gpu\":{{\"storageBindings\":{},\"requiredStorageBindings\":{},\"beforeReady\":{},\"afterReady\":{}{textures},\"bufferScope\":\"model instances, skin and quad buffers; excludes vertex/index, primitive pages and slots\"}}",
                exact_game::json::to_string(&reasons).unwrap(), self.storage_limit, if ASSETS { crate::STORAGE_BINDINGS } else { crate::SCENE_STORAGE_BINDINGS },
                self.ready_work.unwrap_or(work).json(),
                self.ready_work.map_or_else(Default::default, |before| work.since(before)).json()));
            if H::ENABLED {
                if let Some((renderer, _)) = &self.render {
                    if let Some(metrics) = &renderer.hook_metrics {
                        metrics.append(&mut reply, self.hooks.work(), self.hooks.needs());
                    }
                }
            }
            if H::ENABLED {
                if let Some(timing) = &mut self.hook_gpu_timing {
                    if let Some((renderer, _)) = &self.render {
                        timing.poll(&renderer.device, &renderer.queue);
                    }
                    timing.append(&mut reply);
                }
            }
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
        exact_game::bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model"))
            .unwrap()
    }
    #[test]
    #[ignore = "requires two real GPU devices; run explicitly on a GPU host"]
    fn surface_lifecycle_rebuilds_textured_draws_after_prepared_retry_loss() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut s = WorldSurface::<Cosmetic, crate::ModelPresentation, true>::default();
        s.device_ready(wgpu::Features::empty());
        s.bind(&[], None).unwrap();
        let model = model();
        let bytes = exact_game::bin::to_vec(&model);
        let tex = include_bytes!("../../bake/tests/fixtures/crate/0-srgb-straight.tex");
        s.assets();
        s.asset("hero.model", Ok(&bytes));
        s.asset(&model.textures[0], Ok(tex));
        let (before, _) = exact_gpu::fixture::render(&gpu, &mut s, &frame()).unwrap();
        let work = s.render.as_ref().unwrap().0.residency_work().json();
        assert!(!s.render.as_ref().unwrap().0.models.records.is_empty());
        s.device_lost();
        // Failed adapter retries can finish redelivery before device_ready.
        s.device_lost();
        s.assets();
        s.asset(&model.textures[0], Ok(tex));
        s.device_lost(); // another failed retry must preserve delivered bytes too
        let Some(replacement) = crate::test_device::device_or_skip(exact_gpu::fixture::device())
        else {
            return;
        };
        s.device_ready(wgpu::Features::empty());
        s.prepare_assets(
            &replacement.device,
            &replacement.queue,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        s.device_lost(); // A failed retry consumed CPU texture bytes into its renderer.
        let changes = s.assets();
        assert!(changes.retired.contains(&model.textures[0]));
        assert!(changes.requests.contains(&model.textures[0]));
        s.asset(&model.textures[0], Ok(tex));
        s.device_ready(wgpu::Features::empty());
        s.prepare_assets(
            &replacement.device,
            &replacement.queue,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let (after, _) = exact_gpu::fixture::render(&replacement, &mut s, &frame()).unwrap();
        assert!(
            before == after,
            "replacement device must draw the same model pixels"
        );
        assert_eq!(work, s.render.as_ref().unwrap().0.residency_work().json());
    }

    #[test]
    #[ignore = "requires a real GPU device; run cargo test -p exact-game-render retired_model_stays_hidden -- --ignored"]
    fn retired_model_stays_hidden_until_changed_dependency_closure_is_prepared() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut s = WorldSurface::<Cosmetic, crate::ModelPresentation, true>::default();
        s.device_ready(wgpu::Features::empty());
        s.bind(&[], None).unwrap();
        let mut model = model();
        let texture = include_bytes!("../../bake/tests/fixtures/crate/0-srgb-straight.tex");
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
        *s.sim
            .as_ref()
            .unwrap()
            .world()
            .get_mut::<exact_game::Mesh>("hero")
            .unwrap() = exact_game::Mesh::asset("hero.model");
        s.assets();
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
    fn identical_redelivery_survives_entry_and_post_acceptance_budget_compaction() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut s = WorldSurface::<Cosmetic, crate::ModelPresentation, true>::default();
        s.device_ready(wgpu::Features::empty());
        s.bind(&[], None).unwrap();
        let mut model = model();
        // Character-sized resident, already retired when the oversized cache enters preparation.
        model.meshes[0].positions.resize(80_000 * 3, 0.);
        model.meshes[0].normals.resize(80_000 * 3, 0.);
        model.meshes[0].uvs.resize(80_000 * 2, 0.);
        let bytes = exact_game::bin::to_vec(&model);
        let tex = include_bytes!("../../bake/tests/fixtures/crate/0-srgb-straight.tex");
        s.assets();
        s.asset("hero.model", Ok(&bytes));
        s.asset(&model.textures[0], Ok(tex));
        let (before, _) = exact_gpu::fixture::render(&gpu, &mut s, &frame()).unwrap();
        let handles = s.render.as_ref().unwrap().0.models.loaded["hero.model"]
            .nodes
            .clone();
        for name in ["away.model", "hero.model"] {
            *s.sim
                .as_ref()
                .unwrap()
                .world()
                .get_mut::<exact_game::Mesh>("hero")
                .unwrap() = exact_game::Mesh::asset(name);
            s.assets();
        }
        let retired = exact_game::asset::TextureData {
            width: 1024,
            height: 1024,
            mips: (0..11)
                .map(|level| vec![255; (1024usize >> level).pow(2) * 4])
                .collect(),
            ..Default::default()
        };
        let r = &mut s.render.as_mut().unwrap().0;
        for i in 0..13 {
            let name = format!("retired-{i}.tex");
            r.add_texture(&name, &retired).unwrap();
            r.retire_texture(&name);
        }
        let work = r.residency_work();
        // The new peer grows the mesh arena during this preparation pass. Its
        // unused capacity plus retired textures crosses the real 64 MiB budget.
        let mut peer = model.clone();
        let m = &mut peer.meshes[0];
        m.positions.resize(600_000 * 3, 0.);
        m.normals.resize(600_000 * 3, 0.);
        m.uvs.resize(600_000 * 2, 0.);
        s.sim.as_mut().unwrap().world_mut().spawn((
            exact_game::Transform::at(100., 0., 0.),
            exact_game::Mesh::asset("peer.model"),
        ));
        s.assets();
        s.asset("hero.model", Ok(&bytes));
        s.asset("peer.model", Ok(&exact_game::bin::to_vec(&peer)));
        let live = s
            .sim
            .as_ref()
            .unwrap()
            .presentation_assets()
            .map(str::to_owned)
            .collect();
        assert!(s.render.as_ref().unwrap().0.retired_bytes(&live) > RETIRED_BUDGET);
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        assert!(!s.sim.as_ref().unwrap().model_prepared("hero.model"));
        let r = &s.render.as_ref().unwrap().0;
        assert!(
            r.retired_bytes(&live) <= RETIRED_BUDGET,
            "post-preparation compaction ran"
        );
        assert!(
            r.models.loaded.contains_key("hero.model"),
            "accepted identical model was discarded"
        );
        assert_eq!(r.models.loaded["hero.model"].nodes, handles);
        assert_eq!(r.residency_work().since(work).texture_uploads, 0);
        assert_eq!(
            r.residency_work().since(work).mesh_uploads,
            peer.meshes.len() as u64
        );
        s.asset(&model.textures[0], Ok(tex));
        let (after, _) = exact_gpu::fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert_eq!(
            before, after,
            "same hero is drawable without duplicate upload"
        );
        assert_eq!(
            s.render
                .as_ref()
                .unwrap()
                .0
                .residency_work()
                .since(work)
                .mesh_uploads,
            peer.meshes.len() as u64,
            "closing the pending texture dependency must not upload the hero again"
        );
    }

    #[test]
    fn twenty_unique_models_bound_retirement_and_hash_only_at_delivery() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut s = WorldSurface::<Cosmetic, crate::ModelPresentation, true>::default();
        s.device_ready(wgpu::Features::empty());
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
        let mut s = WorldSurface::<Fox, crate::ModelPresentation, true>::default();
        s.device_ready(wgpu::Features::empty());
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
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut surface = WorldSurface::<Fox, crate::ModelPresentation, true>::default();
        surface.device_ready(wgpu::Features::empty());
        surface.bind(&[], None).unwrap();
        surface.asset(
            "fox.model",
            Ok(&exact_game::bin::to_vec(&crate::test_model::skinned_model())),
        );
        surface.asset(
            "fox/0-srgb-straight.tex",
            Ok(include_bytes!(
                "../../bake/tests/fixtures/crate/0-srgb-straight.tex"
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
        let saved = surface.carry().unwrap().unwrap();
        for mode in [Restore::Open, Restore::Carry] {
            surface.restore(&saved, mode).unwrap();
            exact_gpu::fixture::render(&gpu, &mut surface, &frame).unwrap();
            assert_eq!(
                before,
                surface.render.as_ref().unwrap().0.residency_work().json()
            );
            assert_eq!(surface.carry().unwrap().unwrap(), saved);
        }
        let mut texture: exact_game::asset::TextureData = exact_game::bin::from_slice(
            include_bytes!("../../bake/tests/fixtures/crate/0-srgb-straight.tex"),
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
