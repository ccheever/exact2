//! Opt-in, single-entity presentation read. No storage or clock work while off.
use crate::perf::Stamp;
use exact_game::{Data, DataError, Entity, Parent, Reader, Transform, World};
use std::fmt::Write;

pub(crate) const STRIDE: usize = 14;
pub(crate) enum Request {
    Arm { entity: String, frames: usize },
    Read,
    Stop,
}
pub(crate) fn request(text: &str) -> Result<Option<Request>, DataError> {
    let mut r = exact_game::json::Decoder::new(text);
    let mut op = String::new();
    // Decode the trace separately: it is either a string or an object.
    let mut trace = false;
    r.begin_struct()?;
    while let Some(field) = r.field()? {
        match field.as_str() {
            "op" => op = r.string()?,
            "trace" => {
                trace = true;
                r.skip()?;
            }
            _ => r.skip()?,
        }
    }
    r.finish()?;
    if op != "state" {
        return Ok(None);
    }
    if !trace {
        return Ok(None);
    }
    #[derive(Default, Data)]
    struct Command {
        trace: String,
    }
    if let Ok(command) = exact_game::json::from_str::<Command>(text) {
        return match command.trace.as_str() {
            "read" => Ok(Some(Request::Read)),
            "stop" => Ok(Some(Request::Stop)),
            _ => Err(DataError::new(
                "trace expects read, stop, or {entity, frames}",
            )),
        };
    }
    #[derive(Default, Data)]
    struct Arm {
        entity: String,
        frames: u32,
    }
    #[derive(Default, Data)]
    struct ArmRequest {
        trace: Arm,
    }
    let arm = exact_game::json::from_str::<ArmRequest>(text)?.trace;
    if !(1..=1_000_000).contains(&arm.frames) {
        return Err(DataError::new("trace frames must be 1..1000000"));
    }
    Ok(Some(Request::Arm {
        entity: arm.entity,
        frames: arm.frames as usize,
    }))
}

