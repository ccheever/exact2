//! XInput's four slots read into the W3C Gamepad API's standard mapping
//! (<https://w3c.github.io/gamepad/#remapping>), and when to read them. XInput's
//! A is the south button, its thumbsticks' y is negated so down is positive,
//! and its triggers' 0–255 travel is 0–1. The XInput call is `pads.rs`'s
//! (Windows only); this is not, so its tests run on any host.
//! <https://learn.microsoft.com/windows/win32/api/xinput/nf-xinput-xinputgetstate>
use std::time::{Duration, Instant};

/// XInput's button bits by standard index; the guide button (16) is not XInput's.
const BUTTONS: [(usize, u16); 14] = [
    (0, 0x1000),  // A
    (1, 0x2000),  // B
    (2, 0x4000),  // X
    (3, 0x8000),  // Y
    (4, 0x0100),  // left shoulder
    (5, 0x0200),  // right shoulder
    (8, 0x0020),  // back
    (9, 0x0010),  // start
    (10, 0x0040), // left thumb
    (11, 0x0080), // right thumb
    (12, 0x0001), // d-pad up
    (13, 0x0002), // d-pad down
    (14, 0x0004), // d-pad left
    (15, 0x0008), // d-pad right
];
/// How often an empty slot is asked about. Microsoft advises against calling
/// XInputGetState on an empty slot every frame, as it is slow
/// (<https://learn.microsoft.com/windows/win32/xinput/getting-started-with-xinput>);
/// a pad that connects is seen within this.
pub(crate) const EMPTY_EVERY: Duration = Duration::from_millis(250);
/// How often a connected pad is read while the window has focus.
pub(crate) const CONNECTED_EVERY: Duration = Duration::from_millis(8);

/// A slot's `XINPUT_GAMEPAD`, as XInput reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Raw {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    /// Left x, left y, right x, right y; positive y is up.
    pub thumbs: [i16; 4],
}
fn quantize(v: f32, low: f32) -> f32 {
    ((v * 256.).round() / 256.).clamp(low, 1.) + 0.
}
pub(crate) type Reading = ([f32; 17], [f32; 4]);
pub(crate) fn reading(raw: &Raw) -> Reading {
    let mut buttons = [0f32; 17];
    for (index, bit) in BUTTONS {
        buttons[index] = f32::from(u8::from(raw.buttons & bit != 0));
    }
    buttons[6] = quantize(f32::from(raw.left_trigger) / 255., 0.);
    buttons[7] = quantize(f32::from(raw.right_trigger) / 255., 0.);
    let axis = |v: i16| quantize((f32::from(v) / 32767.).max(-1.), -1.);
    let [lx, ly, rx, ry] = raw.thumbs;
    let axes = [axis(lx), -axis(ly) + 0., axis(rx), -axis(ry) + 0.];
    (buttons, axes)
}

