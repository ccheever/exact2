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
/// The pad's face-button layout, from its driver in sysfs and its vendor.
fn layout(path: &Path, d: &Device) -> Layout {
    let driver = crate::gamepad::driver(Path::new("/sys"), path);
    crate::gamepad::layout_for(driver.as_deref(), d.input_id().vendor())
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

/// Who the pad at `path` is, for the registry (`crate::pads`).
pub(super) fn identity(path: &Path, d: &Device) -> crate::pads::Identity {
    crate::pads::Identity {
        id: d.name().unwrap_or("Gamepad").to_owned(),
        vendor: d.input_id().vendor(),
        product: d.input_id().product(),
        driver: crate::gamepad::driver(Path::new("/sys"), path),
        user_index: None,
    }
}

/// The pad's motors, where its driver takes `FF_RUMBLE` and the device
/// opens for writing: a descriptor of their own, so the reader's is as it
/// was. A pad opened read-only lists no effects.
pub(super) fn motors(path: &Path, d: &Device) -> Option<Box<dyn crate::pads::Output>> {
    if !d.supported_ff()?.contains(evdev::FFEffectCode::FF_RUMBLE) {
        return None;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .ok()?;
    let device = Device::from_fd(file.into()).ok()?;
    Some(Box::new(Motors {
        device,
        effect: None,
    }))
}

/// A pad's two motors through evdev's force feedback
/// (<https://docs.kernel.org/input/ff.html>): one `FF_RUMBLE` effect at a
/// time, removed from the device when the next replaces it.
struct Motors {
    device: Device,
    effect: Option<evdev::FFEffect>,
}

impl crate::pads::Output for Motors {
    fn rumble(&mut self, strong: f32, weak: f32, ms: u32) -> bool {
        self.effect = None;
        let magnitude = |m: f32| (m.clamp(0., 1.) * f32::from(u16::MAX)) as u16;
        let data = evdev::FFEffectData {
            direction: 0,
            trigger: evdev::FFTrigger::default(),
            replay: evdev::FFReplay {
                length: ms.min(u32::from(u16::MAX)) as u16,
                delay: 0,
            },
            kind: evdev::FFEffectKind::Rumble {
                strong_magnitude: magnitude(strong),
                weak_magnitude: magnitude(weak),
            },
        };
        let Ok(mut effect) = self.device.upload_ff_effect(data) else {
            return false;
        };
        let played = effect.play(1).is_ok();
        self.effect = Some(effect);
        played
    }

    fn stop(&mut self) {
        if let Some(mut effect) = self.effect.take() {
            let _ = effect.stop();
        }
    }
}
