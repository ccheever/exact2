//! Sampled sensors a canvas takes as input (LLP 1133.013 D6): every sample a
//! host delivers reaches the tick whose window holds its stamp, the latest
//! holds through ticks that receive none, and an end takes it away. A tilt
//! stick steers by the gravity sensor against a neutral. What the samples
//! leave behind (the latest of each sensor, the neutrals) is world state, a
//! resource, so it saves and hashes; a world that never received a sample
//! and never set a neutral has no such resource.
use super::{pad, Input};
use crate::{Data, Resource, Vec2, Vec3, World};

/// A sampled sensor, by its Generic Sensor name (`exact_gpu::SensorKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Data)]
pub enum SensorKind {
    /// `Accelerometer`: m/s², gravity included.
    Accelerometer,
    /// `LinearAccelerationSensor`: m/s², gravity removed.
    LinearAcceleration,
    /// `GravitySensor`: m/s², pointing away from the Earth (+z face up).
    #[default]
    Gravity,
    /// `Gyroscope`: rad/s.
    Gyroscope,
    /// `Magnetometer`: µT.
    Magnetometer,
    /// `AbsoluteOrientationSensor`: a quaternion `[x, y, z, w]`.
    AbsoluteOrientation,
    /// `RelativeOrientationSensor`: a quaternion `[x, y, z, w]`.
    RelativeOrientation,
}

/// One sample as a tick reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub struct SensorSample {
    /// x, y, z, and w for an orientation's quaternion; always finite.
    pub values: [f32; 4],
    /// Samples of this sensor the host lost before this one.
    pub dropped: u32,
    /// World microseconds at which it was taken.
    pub world_us: i64,
}

/// A tilt stick's source: `Stick::wasd().or_tilt(Tilt { max_degrees: 20.0, dead_zone: 2.0 })`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub struct Tilt {
    /// Degrees from the neutral that read full deflection.
    pub max_degrees: f32,
    /// Degrees from the neutral that read nothing (a radial dead zone).
    pub dead_zone: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Data)]
pub(crate) struct Reading {
    sensor: SensorKind,
    sample: SensorSample,
}

#[derive(Clone, Debug, Default, PartialEq, Data)]
pub(crate) struct Neutral {
    action: String,
    gravity: [f32; 3],
}