/// What the window last sent for a slot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Slot {
    /// Nothing since the window last had focus: the next read is sent,
    /// connected or not.
    #[default]
    Unknown,
    Empty,
    Connected(Reading),
}
/// The four slots, as last sent.
#[derive(Default)]
pub(crate) struct Pads {
    slots: [Slot; 4],
    empty_read: Option<Instant>,
}
impl Pads {
    /// Whether a pad is connected, so the window polls often.
    pub(crate) fn any(&self) -> bool {
        self.slots.iter().any(|s| matches!(s, Slot::Connected(_)))
    }
    /// The window lost focus and stops reading: every slot is unknown, so the
    /// first poll after focus returns reads all four and reports each, a pad
    /// unplugged meanwhile as disconnected.
    pub(crate) fn forget(&mut self) {
        self.slots = [Slot::Unknown; 4];
        self.empty_read = None;
    }
    /// When the window should wake to poll again.
    pub(crate) fn next_poll(&self, now: Instant) -> Instant {
        now + if self.any() {
            CONNECTED_EVERY
        } else {
            EMPTY_EVERY
        }
    }
    /// Read the slots through `read` (XInputGetState; `None` for an empty
    /// slot) and `send` each that changed: its index, whether connected, and
    /// its reading. Connected and unknown slots are read every poll, empty
    /// ones every `EMPTY_EVERY`.
    pub(crate) fn poll(
        &mut self,
        now: Instant,
        mut read: impl FnMut(u8) -> Option<Raw>,
        mut send: impl FnMut(u8, bool, [f32; 17], [f32; 4]),
    ) {
        let empty = self.empty_read.is_none_or(|at| now - at >= EMPTY_EVERY);
        if empty {
            self.empty_read = Some(now);
        }
        for slot in 0..4u8 {
            let i = usize::from(slot);
            if self.slots[i] == Slot::Empty && !empty {
                continue;
            }
            let next = read(slot).map_or(Slot::Empty, |raw| Slot::Connected(reading(&raw)));
            if next != self.slots[i] {
                self.slots[i] = next;
                let (connected, (buttons, axes)) = match next {
                    Slot::Connected(r) => (true, r),
                    _ => (false, Default::default()),
                };
                send(slot, connected, buttons, axes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Sent = Vec<(u8, bool, f32)>;
    fn poll(pads: &mut Pads, now: Instant, plugged: &[Option<Raw>; 4]) -> Sent {
        let mut sent = Vec::new();
        pads.poll(
            now,
            |slot| plugged[usize::from(slot)],
            |i, c, b, _| sent.push((i, c, b[0])),
        );
        sent
    }
    const A: Raw = Raw {
        buttons: 0x1000,
        left_trigger: 0,
        right_trigger: 0,
        thumbs: [0; 4],
    };
    #[test]
    fn a_pad_unplugged_while_unfocused_is_reported_gone_on_return() {
        let t = Instant::now();
        let mut pads = Pads::default();
        let held = [Some(A), None, None, None];
        assert_eq!(
            poll(&mut pads, t, &held),
            [
                (0, true, 1.),
                (1, false, 0.),
                (2, false, 0.),
                (3, false, 0.)
            ]
        );
        pads.forget(); // blur; the pad is unplugged before focus returns
        let none = [None; 4];
        assert_eq!(
            poll(&mut pads, t + Duration::from_millis(1), &none)[0],
            (0, false, 0.)
        );
    }
    #[test]
    fn the_first_poll_after_focus_reads_every_slot_at_once() {
        let t = Instant::now();
        let mut pads = Pads::default();
        poll(&mut pads, t, &[None; 4]);
        // A pad that connects is seen within EMPTY_EVERY, not before.
        let plugged = [None, Some(A), None, None];
        assert!(poll(&mut pads, t + Duration::from_millis(100), &plugged).is_empty());
        assert_eq!(poll(&mut pads, t + EMPTY_EVERY, &plugged), [(1, true, 1.)]);
        // Blur and return within EMPTY_EVERY: the held pad reports at once.
        pads.forget();
        let back = poll(
            &mut pads,
            t + EMPTY_EVERY + Duration::from_millis(10),
            &plugged,
        );
        assert_eq!(back.iter().find(|s| s.0 == 1), Some(&(1, true, 1.)));
        assert_eq!(
            back.len(),
            4,
            "every slot is reported after a blur: {back:?}"
        );
    }
    #[test]
    fn xinput_reads_into_the_standard_mapping() {
        let (buttons, axes) = reading(&Raw {
            buttons: 0x4000 | 0x0001,
            left_trigger: 255,
            right_trigger: 0,
            thumbs: [32767, 32767, -32768, 0],
        });
        assert_eq!(
            (buttons[2], buttons[12], buttons[6], buttons[16]),
            (1., 1., 1., 0.)
        );
        assert_eq!(
            axes,
            [1., -1., -1., 0.],
            "XInput's up is the standard's negative y"
        );
    }
}