pub(crate) fn mix(prev: [f32; 3], curr: [f32; 3], alpha: f32) -> [f64; 3] {
    std::array::from_fn(|i| prev[i] as f64 + (curr[i] as f64 - prev[i] as f64) * alpha as f64)
}
pub(crate) struct Trace {
    pub times: [f64; 3],
    entity: Entity,
    prev: [f32; 3],
    curr: [f32; 3],
    tick: u64,
    generation: u64,
    parents: u64,
    transforms: u64,
    values: Vec<f64>,
    count: usize,
    total: usize,
    origin: Stamp,
    missing: bool,
    primed: bool,
    skipped: usize,
}
impl Trace {
    pub fn new(w: &World, entity: &str, frames: usize) -> Result<Self, String> {
        let entity = w.resolve(entity).ok_or("trace entity not found")?;
        let p = w
            .global(entity)
            .ok_or("trace entity has no global pose")?
            .translation
            .to_array();
        Ok(Self {
            times: [0.; 3],
            entity,
            prev: p,
            curr: p,
            tick: w.tick(),
            generation: w.presentation_generation(),
            parents: w.revision::<Parent>(),
            transforms: w.revision::<Transform>(),
            values: vec![0.; frames * STRIDE],
            count: 0,
            total: 0,
            origin: Stamp::now(),
            missing: false,
            primed: false,
            skipped: 0,
        })
    }
    // Called at the same feeds as GPU history, including both final ticks in a burst.
    pub fn feed(&mut self, w: &World) {
        let Some(pose) = w.global(self.entity) else {
            self.missing = true;
            return;
        };
        let p = pose.translation.to_array();
        // A same-tick edit also swaps the GPU's transform buffers. Camera
        // history has its own rule, and is read directly from Feed instead.
        if w.tick() != self.tick
            || self.transforms != w.revision::<Transform>()
            || self.parents != w.revision::<Parent>()
        {
            self.primed = true;
            self.prev = self.curr;
        }
        self.curr = p;
        if self.generation != w.presentation_generation()
            || crate::world::trace_snap(w, self.entity, self.parents != w.revision::<Parent>())
        {
            self.prev = p;
            self.primed = true;
        }
        self.tick = w.tick();
        self.generation = w.presentation_generation();
        self.parents = w.revision::<Parent>();
        self.transforms = w.revision::<Transform>();
    }
    pub fn frame(&mut self, now: f64, alpha: f32, ticks: u32, camera: [f64; 3], times: [f64; 3]) {
        // Arming during motion cannot recover the GPU's preceding tick. Wait
        // until one observed boundary establishes an honest pair of poses.
        if !self.primed {
            self.skipped += 1;
            return;
        }
        let p = mix(self.prev, self.curr, alpha);
        let wall = self.origin.wall_ms();
        let i = self.total % (self.values.len() / STRIDE) * STRIDE;
        self.values[i..i + STRIDE].copy_from_slice(&[
            now,
            wall,
            p[0],
            p[1],
            p[2],
            camera[0],
            camera[1],
            camera[2],
            alpha as f64,
            ticks as f64,
            self.tick as f64,
            times[0],
            times[1],
            times[2],
        ]);
        self.total += 1;
        self.count = self.total.min(self.values.len() / STRIDE);
    }
    pub fn read(&self) -> String {
        let capacity = self.values.len() / STRIDE;
        let mut out = format!("{{\"trace\":{{\"stride\":{STRIDE},\"overflow\":{},\"missing\":{},\"total\":{},\"initial_frames_skipped\":{},\"frames\":[",
            self.total > capacity, self.missing, self.total, self.skipped);
        let first = self.total.saturating_sub(capacity);
        for n in 0..self.count {
            let i = (first + n) % capacity * STRIDE;
            for k in 0..STRIDE {
                if n != 0 || k != 0 {
                    out.push(',');
                }
                write!(out, "{}", self.values[i + k]).unwrap();
            }
        }
        out.push_str("]}}");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::{Clock, Game, Input, Sim};
    struct Walker;
    impl Game for Walker {
        const ID: &'static str = "trace-test";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("player", (Transform::default(),));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            let e = w.named("player").unwrap();
            w.get_mut::<Transform>(e).unwrap().position.x += 1.;
        }
    }
    #[test]
    fn live_interpolation_at_144hz_and_tick_bursts_keeps_the_last_two_ticks() {
        let mut sim = Sim::<Walker>::new(()).unwrap();
        let mut trace = Trace::new(sim.world(), "player", 200).unwrap();
        sim.advance(0., Clock::Live);
        for frame in 1..=144 {
            let now = frame as f64 * 1000. / 144.;
            let ticks = sim.advance_with(now, Clock::Live, |w, left| {
                if left < 2 {
                    trace.feed(w)
                }
            });
            trace.frame(now, sim.alpha(), ticks, [0.; 3], [0.; 3]);
            if !trace.primed {
                continue;
            }
            let drawn = trace.values[(trace.count - 1) * STRIDE + 2];
            // T = frame * 1000/144 ms, L = 1000/144 ms, step = 1000/60 ms.
            // R = T + L - step; x = R/step = (frame + 1)*60/144 - 1.
            let expected = ((frame + 1) as f64 * 60. / 144. - 1.).max(0.);
            assert!(
                (drawn - expected).abs() < 0.0001,
                "{frame}: {drawn} != {expected}"
            );
        }
        sim.advance_with(1100., Clock::Live, |w, left| {
            if left < 2 {
                trace.feed(w)
            }
        });
        assert_eq!(trace.prev[0], 65.);
        assert_eq!(trace.curr[0], 66.);
    }
    #[test]
    fn same_tick_edits_and_teleports_match_gpu_history_rules() {
        let mut sim = Sim::<Walker>::new(()).unwrap();
        sim.advance(0., Clock::Live);
        sim.advance(17., Clock::Live);
        let mut trace = Trace::new(sim.world(), "player", 4).unwrap();
        trace.frame(17., sim.alpha(), 0, [0.; 3], [0.; 3]);
        assert_eq!(trace.count, 0); // No invented pre-arm pose.
        sim.advance_with(34., Clock::Live, |w, _| trace.feed(w));
        // T = [17, 34] ms, L = step = 1000/60 ms, so R = T.
        // x = [1.02, 2.04]; ticks = [2, 3], and the final history is [2, 3].
        assert_eq!(trace.prev[0], 2.);
        assert!((mix(trace.prev, trace.curr, sim.alpha())[0] - 2.04).abs() < 1e-6);
        sim.world()
            .get_mut::<Transform>("player")
            .unwrap()
            .position
            .x = 5.;
        trace.feed(sim.world());
        assert_eq!((trace.prev[0], trace.curr[0]), (3., 5.));
        let player = sim.world().named("player").unwrap();
        sim.world_mut().teleport(player, Transform::at(20., 0., 0.));
        trace.feed(sim.world());
        assert_eq!((trace.prev[0], trace.curr[0]), (20., 20.));
    }
    #[test]
    fn trace_is_bounded_chronological_and_read_does_not_round() {
        let sim = Sim::<Walker>::new(()).unwrap();
        let mut trace = Trace::new(sim.world(), "player", 2).unwrap();
        trace.curr[0] = 1.;
        trace.primed = true;
        for i in 0..3 {
            trace.frame(i as f64, 0.123456, 0, [0.; 3], [0.; 3]);
        }
        let json = trace.read();
        assert!(json.contains("\"overflow\":true"));
        assert!(json.contains("\"total\":3"));
        assert!(json.contains("0.12345600"));
        assert!(json.contains("\"frames\":[1,"));
    }
    #[test]
    fn state_forms_validate_without_turning_other_operations_into_traces() {
        assert!(matches!(
            request(r#"{"op":"state","trace":{"entity":"player","frames":4096}}"#).unwrap(),
            Some(Request::Arm { frames: 4096, .. })
        ));
        for command in ["read", "stop"] {
            assert!(request(&format!(r#"{{"op":"state","trace":"{command}"}}"#))
                .unwrap()
                .is_some());
        }
        assert!(request(r#"{"op":"tree","trace":"read"}"#)
            .unwrap()
            .is_none());
        assert!(request(r#"{"op":"state","trace":{"entity":"player","frames":0}}"#).is_err());
    }
}
