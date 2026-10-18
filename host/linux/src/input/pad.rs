//! Gamepads from evdev, read in the kernel's gamepad layout
//! (<https://docs.kernel.org/input/gamepad.html>) and reported in the W3C
//! Gamepad API's standard mapping (<https://w3c.github.io/gamepad/#remapping>):
//! buttons by position, sticks in `[-1, 1]` with positive y down, analog
//! triggers in `[0, 1]`. Opened at start with the other devices; a pad that
//! goes away is reported disconnected, and none arrives later (no hotplug).
use evdev::{AbsoluteAxisCode, Device};

/// Pads reported at once, indexed from zero.
pub const MAX_PADS: usize = 16;
/// Values travel in 1/256 steps: exact in an f32, and noise below that is no change.
fn quantize(v: f32, low: f32) -> f32 {
    ((v * 256.).round() / 256.).clamp(low, 1.) + 0. // + 0: no -0
}

/// A pad's state in the standard mapping.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Reading {
    /// Button values by standard index.
    pub buttons: [f32; 17],
    /// Left x, left y, right x, right y.
    pub axes: [f32; 4],
}

/// The standard index of an evdev gamepad button. `BTN_NORTH` is `BTN_X` and
/// `BTN_WEST` is `BTN_Y` in the kernel's codes.
fn button(code: u16) -> Option<usize> {
    Some(match code {
        0x130 => 0,  // BTN_SOUTH
        0x131 => 1,  // BTN_EAST
        0x134 => 2,  // BTN_WEST
        0x133 => 3,  // BTN_NORTH
        0x136 => 4,  // BTN_TL
        0x137 => 5,  // BTN_TR
        0x138 => 6,  // BTN_TL2, a digital trigger
        0x139 => 7,  // BTN_TR2
        0x13a => 8,  // BTN_SELECT
        0x13b => 9,  // BTN_START
        0x13d => 10, // BTN_THUMBL
        0x13e => 11, // BTN_THUMBR
        0x220 => 12, // BTN_DPAD_UP
        0x221 => 13, // BTN_DPAD_DOWN
        0x222 => 14, // BTN_DPAD_LEFT
        0x223 => 15, // BTN_DPAD_RIGHT
        0x13c => 16, // BTN_MODE
        _ => return None,
    })
}
/// The absolute axes a pad reports, in the order `State::ranges` keeps them.
const AXES: [AbsoluteAxisCode; 8] = [
    AbsoluteAxisCode::ABS_X,
    AbsoluteAxisCode::ABS_Y,
    AbsoluteAxisCode::ABS_RX,
    AbsoluteAxisCode::ABS_RY,
    AbsoluteAxisCode::ABS_Z,
    AbsoluteAxisCode::ABS_RZ,
    AbsoluteAxisCode::ABS_HAT0X,
    AbsoluteAxisCode::ABS_HAT0Y,
];

/// A pad's ranges and reading, apart from its device so it can be tested.
#[derive(Debug, Clone, Default)]
pub(super) struct State {
    ranges: [Option<(i32, i32)>; 8],
    pub(super) reading: Reading,
}
impl State {
    pub(super) fn new(ranges: [Option<(i32, i32)>; 8]) -> Self {
        Self {
            ranges,
            reading: Reading::default(),
        }
    }
    /// A key edge; true when the reading changed.
    pub(super) fn key(&mut self, code: u16, value: i32) -> bool {
        let Some(b) = button(code) else {
            return false;
        };
        self.set(b, if value != 0 { 1. } else { 0. }, true)
    }
    /// An absolute axis; true when the reading changed.
    pub(super) fn axis(&mut self, code: u16, value: i32) -> bool {
        let Some(slot) = AXES.iter().position(|a| a.0 == code) else {
            return false;
        };
        let Some((min, max)) = self.ranges[slot] else {
            return false;
        };
        let unit = (value - min) as f32 / (max - min).max(1) as f32;
        match slot {
            0..=3 => {
                let v = quantize(unit * 2. - 1., -1.);
                let changed = self.reading.axes[slot] != v;
                self.reading.axes[slot] = v;
                changed
            }
            4 | 5 => self.set(slot + 2, quantize(unit, 0.), false),
            // A hat is the d-pad: -1, 0 or 1 on each axis.
            _ => {
                let (negative, positive) = if slot == 6 { (14, 15) } else { (12, 13) };
                let n = self.set(negative, if value < 0 { 1. } else { 0. }, false);
                self.set(positive, if value > 0 { 1. } else { 0. }, false) || n
            }
        }
    }
    // A digital trigger edge does not override an analog one's travel.
    fn set(&mut self, b: usize, v: f32, digital: bool) -> bool {
        if digital && (b == 6 || b == 7) && self.ranges[b - 2].is_some() {
            return false;
        }
        let changed = self.reading.buttons[b] != v;
        self.reading.buttons[b] = v;
        changed
    }
}
/// Whether a device is a gamepad: it has the south face button.
pub(super) fn is_pad(d: &Device) -> bool {
    d.supported_keys()
        .is_some_and(|k| k.contains(evdev::KeyCode::BTN_SOUTH))
}
/// A pad's axis ranges from its absolute state.
pub(super) fn ranges(d: &Device) -> [Option<(i32, i32)>; 8] {
    let supported = d.supported_absolute_axes();
    let state = d.get_abs_state().ok();
    AXES.map(|a| {
        let info = state.as_ref()?[a.0 as usize];
        (supported?.contains(a) && info.maximum > info.minimum)
            .then_some((info.minimum, info.maximum))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evdev_codes_map_to_the_standard_layout() {
        // Sticks 0..255 centred at 128, triggers 0..1023, a hat d-pad.
        let mut s = State::new([
            Some((0, 255)),
            Some((0, 255)),
            Some((0, 255)),
            Some((0, 255)),
            Some((0, 1023)),
            Some((0, 1023)),
            Some((-1, 1)),
            Some((-1, 1)),
        ]);
        assert!(s.key(0x130, 1) && s.reading.buttons[0] == 1.);
        assert!(
            s.key(0x133, 1) && s.reading.buttons[3] == 1.,
            "BTN_NORTH is BTN_X"
        );
        assert!(!s.key(0x130, 1), "no change, no event");
        assert!(s.axis(0x00, 255) && s.reading.axes[0] == 1.);
        assert!(
            s.axis(0x01, 0) && s.reading.axes[1] == -1.,
            "up is negative, as on the web"
        );
        s.axis(0x00, 128);
        assert_eq!(s.reading.axes[0], quantize(256. / 255. - 1., -1.));
        assert!(s.axis(0x05, 1023) && s.reading.buttons[7] == 1.);
        assert!(
            !s.key(0x139, 0),
            "an analog trigger's digital edge is ignored"
        );
        assert!(s.axis(0x11, -1) && s.reading.buttons[12] == 1.);
        s.axis(0x11, 1);
        assert_eq!((s.reading.buttons[12], s.reading.buttons[13]), (0., 1.));
        assert!(!s.axis(0x2f, 3), "an axis a pad does not use");
    }
}
