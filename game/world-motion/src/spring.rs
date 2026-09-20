use exact_motion::spring::SpringSample;
use exact_world::{Data, DataError, Now, Reader, Writer};

#[derive(Clone, Copy, Debug)]
pub struct SpringConfig {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
}
impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            stiffness: 100.,
            damping: 10.,
            mass: 1.,
        }
    }
}
impl SpringConfig {
    fn core(self) -> exact_motion::SpringConfig {
        exact_motion::SpringConfig {
            stiffness: self.stiffness,
            damping: self.damping,
            mass: self.mass,
        }
    }
    pub fn validate(&self) -> Result<(), DataError> {
        self.core()
            .validate()
            .map_err(|e| DataError::new(format!("invalid spring config: {e:?}")))?;
        let alpha = self.damping / (2. * self.mass);
        let omega_squared = self.stiffness / self.mass;
        if !(1e-12..=1e12).contains(&omega_squared)
            || !(0.0..=1e6).contains(&alpha)
            || !(2. * self.mass).is_finite()
        {
            return Err(DataError::new("spring numerical range"));
        }
        Ok(())
    }
}

impl Data for SpringConfig {
    fn write(&self, w: &mut dyn Writer) {
        if self.validate().is_err() {
            w.reject("invalid spring config");
            return;
        }
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
            match f {
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
    target: f64,
    /// Value at the last target change.
    start_value: f64,
    /// Velocity per second at the last target change.
    start_velocity: f64,
    /// Tick of the last target change.
    start_tick: u64,
    /// The exact-motion closed-form oscillator parameters.
    config: SpringConfig,
}
impl Spring {
    /// A motionless spring at value.
    pub fn new(value: f32) -> Self {
        assert!(value.is_finite(), "non-finite spring value");
        Self {
            target: value as f64,
            start_value: value as f64,
            ..Self::default()
        }
    }
    pub fn with_config(mut self, config: SpringConfig) -> Result<Self, DataError> {
        config
            .validate()
            .map_err(|e| DataError::new(format!("invalid spring config: {e:?}")))?;
        self.config = config;
        Ok(self)
    }
    fn sample(&self, Now { tick, hz }: Now) -> SpringSample {
        assert!(hz > 0, "spring hz must be positive");
        self.config.core().sample(
            self.start_value - self.target,
            self.start_velocity,
            tick.saturating_sub(self.start_tick) as f64 / hz as f64,
        )
    }
    /// Retarget continuously at now, preserving the old value and velocity.
    pub fn set_target(&mut self, now: Now, target: f32) {
        assert!(target.is_finite(), "non-finite spring target");
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
    /// Whether all future displacement and speed stay below the rest threshold.
    pub fn at_rest(&self, now: Now) -> bool {
        let sample = self.sample(now);
        let omega = exact_motion::math::sqrt(self.config.stiffness / self.config.mass);
        let amplitude = exact_motion::math::sqrt(
            sample.displacement * sample.displacement
                + (sample.velocity / omega) * (sample.velocity / omega),
        );
        amplitude < exact_motion::spring::REST_THRESHOLD
            && amplitude * omega < exact_motion::spring::REST_THRESHOLD
    }
}

impl Spring {
    pub fn settle_tick(&self, now: Now) -> Option<u64> {
        if self.at_rest(now) {
            return Some(now.tick);
        }
        // Bound the module's search to the core presentation horizon, without
        // treating its ten-second snap as permanent rest.
        (1..=2400).find_map(|n| {
            let tick = self
                .start_tick
                .saturating_add((n as f64 / 240. * now.hz as f64).ceil() as u64);
            (tick > now.tick && self.at_rest(Now { tick, ..now })).then_some(tick)
        })
    }
}
impl Data for Spring {
    fn write(&self, w: &mut dyn Writer) {
        if ![self.target, self.start_value, self.start_velocity]
            .into_iter()
            .all(f64::is_finite)
        {
            w.reject("non-finite spring state");
            return;
        }
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
            match f {
                "target" => self.target.read(r).map_err(|e| e.at(f))?,
                "start_value" => self.start_value.read(r).map_err(|e| e.at(f))?,
                "start_velocity" => self.start_velocity.read(r).map_err(|e| e.at(f))?,
                "start_tick" => self.start_tick.read(r).map_err(|e| e.at(f))?,
                "config" => self.config.read(r).map_err(|e| e.at(f))?,
                _ => r.skip()?,
            }
        }
        if ![self.target, self.start_value, self.start_velocity]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(DataError::new("non-finite spring state"));
        }
        Ok(())
    }
}
