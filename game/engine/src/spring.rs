use crate::{Data, DataError, Reader, SpringConfig, Writer};
use exact_motion::spring::SpringSample;

impl Data for SpringConfig {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("stiffness");
        self.stiffness.write(w);
        w.field("damping");
        self.damping.write(w);
        w.field("mass");
        self.mass.write(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(f) = r.field()? {
            match f.as_str() {
                "stiffness" => self.stiffness.read(r).map_err(|e| e.at(f))?,
                "damping" => self.damping.read(r).map_err(|e| e.at(f))?,
                "mass" => self.mass.read(r).map_err(|e| e.at(f))?,
                _ => r.skip()?,
            }
        }
        self.validate()
            .map_err(|e| DataError::new(format!("invalid spring config: {e:?}")))
    }
}

/// A seekable scalar spring. Sampling never changes its anchor or accumulates dt.
#[derive(Clone, Debug, Default, Data)]
pub struct Spring {
    /// Desired resting value.
    pub target: f64,
    /// Value at the last target change.
    pub start_value: f64,
    /// Velocity per second at the last target change.
    pub start_velocity: f64,
    /// Tick of the last target change.
    pub start_tick: u64,
    /// The exact-motion closed-form oscillator parameters.
    pub config: SpringConfig,
}
impl Spring {
    /// A motionless spring at value.
    pub fn new(value: f64) -> Self {
        Self {
            target: value,
            start_value: value,
            ..Self::default()
        }
    }
    fn sample(&self, tick: u64, hz: u32) -> SpringSample {
        assert!(hz > 0, "spring hz must be positive");
        self.config.sample(
            self.start_value - self.target,
            self.start_velocity,
            tick.saturating_sub(self.start_tick) as f64 / hz as f64,
        )
    }
    /// Retarget continuously at tick; hz is needed to recover the old velocity.
    pub fn set_target(&mut self, tick: u64, hz: u32, target: f64) {
        assert!(
            tick >= self.start_tick,
            "spring retarget precedes its anchor"
        );
        let s = self.sample(tick, hz);
        self.start_value = self.target + s.displacement;
        self.start_velocity = s.velocity;
        self.start_tick = tick;
        self.target = target;
    }
    /// Sample the value at tick, independent of all previous reads.
    pub fn value(&self, tick: u64, hz: u32) -> f64 {
        self.target + self.sample(tick, hz).displacement
    }
    /// Whether displacement and speed satisfy exact-motion's rest threshold.
    pub fn at_rest(&self, tick: u64, hz: u32) -> bool {
        self.sample(tick, hz).at_rest()
    }
}
