//! Device-free ownership. Presentation and host pacing never enter kernel state.
use crate::{args, publication};
use exact_gpu::{InputEvent, Lifecycle, PointerPhase, Restore, Surface, SurfaceError, Value};
use exact_world::{json, Args, Data, DataError, Game, Paranoid, Sim, Writer};
struct Request(Vec<(String, Value)>);
impl Request {
    fn get(&self, name: &str) -> Option<&Value> {
        self.0.iter().find(|(key, _)| key == name).map(|(_, v)| v)
    }
}

const MAX_REQUEST: usize = 16_384;

pub struct WorldSurface<G: Game> {
    sim: Option<Sim<G>>,
    host: Option<f64>,
    seekable: bool,
    hidden: bool,
    interrupted: bool,
    restored: bool,
    publish: bool,
    error: Option<SurfaceError>,
    failed: Option<String>,
    logs: crate::logs::History,
}
impl<G: Game> Default for WorldSurface<G> {
    fn default() -> Self {
        Self {
            sim: None,
            host: None,
            seekable: true,
            hidden: false,
            interrupted: false,
            restored: false,
            publish: false,
            error: None,
            failed: None,
            logs: crate::logs::History::default(),
        }
    }
}
fn error(e: impl ToString) -> SurfaceError {
    let mut text = e.to_string();
    if text.len() > 4096 {
        let mut end = 4096;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("… (error truncated at 4096 bytes)");
    }
    SurfaceError(text)
}
fn invalid(e: impl ToString) -> DataError {
    DataError::new(e.to_string())
}
fn number(q: &Request, key: &str) -> Result<Option<f64>, DataError> {
    match q.get(key) {
        None => Ok(None),
        Some(v) => v
            .as_number()
            .filter(|n| n.is_finite() && *n >= 0.)
            .map(Some)
            .ok_or_else(|| invalid(format!("invalid {key}"))),
    }
}
fn paranoid() -> Paranoid {
    let mode = if cfg!(target_arch = "wasm32") {
        option_env!("EXACT_GAME_PARANOID").unwrap_or("0").to_owned()
    } else {
        std::env::var("EXACT_GAME_PARANOID").unwrap_or_default()
    };
    match mode.as_str() {
        "1" => Paranoid::Save,
        "fresh-game" => Paranoid::FreshGame,
        _ => Paranoid::Off,
    }
}
impl<G: Game> WorldSurface<G> {
    pub fn sim(&self) -> Option<&Sim<G>> {
        self.sim.as_ref()
    }
    fn sim_mut(&mut self) -> Result<&mut Sim<G>, DataError> {
        self.sim
            .as_mut()
            .ok_or_else(|| invalid("world has not been bound"))
    }
    // A zero-delta drive distinguishes a refused request from a failed simulation.
    // Only the latter survives inspection and reports once through take_error.
    fn record_failure(&mut self) {
        if self.failed.is_none() {
            if let Some(e) = self.sim.as_mut().and_then(|sim| sim.run(0.).err()) {
                let e = error(e);
                self.failed = Some(e.0.clone());
                self.error = Some(e);
            }
        }
    }
    fn run(&mut self, ms: f64) -> Result<(), DataError> {
        let result = self.sim_mut()?.run(ms);
        if result.is_err() {
            self.record_failure();
        }
        result?;
        Ok(())
    }
    fn advance_to(&mut self, now: f64) -> Result<bool, DataError> {
        if !now.is_finite() || now < 0. {
            return Err(invalid("invalid clock"));
        }
        let dt = self.host.map_or(0., |old| now - old);
        if dt < 0. {
            return Err(invalid("host clock cannot retreat"));
        }
        let ticks = if self.seekable || !(self.hidden || self.interrupted) {
            let sim = self.sim_mut()?;
            let result = sim.advance_to(sim.clock_ms() + dt);
            if result.is_err() {
                self.record_failure();
            }
            result?
        } else {
            0
        };
        self.host = Some(now);
        self.publish |= self.sim_mut()?.world().take_published().is_some();
        Ok(ticks > 0 || self.publish)
    }
    fn stamp(&self, now: f64) -> f64 {
        if !now.is_finite() || now < 0. {
            return f64::NAN;
        }
        self.sim.as_ref().map_or(0., Sim::clock_ms)
            + self.host.map_or(0., |old| (now - old).max(0.))
    }
    fn ownership(&self, out: &mut dyn Writer) {
        out.begin_struct();
        out.field("owner");
        out.string(if self.seekable { "agent" } else { "human" });
        out.end_struct();
    }
    fn request(&mut self, text: &str) -> Result<String, DataError> {
        if text.len() > MAX_REQUEST {
            return Err(invalid("agent request exceeds 16384 bytes"));
        }
        let q = Request(exact_gpu::json::parse_fields(text).map_err(invalid)?);
        let op = q
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("missing op"))?;
        if op == "clock" {
            if let Some(e) = &self.failed {
                return Err(invalid(e));
            }
            let now = number(&q, "now")?;
            if let Some(owner) = q.get("owner").and_then(Value::as_str) {
                if !matches!(owner, "agent" | "human") {
                    return Err(invalid("unknown clock owner"));
                }
                let sim = self.sim_mut()?;
                sim.reconcile_input(sim.clock_ms(), &[])?;
                self.seekable = owner == "agent";
                self.host = now;
            } else if q.get("reload").and_then(Value::as_bool) == Some(true) {
                if q.get("releaseInput").and_then(Value::as_bool) == Some(true) {
                    let sim = self.sim_mut()?;
                    sim.reconcile_input(sim.clock_ms(), &[])?;
                }
                self.host = now;
            } else if q.get("settle").and_then(Value::as_bool) == Some(true) {
                let result = self.sim_mut()?.settle(3600);
                if result.is_err() {
                    self.record_failure();
                }
                result?;
                self.host = now.or(self.host);
            } else if let Some(ticks) = number(&q, "ticks")? {
                if ticks.fract() != 0. || ticks > 216_000. {
                    return Err(invalid("clock request exceeds 216000 ticks"));
                }
                self.run(ticks * 1000. / G::HZ as f64)?;
                self.host = now.or(self.host);
            } else if let Some(now) = now {
                self.advance_to(now)?;
            }
        }
        let sim = self
            .sim
            .as_ref()
            .ok_or_else(|| invalid("world has not been bound"))?;
        let world = sim.world();
        if op == "logs" {
            let since = number(&q, "since")?.unwrap_or(0.);
            if since.fract() != 0. || since > 9_007_199_254_740_991. {
                return Err(invalid("invalid log cursor"));
            }
            return self.logs.read(world, since as u64);
        }
        let mut out = json::Encoder::default();
        out.begin_struct();
        out.field("tick");
        world.tick().write(&mut out);
        match op {
            "state" | "tree" if q.get("entity").and_then(Value::as_str).is_some() => {
                let name = q
                    .get("entity")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("missing entity"))?;
                if name != "*" {
                    let e = world
                        .resolve(name)
                        .ok_or_else(|| invalid("unknown entity"))?;
                    out.field("entity");
                    out.begin_struct();
                    out.field("id");
                    e.index().write(&mut out);
                    out.field("name");
                    out.string(world.name(e).unwrap_or(""));
                    out.field("components");
                    world.visit(Some(e), &mut out)?;
                    out.end_struct();
                } else {
                    self.entities(&mut out)?;
                }
            }
            "state" => {
                out.field("world");
                out.begin_struct();
                out.field("name");
                out.string("world");
                out.field("game");
                out.string(G::ID);
                out.field("tick");
                world.tick().write(&mut out);
                out.field("hz");
                G::HZ.write(&mut out);
                out.field("hash");
                if self.failed.is_some() {
                    out.unit();
                } else {
                    out.string(&format!("0x{:016x}", world.hash()?));
                }
                out.field("failed");
                self.failed.is_some().write(&mut out);
                out.field("error");
                if let Some(e) = &self.failed {
                    out.string(e);
                } else {
                    out.unit();
                }
                out.field("ready");
                (world.tick() > 0 && self.failed.is_none()).write(&mut out);
                out.field("readyReasons");
                if let Some(e) = &self.failed {
                    vec![e.clone()]
                } else if world.tick() > 0 {
                    Vec::<String>::new()
                } else {
                    vec!["first tick pending".into()]
                }
                .write(&mut out);
                out.field("presentation");
                out.string("none");
                out.field("restored");
                self.restored.write(&mut out);
                out.field("args");
                sim.args().write(&mut out);
                out.field("ownership");
                self.ownership(&mut out);
                out.field("resources");
                world.visit(None, &mut out)?;
                out.field("published");
                out.begin_struct();
                for (name, value) in world.publications().iter() {
                    out.key(name);
                    publication::inspect(value, &mut out);
                }
                out.end_struct();
                out.field("report");
                world.report(&mut out)?;
                out.end_struct();
            }
            "tree" => self.entities(&mut out)?,
            "clock" => {
                out.field("hash");
                out.string(&format!("0x{:016x}", world.hash()?));
                out.field("quiescent");
                world.quiescent().write(&mut out);
                out.field("ownership");
                self.ownership(&mut out);
                if q.get("reload").and_then(Value::as_bool) == Some(true) {
                    out.field("reload");
                    out.begin_struct();
                    out.field("values");
                    let values = args::argument_values(sim.args())?;
                    out.begin_seq(values.len());
                    for value in values {
                        out.item();
                        publication::inspect(&publication::from_contract(value)?, &mut out);
                    }
                    out.end_seq();
                    out.field("names");
                    out.begin_seq(G::Args::FIELDS.len());
                    for (name, _) in G::Args::FIELDS {
                        out.item();
                        out.string(name);
                    }
                    out.end_seq();
                    out.field("setupIndices");
                    out.begin_seq(0);
                    for (i, (_, kind)) in G::Args::FIELDS.iter().enumerate() {
                        if *kind == exact_world::args::ArgumentKind::Setup {
                            out.item();
                            (i as u32).write(&mut out);
                        }
                    }
                    out.end_seq();
                    out.field("releasedInput");
                    (q.get("releaseInput").and_then(Value::as_bool) == Some(true)).write(&mut out);
                    out.field("rebased");
                    out.boolean(true);
                    out.end_struct();
                }
            }
            _ => return Err(invalid(format!("unsupported world operation: {op}"))),
        }
        out.end_struct();
        out.finish()
    }
    fn entities(&self, out: &mut dyn Writer) -> Result<(), DataError> {
        let w = self
            .sim
            .as_ref()
            .ok_or_else(|| invalid("world has not been bound"))?
            .world();
        out.field("game");
        out.string(G::ID);
        out.field("hash");
        if let Some(error) = &self.failed {
            out.unit();
            out.field("failed");
            out.boolean(true);
            // A top-level error is a refused agent operation. This is a
            // successful inspection of a failed world, as with state.world.
            out.field("world");
            out.begin_struct();
            out.field("failed");
            out.boolean(true);
            out.field("error");
            out.string(error);
            out.end_struct();
        } else {
            out.string(&format!("0x{:016x}", w.hash()?));
        }
        out.field("entities");
        out.begin_seq(w.entities().take(512).count());
        for e in w.entities().take(512) {
            if out.stopped() {
                break;
            }
            out.item();
            out.begin_struct();
            out.field("id");
            e.index().write(out);
            out.field("name");
            out.string(w.name(e).unwrap_or(""));
            out.field("components");
            w.visit(Some(e), out)?;
            out.end_struct();
        }
        out.end_seq();
        out.field("truncated");
        w.entities().nth(512).is_some().write(out);
        Ok(())
    }
}
impl<G: Game> Surface for WorldSurface<G> {
    fn advance(&mut self, now_ms: f64) -> bool {
        if self.failed.is_some() {
            return false;
        }
        match self.advance_to(now_ms) {
            Ok(changed) => changed,
            Err(e) => {
                if self.error.is_none() {
                    self.error = Some(error(e));
                }
                false
            }
        }
    }
    fn render(
        &mut self,
        frame: &exact_gpu::Frame,
        _: &exact_gpu::wgpu::Device,
        _: &exact_gpu::wgpu::Queue,
        _: &exact_gpu::wgpu::TextureView,
        _: exact_gpu::wgpu::TextureFormat,
    ) -> bool {
        Surface::advance(self, frame.now_ms);
        self.failed.is_none()
    }
    fn lifecycle(&mut self, event: Lifecycle) {
        match event {
            Lifecycle::Hidden => self.hidden = true,
            Lifecycle::Visible => self.hidden = false,
            Lifecycle::Interrupted => self.interrupted = true,
            Lifecycle::Resumed => self.interrupted = false,
            _ => {}
        }
        if !self.seekable {
            self.host = None;
        }
    }
    fn clock(&mut self, seekable: bool) {
        if self.seekable != seekable {
            self.host = None;
        }
        self.seekable = seekable;
    }
    fn arguments(&self) -> Vec<(&'static str, Value)> {
        G::Args::FIELDS
            .iter()
            .map(|(name, _)| *name)
            .zip(
                args::argument_values(&G::Args::default())
                    .unwrap_or_else(|_| vec![Value::Unit; G::Args::FIELDS.len()]),
            )
            .collect()
    }
    fn bind(&mut self, values: &[Value], at_ms: Option<f64>) -> Result<(), SurfaceError> {
        if at_ms.is_some_and(|at| !at.is_finite() || at < 0.) {
            return Err(error("invalid bind clock"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        let started = (self.sim.is_none() && std::env::var_os("EXACT_WORLD_TIMING").is_some())
            .then(std::time::Instant::now);
        let args = args::decode_args::<G::Args>(values).map_err(error)?;
        G::validate(&args).map_err(error)?;
        if self.sim.is_some() {
            let restart = self
                .sim
                .as_ref()
                .is_some_and(|sim| sim.args().setup_changed(&args));
            if !restart || self.failed.is_none() {
                if let Some(at) = at_ms {
                    self.advance_to(at).map_err(error)?;
                }
            }
            self.sim_mut().map_err(error)?.bind(args).map_err(error)?;
            if restart {
                self.failed = None;
                self.error = None;
                self.host = at_ms;
            }
        } else {
            self.sim = Some(Sim::new(args).map_err(error)?.paranoid(paranoid()));
            self.host = at_ms;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let bound = started.map(|_| std::time::Instant::now());
        if self.sim_mut().map_err(error)?.world().tick() == 0 {
            self.run((1_000_000. / G::HZ as f64).ceil() / 1000.)
                .map_err(error)?;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let (Some(started), Some(bound)) = (started, bound) {
            let tick = bound.elapsed().as_secs_f64() * 1000.;
            eprintln!(
                "exact-world-binding: {{\"bind_ms\":{},\"first_tick_ms\":{tick}}}",
                bound.duration_since(started).as_secs_f64() * 1000.
            );
        }
        self.publish = true;
        Ok(())
    }
    fn carry(&mut self) -> Result<Option<Vec<u8>>, SurfaceError> {
        self.sim
            .as_ref()
            .map(|sim| sim.save().map_err(error))
            .transpose()
    }
    fn restore(&mut self, bytes: &[u8], mode: Restore) -> Result<(), String> {
        let sim = self.sim.as_mut().ok_or("world has not been bound")?;
        match mode {
            Restore::Open => sim.restore(bytes),
            Restore::Carry => sim.carry(bytes).map(|_| ()),
        }
        .map_err(|e| e.to_string())?;
        self.restored = true;
        self.publish = true;
        if !self.seekable {
            self.host = None;
        }
        self.error = None;
        self.failed = None;
        Ok(())
    }
    fn take_error(&mut self) -> Option<SurfaceError> {
        self.error.take()
    }
    fn wants_input(&self) -> bool {
        true
    }
    fn input(&mut self, event: &InputEvent) {
        use exact_world::InputEvent as E;
        let e = match event {
            InputEvent::Key {
                code, down, at_ms, ..
            } => E::Key {
                code: code.clone(),
                down: *down,
                at_ms: self.stamp(*at_ms),
            },
            InputEvent::Control {
                name,
                phase,
                at_ms,
                x,
                ..
            } => {
                if G::ACTIONS
                    .iter()
                    .any(|a| a.name == name && a.axis_keys.is_some())
                {
                    E::Axis {
                        name: name.clone(),
                        value: if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
                            0.
                        } else {
                            *x
                        },
                        at_ms: self.stamp(*at_ms),
                    }
                } else {
                    E::Action {
                        name: name.clone(),
                        down: matches!(phase, PointerPhase::Down | PointerPhase::Move),
                        at_ms: self.stamp(*at_ms),
                    }
                }
            }
            InputEvent::Blur { at_ms } => E::Blur {
                at_ms: self.stamp(*at_ms),
            },
            _ => return,
        };
        if let Err(e) = self.sim_mut().and_then(|s| s.input(e)) {
            if self.failed.is_none() {
                self.error = Some(error(e));
            }
        }
    }
    fn messages(&mut self) -> Vec<String> {
        if self.failed.is_some() {
            return Vec::new();
        }
        self.sim
            .as_ref()
            .map_or_else(Vec::new, |s| s.world().take_messages())
    }
    fn published(&mut self) -> Option<String> {
        if self.failed.is_some() {
            return None;
        }
        let w = self.sim.as_ref()?.world();
        if w.take_published().is_none() && !self.publish {
            return None;
        }
        self.publish = false;
        match publication::json(w) {
            Ok(value) => Some(value),
            Err(e) => {
                self.error = Some(error(e));
                None
            }
        }
    }
    fn agent(&mut self, request: &str) -> Option<String> {
        Some(self.request(request).unwrap_or_else(|e| {
            format!(
                "{{\"error\":{}}}",
                json::to_string(&error(e).0)
                    .unwrap_or_else(|_| "\"world error exceeds inspection budget\"".into())
            )
        }))
    }
}
