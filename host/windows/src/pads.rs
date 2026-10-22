//! Gamepads through XInput (LLP 1127.003 D2): the call that reads a slot.
//! What a slot's state means, and when to read it, is `pad_poll.rs`'s.
#![allow(unsafe_code)]

use crate::pad_poll::Raw;
use windows_sys::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};

/// A slot's state, or `None` when no pad is in it.
pub(crate) fn read(slot: u8) -> Option<Raw> {
    // SAFETY: XInputGetState writes one XINPUT_STATE through a valid,
    // exclusively borrowed pointer.
    let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
    if unsafe { XInputGetState(u32::from(slot), &mut state) } != 0 {
        return None;
    }
    let g = state.Gamepad;
    Some(Raw {
        buttons: g.wButtons,
        left_trigger: g.bLeftTrigger,
        right_trigger: g.bRightTrigger,
        thumbs: [g.sThumbLX, g.sThumbLY, g.sThumbRX, g.sThumbRY],
    })
}
