use crate::{
    perf::{Perf, Stamp},
    Feed, Renderer,
};
use exact_game::{Clock, Game, Sim, World};
use exact_gpu::{wgpu, Frame, InputEvent, Surface, SurfaceError, Value};

/// One simulation and its lazily created GPU renderer, for an exact canvas.
/// Pipeline creation happens at the first render, never during bind or agent reads.
pub struct WorldSurface<G: Game> {
    sim: Option<Sim<G>>,
    render: Option<(Renderer, Feed)>,
    format: Option<wgpu::TextureFormat>,
    perf: Perf,
    trace: Option<crate::trace::Trace>,
    error: Option<SurfaceError>,
    dirty: bool,
    reported: bool,
    generation: u64,
}
impl<G: Game> Default for WorldSurface<G> {
    fn default() -> Self {
        Self {
            sim: None,
            render: None,
            format: None,
            perf: Perf::default(),
            trace: None,
            error: None,
            dirty: true,
            reported: false,
            generation: 0,
        }
    }
}
impl<G: Game> WorldSurface<G> {
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
impl<G: Game> Surface for WorldSurface<G> {
    fn bind(&mut self, values: &[Value]) -> Result<(), SurfaceError> {
        self.bind_at(values, None)
    }
    fn bind_at(&mut self, values: &[Value], at_ms: Option<f64>) -> Result<(), SurfaceError> {
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
            self.sim = Some(Sim::from_values(values).map_err(SurfaceError)?);
        }
        self.dirty = true;
        Ok(())
    }
    fn carry(&mut self) -> Option<Vec<u8>> {
        self.sim.as_ref().map(Sim::save)
    }
    fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
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
            self.dirty = true;
            return self.error.is_none();
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
        self.perf.ticks.push(ticks as f64);
        if self.error.is_some() {
            return false;
        }
        let (renderer, feed) = self.render.as_mut().unwrap();
        let start = (!frame.seekable).then(Stamp::now);
        let input = feed.frame(sim.world(), sim.alpha(), frame.width / frame.height);
        self.perf.stats = renderer.draw(device, queue, target, format, frame.pixels(), &input);
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
        self.sim.as_mut().map_or_else(Vec::new, Sim::take_messages)
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
            reply.truncate(reply.len() - 2);
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
