use crate::{Data, DataError, Now, Reader, SpringConfig, Writer};
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
#[derive(Clone, Debug, Default)]
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
    pub fn new(value: f32) -> Self {
        Self {
            target: value as f64,
            start_value: value as f64,
            ..Self::default()
        }
    }
    fn sample(&self, Now { tick, hz }: Now) -> SpringSample {
        assert!(hz > 0, "spring hz must be positive");
        self.config.sample(
            self.start_value - self.target,
            self.start_velocity,
            tick.saturating_sub(self.start_tick) as f64 / hz as f64,
        )
    }
    /// Retarget continuously at now, preserving the old value and velocity.
    pub fn set_target(&mut self, now: Now, target: f32) {
        assert!(
            now.tick >= self.start_tick,
            "spring retarget precedes its anchor"
        );
        let s = self.sample(now);
        self.start_value = self.target + s.displacement;
        self.start_velocity = s.velocity;
        self.start_tick = now.tick;
        self.target = target as f64;
    }
    /// Sample the value at tick, independent of all previous reads.
    pub fn value(&self, now: Now) -> f32 {
        (self.target + self.sample(now).displacement) as f32
    }
    /// Velocity per second at now.
    pub fn velocity(&self, now: Now) -> f32 {
        self.sample(now).velocity as f32
    }
    /// Whether displacement and speed satisfy exact-motion's rest threshold.
    pub fn at_rest(&self, now: Now) -> bool {
        self.sample(now).at_rest()
    }
}

impl Data for Spring {
    fn moving(&self, now: Now) -> bool {
        !self.at_rest(now)
    }
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("target");
        self.target.write(w);
        w.field("start_value");
        self.start_value.write(w);
        w.field("start_velocity");
        self.start_velocity.write(w);
        w.field("start_tick");
        self.start_tick.write(w);
        w.field("config");
        self.config.write(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(f) = r.field()? {
            match f.as_str() {
                "target" => self.target.read(r).map_err(|e| e.at(f))?,
                "start_value" => self.start_value.read(r).map_err(|e| e.at(f))?,
                "start_velocity" => self.start_velocity.read(r).map_err(|e| e.at(f))?,
                "start_tick" => self.start_tick.read(r).map_err(|e| e.at(f))?,
                "config" => self.config.read(r).map_err(|e| e.at(f))?,
                _ => r.skip()?,
            }
        }
        Ok(())
    }
}
