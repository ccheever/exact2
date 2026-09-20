use exact_world::{Data, DataError, Now, Reader, Writer};

#[derive(Clone, Debug, Default)]
pub struct Tween {
    start_value: f32,
    target: f32,
    start_tick: u64,
    duration: f32,
}
impl Tween {
    fn valid(&self) -> bool {
        self.start_value.is_finite()
            && self.target.is_finite()
            && self.duration.is_finite()
            && self.duration >= 0.
    }
    pub fn new(value: f32) -> Self {
        assert!(value.is_finite());
        Self {
            start_value: value,
            target: value,
            ..Self::default()
        }
    }
    pub fn to(&mut self, now: Now, target: f32, duration: f32) {
        assert!(target.is_finite() && duration.is_finite() && duration >= 0.0);
        assert!(now.tick >= self.start_tick);
        self.start_value = self.value(now);
        self.target = target;
        self.start_tick = now.tick;
        self.duration = duration;
    }
    fn deadline(&self, now: Now) -> u64 {
        self.start_tick
            .saturating_add((self.duration as f64 * now.hz as f64).ceil() as u64)
    }
    pub fn value(&self, now: Now) -> f32 {
        assert!(now.hz > 0);
        if now.tick >= self.deadline(now) || self.duration == 0.0 {
            return self.target;
        }
        self.sample(now.tick.saturating_sub(self.start_tick) as f64 / now.hz as f64)
    }
    pub fn value_at(&self, seconds: f64, hz: u32) -> f32 {
        assert!(hz > 0 && seconds.is_finite());
        self.sample((seconds - self.start_tick as f64 / hz as f64).max(0.))
    }
    fn sample(&self, elapsed: f64) -> f32 {
        if self.duration == 0. || elapsed >= self.duration as f64 {
            return self.target;
        }
        let t = (elapsed / self.duration as f64).clamp(0.0, 1.0);
        let t = exact_motion::Easing::CubicBezier {
            x1: 1.0 / 3.0,
            y1: 0.0,
            x2: 2.0 / 3.0,
            y2: 1.0,
        }
        .progress(t);
        (self.start_value as f64 + (self.target as f64 - self.start_value as f64) * t) as f32
    }
}
impl Tween {
    pub fn settle_tick(&self, now: Now) -> Option<u64> {
        Some(
            if self.start_value != self.target && now.tick < self.deadline(now) {
                self.deadline(now)
            } else {
                now.tick
            },
        )
    }
}
impl Data for Tween {
    fn write(&self, w: &mut dyn Writer) {
        if !self.valid() {
            w.reject("Tween requires finite endpoints and nonnegative duration");
            return;
        }
        w.begin_struct();
        w.field("start_value");
        self.start_value.write(w);
        w.field("target");
        self.target.write(w);
        w.field("start_tick");
        self.start_tick.write(w);
        w.field("duration");
        self.duration.write(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            match field {
                "start_value" => self.start_value.read(r)?,
                "target" => self.target.read(r)?,
                "start_tick" => self.start_tick.read(r)?,
                "duration" => self.duration.read(r)?,
                _ => r.skip()?,
            }
        }
        if !self.valid() {
            return Err(DataError::new(
                "Tween requires finite endpoints and nonnegative duration",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SpringConfig;
    use exact_world::{bin, Component, World};
    #[test]
    fn built_in_motion_values_cannot_save_bytes_they_refuse() {
        for (start_value, target, duration) in [
            (0., 1., -1.),
            (f32::NAN, 1., 1.),
            (0., f32::INFINITY, 1.),
            (0., 1., f32::NAN),
        ] {
            let value = Tween {
                start_value,
                target,
                duration,
                start_tick: 0,
            };
            assert!(bin::to_vec(&value).is_err(), "invalid Tween was encoded");
        }
        for value in [
            SpringConfig {
                mass: 0.,
                ..Default::default()
            },
            SpringConfig {
                damping: f64::NAN,
                ..Default::default()
            },
        ] {
            assert!(
                bin::to_vec(&value).is_err(),
                "invalid SpringConfig was encoded"
            );
        }
        #[derive(Default, Component)]
        struct Motion(Tween);
        let mut world = World::new(60, 0);
        world.register::<Motion>().unwrap();
        world
            .spawn(Motion(Tween {
                duration: -1.,
                ..Default::default()
            }))
            .unwrap();
        assert!(world.save().is_err());
        assert!(world.sample().is_err());
        let value = Tween::new(5.);
        let bytes = bin::to_vec(&value).unwrap();
        assert_eq!(
            bin::to_vec(&bin::from_slice::<Tween>(&bytes).unwrap()).unwrap(),
            bytes
        );
        let value = SpringConfig::default();
        let bytes = bin::to_vec(&value).unwrap();
        assert_eq!(
            bin::to_vec(&bin::from_slice::<SpringConfig>(&bytes).unwrap()).unwrap(),
            bytes
        );
    }

    #[test]
    fn smoothstep_preserves_nan_with_distinct_increasing_edges() {
        assert!(crate::smoothstep(0., 1., f32::NAN).is_nan());
        assert_eq!(crate::smoothstep(0., 1., 0.5), 0.5);
    }

    #[test]
    fn invalid_motion_hash_refuses() {
        #[derive(Default, Component)]
        struct Motion(Tween);
        let mut w = World::new(60, 0);
        w.register::<Motion>().unwrap();
        w.spawn(Motion(Tween {
            duration: -1.,
            ..Default::default()
        }))
        .unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.hash()))
                .unwrap()
                .is_err()
        );
    }
}
