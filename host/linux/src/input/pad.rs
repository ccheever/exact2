//! Gamepads from evdev (`crate::gamepad` reads their codes): which devices are
//! pads, their axis ranges, their face-button layout, and their state at open.
//! Opened at start with the other devices; a pad that goes away is reported
//! disconnected, and none arrives later (no hotplug).
use crate::gamepad::{Layout, State, AXES};
use evdev::{AbsoluteAxisCode, Device};
use std::path::Path;

/// Pads reported at once, indexed from zero.
pub const MAX_PADS: usize = 16;

/// Whether a device is a gamepad: it has the south face button.
pub(super) fn is_pad(d: &Device) -> bool {
    d.supported_keys()
        .is_some_and(|k| k.contains(evdev::KeyCode::BTN_SOUTH))
}
/// A pad's axis ranges from its absolute state, by `AXES`.
fn ranges(d: &Device) -> [Option<(i32, i32)>; 8] {
    let supported = d.supported_absolute_axes();
    let state = d.get_abs_state().ok();
    AXES.map(|code| {
        let info = state.as_ref()?[code as usize];
        (supported?.contains(AbsoluteAxisCode(code)) && info.maximum > info.minimum)
            .then_some((info.minimum, info.maximum))
    })
}
/// Xbox drivers name the face buttons by letter: the device's driver
/// (`/sys/class/input/eventN/device/device/driver`) is `xpad` or `xone…`, or
/// its vendor is Microsoft's (0x045e, hid-microsoft).
fn layout(path: &Path, d: &Device) -> Layout {
    let driver = path
        .file_name()
        .map(|event| {
            Path::new("/sys/class/input")
                .join(event)
                .join("device/device/driver")
        })
        .and_then(|link| std::fs::read_link(link).ok())
        .and_then(|target| target.file_name().map(|n| n.to_string_lossy().into_owned()));
    let xbox = driver.is_some_and(|name| name == "xpad" || name.starts_with("xone"))
        || d.input_id().vendor() == 0x045e;
    if xbox {
        Layout::Xbox
    } else {
        Layout::Positional
    }
}
/// A pad as it is when opened: its held buttons and its axes, so the first
/// reading is the device's, not a centred default.
pub(super) fn open(path: &Path, d: &Device) -> State {
    let mut state = State::new(ranges(d), layout(path, d));
    if let Ok(keys) = d.get_key_state() {
        for key in keys.iter() {
            state.key(key.0, 1);
        }
    }
    if let Ok(abs) = d.get_abs_state() {
        for code in AXES {
            state.axis(code, abs[code as usize].value);
        }
    }
    state
}
