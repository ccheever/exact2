//! Gamepads through XInput (LLP 1127.003 D2), reported in the W3C Gamepad
//! API's standard mapping (<https://w3c.github.io/gamepad/#remapping>):
//! XInput's A is the south button, its thumbsticks' y is negated so down is
//! positive, and its triggers' 0–255 travel is 0–1. XInput has four user
//! slots, each a pad index; a slot that empties is reported disconnected.
//! <https://learn.microsoft.com/windows/win32/api/xinput/nf-xinput-xinputgetstate>
#![allow(unsafe_code)]

use windows_sys::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_GAMEPAD, XINPUT_STATE};

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
/// An empty slot is asked about once in this many polls: XInputGetState on a
/// slot with no pad is slow.
const EMPTY_EVERY: u32 = 60;

fn quantize(v: f32, low: f32) -> f32 {
    ((v * 256.).round() / 256.).clamp(low, 1.) + 0.
}
type Reading = ([f32; 17], [f32; 4]);
fn reading(g: &XINPUT_GAMEPAD) -> Reading {
    let mut buttons = [0f32; 17];
    for (index, bit) in BUTTONS {
        buttons[index] = f32::from(u8::from(g.wButtons & bit != 0));
    }
    buttons[6] = quantize(f32::from(g.bLeftTrigger) / 255., 0.);
    buttons[7] = quantize(f32::from(g.bRightTrigger) / 255., 0.);
    let axis = |v: i16| quantize((f32::from(v) / 32767.).max(-1.), -1.);
    let axes = [
        axis(g.sThumbLX),
        -axis(g.sThumbLY) + 0.,
        axis(g.sThumbRX),
        -axis(g.sThumbRY) + 0.,
    ];
    (buttons, axes)
}

/// The four slots' last readings.
#[derive(Default)]
pub(crate) struct Pads {
    last: [Option<Reading>; 4],
    polls: u32,
}
impl Pads {
    /// Whether a pad is connected, so the window keeps polling.
    pub(crate) fn any(&self) -> bool {
        self.last.iter().any(Option::is_some)
    }
    /// Forget what was sent: after a blur every pad reports itself again.
    pub(crate) fn forget(&mut self) {
        self.last = [None; 4];
    }
    /// Each slot that changed: its index, whether connected, and its reading.
    pub(crate) fn poll(&mut self, mut send: impl FnMut(u8, bool, [f32; 17], [f32; 4])) {
        self.polls = self.polls.wrapping_add(1);
        for slot in 0..4u8 {
            let i = usize::from(slot);
            if self.last[i].is_none() && self.polls % EMPTY_EVERY != 1 {
                continue;
            }
            // SAFETY: XInputGetState writes one XINPUT_STATE through a valid,
            // exclusively borrowed pointer.
            let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
            let connected = unsafe { XInputGetState(u32::from(slot), &mut state) } == 0;
            let now = connected.then(|| reading(&state.Gamepad));
            if now != self.last[i] {
                self.last[i] = now;
                let (buttons, axes) = now.unwrap_or_default();
                send(slot, connected, buttons, axes);
            }
        }
    }
}
