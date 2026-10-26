//! Gamepads through XInput (LLP 1127.003 D2): the call that reads a slot.
//! What a slot's state means, and when to read it, is `pad_poll.rs`'s.
#![allow(unsafe_code)]

use crate::pad_poll::Raw;
use windows_sys::Win32::UI::Input::XboxController::{
    XInputGetState, XInputSetState, XINPUT_STATE, XINPUT_VIBRATION,
};

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

/// A slot's change, told to the pad registry a package reads
/// (`exact_linux::pads`): the same pads by index as the canvas's.
pub(crate) fn register(slot: u8, connected: bool, buttons: [f32; 17], axes: [f32; 4]) {
    use exact_linux::pads;
    if !connected {
        return pads::disconnect(slot);
    }
    if !pads::list().0.iter().any(|p| p.index == slot) {
        let identity = pads::Identity {
            id: "Xbox Controller (XInput)".into(),
            user_index: Some(slot),
            ..Default::default()
        };
        pads::connect(slot, identity, Some(Box::new(Motors(slot))));
    }
    pads::read(slot, exact_linux::gamepad::Reading { buttons, axes });
}

/// A slot's two motors: XInput's left is the low-frequency (strong) one
/// and runs until set again, so the registry stops it at its span's end.
/// <https://learn.microsoft.com/windows/win32/api/xinput/nf-xinput-xinputsetstate>
struct Motors(u8);

impl exact_linux::pads::Output for Motors {
    fn rumble(&mut self, strong: f32, weak: f32, _ms: u32) -> bool {
        let speed = |m: f32| (m.clamp(0., 1.) * f32::from(u16::MAX)) as u16;
        set(self.0, speed(strong), speed(weak))
    }
    fn stop(&mut self) {
        set(self.0, 0, 0);
    }
}

fn set(slot: u8, left: u16, right: u16) -> bool {
    let vibration = XINPUT_VIBRATION {
        wLeftMotorSpeed: left,
        wRightMotorSpeed: right,
    };
    // SAFETY: XInputSetState reads one XINPUT_VIBRATION through a valid pointer.
    unsafe { XInputSetState(u32::from(slot), &vibration) == 0 }
}
