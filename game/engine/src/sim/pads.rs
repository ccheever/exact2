//! A pad driven from a test or an agent: each change is the pad's whole state,
//! as a host reports it, stamped at the current clock.
use super::*;
use crate::{Pad, PadState, PAD_BUTTONS};

impl<G: Game> Sim<G> {
    /// A pad's state at the host boundary, including events still queued.
    pub fn pad(&self, index: u32) -> Option<PadState> {
        self.host_input().pad(index)
    }
    fn send_pad(&mut self, index: u32, change: impl FnOnce(&mut PadState)) {
        let mut pad = self.pad(index).unwrap_or(PadState {
            index,
            standard: true,
            ..PadState::default()
        });
        change(&mut pad);
        self.input(InputEvent::Pad {
            index,
            connected: true,
            standard: true,
            buttons: pad.buttons,
            axes: pad.axes,
            at_ms: self.last_us.unwrap_or(0) as f64 / 1000.0,
            player: 0,
        });
    }
    /// Set a pad button's value at the current clock (1 pressed, 0 released, a
    /// trigger anything between), connecting the pad if it is not.
    pub fn pad_button(&mut self, index: u32, code: &str, value: f32) {
        let button = PAD_BUTTONS
            .iter()
            .position(|b| *b == code)
            .unwrap_or_else(|| {
                panic!(
                    "unknown pad button `{code}`; pad buttons: {}",
                    PAD_BUTTONS.join(", ")
                )
            });
        self.send_pad(index, |p| p.buttons[button] = value);
    }
    /// Move a pad's stick at the current clock, in the Gamepad API's axes
    /// (positive y is down), connecting the pad if it is not.
    pub fn pad_stick(&mut self, index: u32, stick: Pad, x: f32, y: f32) {
        let first = match stick {
            Pad::Left => 0,
            Pad::Right => 2,
            Pad::Triggers => {
                panic!("the triggers are buttons: pad_button(index, \"GamepadR2\", value)")
            }
        };
        self.send_pad(index, |p| {
            p.axes[first] = x;
            p.axes[first + 1] = y;
        });
    }
    /// Disconnect a pad at the current clock: everything it held releases.
    pub fn pad_disconnect(&mut self, index: u32) {
        self.input(InputEvent::Pad {
            index,
            connected: false,
            standard: true,
            buttons: [0.; 17],
            axes: [0.; 4],
            at_ms: self.last_us.unwrap_or(0) as f64 / 1000.0,
            player: 0,
        });
    }
}