/// What sampled input leaves for the coming ticks: the latest sample of each
/// sensor whose input runs, the first gravity sample since that input began
/// (a tilt stick's neutral where the game set none) and the neutrals the game
/// set. Input works on a copy; the world's is the one that saves and hashes.
#[derive(Clone, Debug, Default, PartialEq, Resource)]
pub struct SensorState {
    readings: Vec<Reading>,
    first_gravity: Option<[f32; 3]>,
    neutrals: Vec<Neutral>,
}
impl SensorState {
    pub(crate) fn is_empty(&self) -> bool {
        self.readings.is_empty() && self.first_gravity.is_none() && self.neutrals.is_empty()
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.readings.windows(2).any(|p| p[0].sensor >= p[1].sensor)
            || self.neutrals.windows(2).any(|p| p[0].action >= p[1].action)
        {
            return Err("saved sensor state must be sorted and unique".into());
        }
        let finite = self.readings.iter().all(|r| finite(&r.sample.values))
            && self.first_gravity.is_none_or(|g| gravity(g).is_some())
            && self.neutrals.iter().all(|n| gravity(n.gravity).is_some());
        if !finite {
            return Err("saved sensor state needs finite readings".into());
        }
        Ok(())
    }
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// A finite, nonzero gravity vector.
fn gravity(v: [f32; 3]) -> Option<Vec3> {
    let v = Vec3::from_array(v);
    (v.is_finite() && v.length_squared() > 0.).then_some(v)
}

/// The angle in degrees of `v` toward one axis: `asin(component / |v|)`.
fn degrees(component: f32, length: f32) -> f32 {
    crate::math::asin((component / length).clamp(-1., 1.)) * (180. / std::f32::consts::PI)
}

/// A tilt stick's reading for gravity `g` against neutral `n` (LLP 1133.013
/// D6): the roll and pitch from the neutral in degrees, `(−(asin(g.x/|g|) −
/// asin(n.x/|n|)), −(asin(g.y/|g|) − asin(n.y/|n|))) / max_degrees`, then the
/// radial dead zone of `dead_zone / max_degrees` and the rescaling a pad's
/// stick takes, so the edge of the zone reads zero and `max_degrees` reads
/// one, clamped to the unit circle. Zero for a zero or non-finite vector.
pub fn tilt_stick(g: Vec3, n: Vec3, tilt: Tilt) -> Vec2 {
    let (lg, ln) = (g.length(), n.length());
    if !(lg > 0. && ln > 0. && g.is_finite() && n.is_finite()) || tilt.max_degrees <= 0. {
        return Vec2::ZERO;
    }
    let raw = Vec2::new(
        -(degrees(g.x, lg) - degrees(n.x, ln)),
        -(degrees(g.y, lg) - degrees(n.y, ln)),
    ) / tilt.max_degrees;
    pad::shape(
        raw,
        (tilt.dead_zone / tilt.max_degrees).clamp(0., 1.),
        pad::Curve::Linear,
    )
}

/// One tick's sample of a sensor.
#[derive(Clone, Debug)]
pub(crate) struct TickSample {
    sensor: SensorKind,
    sample: SensorSample,
}

impl Input {
    /// The latest sample of `sensor` at or before this tick's end. It holds
    /// through ticks that receive none (a gap: the device is still the
    /// input), and is gone from the input's end until its next sample.
    pub fn sensor(&self, sensor: SensorKind) -> Option<SensorSample> {
        (self.sensors.readings.iter())
            .find(|r| r.sensor == sensor)
            .map(|r| r.sample)
    }
    /// This tick's own samples of `sensor`, oldest first: every one the host
    /// delivered whose stamp falls in the tick's window.
    pub fn sensor_samples(&self, sensor: SensorKind) -> impl Iterator<Item = SensorSample> + '_ {
        (self.tick_samples.iter())
            .filter(move |s| s.sensor == sensor)
            .map(|s| s.sample)
    }
    /// The gravity a tilt stick of `action` reads as level: the game's
    /// (`World::set_tilt_neutral`), else the first gravity sample since the
    /// input began.
    pub fn tilt_neutral(&self, action: &str) -> Option<Vec3> {
        (self.sensors.neutrals.iter())
            .find(|n| n.action == action)
            .map(|n| n.gravity)
            .or(self.sensors.first_gravity)
            .and_then(gravity)
    }
    /// A tilt stick's contribution: nothing before the first gravity sample
    /// and from an end until the next.
    pub(super) fn tilt(&self, action: &str, tilt: Tilt) -> Vec2 {
        let (Some(g), Some(n)) = (self.sensor(SensorKind::Gravity), self.tilt_neutral(action))
        else {
            return Vec2::ZERO;
        };
        let g = Vec3::new(g.values[0], g.values[1], g.values[2]);
        tilt_stick(g, n, tilt)
    }
    pub(super) fn apply_sample(
        &mut self,
        sensor: SensorKind,
        values: [f32; 4],
        dropped: u32,
        world_us: i64,
    ) {
        let sample = SensorSample {
            values,
            dropped,
            world_us,
        };
        let readings = &mut self.sensors.readings;
        match readings.binary_search_by(|r| r.sensor.cmp(&sensor)) {
            Ok(i) => readings[i].sample = sample,
            Err(i) => readings.insert(i, Reading { sensor, sample }),
        }
        if sensor == SensorKind::Gravity && self.sensors.first_gravity.is_none() {
            self.sensors.first_gravity = Some([values[0], values[1], values[2]]);
        }
        self.tick_samples.push(TickSample { sensor, sample });
    }
    pub(super) fn end_sensor(&mut self, sensor: SensorKind) {
        self.sensors.readings.retain(|r| r.sensor != sensor);
        if sensor == SensorKind::Gravity {
            self.sensors.first_gravity = None;
        }
    }
    pub(crate) fn sensor_state(&self) -> &SensorState {
        &self.sensors
    }
    pub(crate) fn load_sensors(&mut self, state: SensorState) {
        self.sensors = state;
    }
}

impl World {
    /// Make `gravity` (a reading of the gravity sensor, in the frame the input
    /// asked for) the level of tilt stick `action`, from the next tick: a
    /// player who holds the device at an angle and sets level stands still
    /// there. It holds until set again, through the input's ends and starts.
    /// A zero or non-finite vector is refused in the journal.
    pub fn set_tilt_neutral(&mut self, action: &str, gravity: Vec3) {
        if !(gravity.is_finite() && gravity.length_squared() > 0.) {
            self.log(format_args!(
                "tilt neutral for {action} refused: {gravity} is not a finite, nonzero vector"
            ));
            return;
        }
        if self.try_resource::<SensorState>().is_none() {
            self.insert_resource(SensorState::default());
        }
        let mut state = self.resource_mut::<SensorState>();
        let neutral = Neutral {
            action: action.into(),
            gravity: gravity.to_array(),
        };
        match (state.neutrals).binary_search_by(|n| n.action.as_str().cmp(action)) {
            Ok(i) => state.neutrals[i] = neutral,
            Err(i) => state.neutrals.insert(i, neutral),
        }
    }
}
