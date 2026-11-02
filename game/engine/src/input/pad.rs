//! Gamepads in the W3C Gamepad API's standard mapping
//! (<https://w3c.github.io/gamepad/#remapping>): buttons by position, sticks
//! as axes, read on world time from the events a host reported.
use crate::{Data, Vec2};

/// Pad buttons bind as keys of these names, by standard-mapping index:
/// `.button("jump", &["Space", "GamepadSouth"])`. They name positions, not
/// letters, so an Xbox pad's A and a Nintendo pad's B are both `GamepadSouth`.
pub const PAD_BUTTONS: [&str; 17] = [
    "GamepadSouth",
    "GamepadEast",
    "GamepadWest",
    "GamepadNorth",
    "GamepadL1",
    "GamepadR1",
    "GamepadL2",
    "GamepadR2",
    "GamepadSelect",
    "GamepadStart",
    "GamepadL3",
    "GamepadR3",
    "GamepadUp",
    "GamepadDown",
    "GamepadLeft",
    "GamepadRight",
    "GamepadHome",
];
/// Pads a host may report at once, indexed from zero.
pub const MAX_PADS: u32 = 16;
/// A pad button is down at or past half its travel; digital buttons report 0 or 1.
pub(crate) const PAD_DOWN: f32 = 0.5;
/// A stick's radial dead zone when a game names none.
pub(crate) const DEAD_ZONE: f32 = 0.15;

/// The standard-mapping index of a pad button's name.
pub(crate) fn pad_index(code: &str) -> Option<usize> {
    PAD_BUTTONS.iter().position(|name| *name == code)
}
/// Refuse a `Gamepad…` name the standard mapping does not have.
pub(crate) fn check_code(code: &str) {
    assert!(
        !code.starts_with("Gamepad") || pad_index(code).is_some(),
        "unknown pad button `{code}`; pad buttons: {}",
        PAD_BUTTONS.join(", ")
    );
}

/// A pad's analog control, read by a `Stick`: `Stick::wasd().or_pad(Pad::Left)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Pad {
    /// The left stick (standard axes 0 and 1).
    #[default]
    Left,
    /// The right stick (axes 2 and 3).
    Right,
    /// The two analog triggers as one axis: forward is `GamepadR2`'s travel
    /// less `GamepadL2`'s, as a throttle and a brake.
    Triggers,
}

/// How a pad stick's magnitude past the dead zone becomes the action's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub enum Curve {
    /// Proportional.
    #[default]
    Linear,
    /// Magnitude raised to this positive power: above 1, finer control near the centre.
    Power(f32),
}

/// One connected pad as the host last read it: what is saved, before any dead
/// zone or curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub struct PadState {
    /// The host's device index.
    pub index: u32,
    /// False for a pad the host cannot read in the standard mapping: connected,
    /// delivering nothing.
    pub standard: bool,
    /// Button values in `[0, 1]`, by [`PAD_BUTTONS`] index.
    pub buttons: [f32; 17],
    /// Left x, left y, right x, right y in `[-1, 1]`; positive y is down, as
    /// the Gamepad API reports it.
    pub axes: [f32; 4],
}
impl PadState {
    pub(crate) fn down(&self, button: usize) -> bool {
        self.buttons[button] >= PAD_DOWN
    }
    /// The control in a stick's plane: positive y is forward.
    pub(crate) fn stick(&self, pad: Pad) -> Vec2 {
        match pad {
            Pad::Left => Vec2::new(self.axes[0], -self.axes[1]),
            Pad::Right => Vec2::new(self.axes[2], -self.axes[3]),
            Pad::Triggers => Vec2::new(0., self.buttons[7] - self.buttons[6]),
        }
    }
    pub(crate) fn valid(&self) -> Result<(), String> {
        valid(self.index, &self.buttons, &self.axes)
    }
}
pub(crate) fn valid(index: u32, buttons: &[f32; 17], axes: &[f32; 4]) -> Result<(), String> {
    if index >= MAX_PADS {
        return Err(format!(
            "pad index {index} is past the {MAX_PADS} a host may report"
        ));
    }
    if !buttons.iter().all(|v| (0.0..=1.0).contains(v)) {
        return Err(format!("pad {index}: button values must be in [0, 1]"));
    }
    if !axes.iter().all(|v| (-1.0..=1.0).contains(v)) {
        return Err(format!("pad {index}: axes must be in [-1, 1]"));
    }
    Ok(())
}
/// A radial dead zone, rescaled so the edge of the zone reads zero and full
/// deflection reads one, then the curve on the magnitude.
pub(crate) fn shape(v: Vec2, dead_zone: f32, curve: Curve) -> Vec2 {
    let magnitude = v.length();
    if magnitude <= dead_zone || magnitude == 0. {
        return Vec2::ZERO;
    }
    let mut t = (magnitude.min(1.) - dead_zone) / (1. - dead_zone);
    if let Curve::Power(p) = curve {
        t = crate::math::powf(t, p);
    }
    v * (t / magnitude)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dead_zone_rescales_and_the_curve_shapes_the_magnitude() {
        let east = Vec2::new(1., 0.);
        assert_eq!(shape(east * 0.1, 0.15, Curve::Linear), Vec2::ZERO);
        assert_eq!(shape(east, 0.15, Curve::Linear), east);
        let half = shape(east * 0.575, 0.15, Curve::Linear);
        assert!((half.x - 0.5).abs() < 1e-6 && half.y == 0.);
        let shaped = shape(east * 0.575, 0.15, Curve::Power(2.));
        assert!((shaped.x - 0.25).abs() < 1e-6);
        // Radial: a diagonal past the zone keeps its direction.
        let diagonal = shape(Vec2::new(0.6, 0.6), 0.15, Curve::Linear);
        assert!((diagonal.x - diagonal.y).abs() < 1e-7 && diagonal.x > 0.);
        assert!(valid(16, &[0.; 17], &[0.; 4]).is_err());
        assert!(valid(0, &[1.5; 17], &[0.; 4]).is_err());
        assert!(valid(0, &[0.; 17], &[f32::NAN; 4]).is_err());
    }
}
