//! A gamepad's evdev codes read into the W3C Gamepad API's standard mapping
//! (<https://w3c.github.io/gamepad/#remapping>): buttons by position, sticks
//! in `[-1, 1]` with positive y down, analog triggers in `[0, 1]`. The kernel's
//! gamepad layout (<https://docs.kernel.org/input/gamepad.html>) names the face
//! buttons by position; Xbox drivers (xpad, xone, hid-microsoft) send the codes
//! of their printed letters instead, so `Layout::Xbox` reads `BTN_X` as the
//! west button. The device reader (`input/pad.rs`) is Linux's; this is not, so
//! its tests run on any host.

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

/// How a driver names the face buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// By position: `BTN_NORTH` (`BTN_X`) is the top button.
    #[default]
    Positional,
    /// By an Xbox pad's letters: `BTN_X` is X, the west button; `BTN_Y` is Y, the north.
    Xbox,
}

/// One evdev event a pad reported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    /// A key edge: code and value.
    Key(u16, i32),
    /// An absolute axis: code and value.
    Abs(u16, i32),
    /// `SYN_REPORT`: the device's report is complete.
    Report,
}

/// The standard index of an evdev gamepad button.
fn button(code: u16, layout: Layout) -> Option<usize> {
    Some(match (code, layout) {
        (0x130, _) => 0,                  // BTN_SOUTH, BTN_A
        (0x131, _) => 1,                  // BTN_EAST, BTN_B
        (0x134, Layout::Positional) => 2, // BTN_WEST
        (0x133, Layout::Positional) => 3, // BTN_NORTH
        (0x133, Layout::Xbox) => 2,       // BTN_X: Xbox X, on the west
        (0x134, Layout::Xbox) => 3,       // BTN_Y: Xbox Y, on the north
        (0x136, _) => 4,                  // BTN_TL
        (0x137, _) => 5,                  // BTN_TR
        (0x138, _) => 6,                  // BTN_TL2, a digital trigger
        (0x139, _) => 7,                  // BTN_TR2
        (0x13a, _) => 8,                  // BTN_SELECT
        (0x13b, _) => 9,                  // BTN_START
        (0x13d, _) => 10,                 // BTN_THUMBL
        (0x13e, _) => 11,                 // BTN_THUMBR
        (0x220, _) => 12,                 // BTN_DPAD_UP
        (0x221, _) => 13,                 // BTN_DPAD_DOWN
        (0x222, _) => 14,                 // BTN_DPAD_LEFT
        (0x223, _) => 15,                 // BTN_DPAD_RIGHT
        (0x13c, _) => 16,                 // BTN_MODE
        _ => return None,
    })
}
/// The absolute axes a pad reports, in the order `State`'s ranges keep them:
/// ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_Z, ABS_RZ, ABS_HAT0X, ABS_HAT0Y.
pub const AXES: [u16; 8] = [0x00, 0x01, 0x03, 0x04, 0x02, 0x05, 0x10, 0x11];

/// A pad's ranges, layout and reading.
#[derive(Debug, Clone, Default)]
pub struct State {
    ranges: [Option<(i32, i32)>; 8],
    layout: Layout,
    /// The reading after the last event.
    pub reading: Reading,
}
impl State {
    /// A pad with these axis ranges (by [`AXES`]) and face-button layout.
    pub fn new(ranges: [Option<(i32, i32)>; 8], layout: Layout) -> Self {
        Self {
            ranges,
            layout,
            reading: Reading::default(),
        }
    }
    /// A key edge; true when the reading changed.
    pub fn key(&mut self, code: u16, value: i32) -> bool {
        let Some(b) = button(code, self.layout) else {
            return false;
        };
        self.set(b, if value != 0 { 1. } else { 0. }, true)
    }
    /// An absolute axis; true when the reading changed.
    pub fn axis(&mut self, code: u16, value: i32) -> bool {
        let Some(slot) = AXES.iter().position(|a| *a == code) else {
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
    /// One fetched batch: a reading at each report that changed it, so a
    /// press and release in one read are two readings, in order.
    pub fn events(&mut self, changes: impl IntoIterator<Item = Change>) -> Vec<Reading> {
        let (mut out, mut changed) = (Vec::new(), false);
        for change in changes {
            match change {
                Change::Key(code, value) => changed |= self.key(code, value),
                Change::Abs(code, value) => changed |= self.axis(code, value),
                Change::Report => {
                    if std::mem::take(&mut changed) {
                        out.push(self.reading);
                    }
                }
            }
        }
        if changed {
            out.push(self.reading);
        }
        out
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

#[cfg(test)]
mod tests {
    use super::*;
    fn device(layout: Layout) -> State {
        // Sticks 0..255 centred at 128, triggers 0..1023, a hat d-pad.
        let stick = Some((0, 255));
        State::new(
            [
                stick,
                stick,
                stick,
                stick,
                Some((0, 1023)),
                Some((0, 1023)),
                Some((-1, 1)),
                Some((-1, 1)),
            ],
            layout,
        )
    }
    #[test]
    fn evdev_codes_map_to_the_standard_layout() {
        let mut s = device(Layout::Positional);
        assert!(s.key(0x130, 1) && s.reading.buttons[0] == 1.);
        assert!(
            s.key(0x133, 1) && s.reading.buttons[3] == 1.,
            "BTN_NORTH is the top"
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
    /// An Xbox pad under xpad: BTN_X (0x133) is its X, on the west; BTN_Y (0x134) its Y.
    #[test]
    fn an_xbox_pads_letters_are_its_positions() {
        let mut s = device(Layout::Xbox);
        s.key(0x133, 1);
        assert_eq!(
            (s.reading.buttons[2], s.reading.buttons[3]),
            (1., 0.),
            "X is GamepadWest"
        );
        s.key(0x133, 0);
        s.key(0x134, 1);
        assert_eq!(
            (s.reading.buttons[2], s.reading.buttons[3]),
            (0., 1.),
            "Y is GamepadNorth"
        );
        // A and B are positional on every layout.
        s.key(0x130, 1);
        s.key(0x131, 1);
        assert_eq!((s.reading.buttons[0], s.reading.buttons[1]), (1., 1.));
    }
    /// A press and its release in one read are two readings.
    #[test]
    fn a_buffered_tap_is_a_press_then_a_release() {
        let mut s = device(Layout::Positional);
        let readings = s.events([
            Change::Key(0x130, 1),
            Change::Report,
            Change::Key(0x130, 0),
            Change::Report,
            Change::Abs(0x00, 128),
            Change::Abs(0x00, 128),
        ]);
        let south: Vec<f32> = readings.iter().map(|r| r.buttons[0]).collect();
        assert_eq!(south, [1., 0., 0.], "{readings:?}");
        assert!(
            s.events([Change::Report]).is_empty(),
            "an empty report is no reading"
        );
    }
}
