//! Input: evdev, read directly — pointer motion, buttons, wheels, keys —
//! with a US keymap for typing. No libinput (nothing to link), so no
//! pointer acceleration, touchpad gestures, or hotplug: the trades LLP 1015
//! §7 declares.
//!
//! @ref LLP 1015 §6

#![allow(unsafe_code)]

use evdev::{
    AbsoluteAxisCode, Device, EventSummary, EventType, RelativeAxisCode, SynchronizationCode,
};
use std::os::fd::AsRawFd;

/// One thing the user did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    /// The pointer moved by (dx, dy) device pixels.
    Motion(f32, f32),
    /// The pointer is at a fraction of the screen (an absolute device — a
    /// tablet, a KVM's mouse): `None` on an axis that did not move.
    Absolute(Option<f32>, Option<f32>),
    /// The primary button went down (`true`) or up.
    Button(bool),
    /// The secondary (2) or middle (4) button, by its `PointerEvent.buttons`
    /// bit, went down (`true`) or up: a canvas's alone.
    Aux(u32, bool),
    /// Carrier/device loss cancels without manufacturing a successful release.
    Cancel,
    /// A wheel: (dx, dy) in points, the web's sign (a positive `dy` scrolls
    /// down).
    Wheel(f32, f32),
    /// A keyboard edge. Translate US codes at dispatch, keeping the queue compact.
    Key {
        /// Physical evdev key code, also used by VNC's US mapping.
        code: u16,
        /// Whether the logical key uses the shifted US character.
        shift: bool,
        /// Press (`true`) or release.
        down: bool,
        /// A repeated press while the key is held.
        repeat: bool,
    },
}

/// US keyboard state shared by evdev and VNC. Shift sides are independent.
/// A Ctrl/Meta chord passes: it is a `key` handler's (⌘S, kanban F27), and
/// the presenter keeps it from typing or starting a game action
/// (`hardware_key`).
#[derive(Default)]
pub(crate) struct Keyboard {
    /// The Shift keys held, a bit per side.
    shifts: u8,
}
impl Keyboard {
    pub(crate) fn event(
        &mut self,
        code: u16,
        value: i32,
        shift: Option<bool>,
    ) -> Option<InputEvent> {
        if !(0..=2).contains(&value) {
            return None;
        }
        let down = value != 0;
        let bit = match code {
            42 => 1,
            54 => 2,
            _ => 0,
        };
        if down {
            self.shifts |= bit;
        } else {
            self.shifts &= !bit;
        }
        let shift = shift.unwrap_or(self.shifts != 0);
        key(code, shift)?;
        Some(InputEvent::Key {
            code,
            shift,
            down,
            repeat: value == 2,
        })
    }
}

/// Points per wheel notch — the browser's tick.
pub const LINE: f32 = 40.0;

/// An absolute device's (min, max) per axis, x then y.
type Ranges = [(i32, i32); 2];

/// Every device that points or types.
pub struct Input {
    devices: Vec<(Device, Option<Ranges>)>,
    keyboard: Keyboard,
}

impl Input {
    /// Open every evdev device with relative axes or a keyboard's keys.
    /// Devices that cannot be opened (no permission — the `input` group) are
    /// skipped.
    pub fn open() -> Input {
        let mut devices = Vec::new();
        for (_, d) in evdev::enumerate() {
            let pointer = d.supported_events().contains(EventType::RELATIVE);
            // An absolute pointer: ABS_X/ABS_Y with their ranges (a KVM's
            // mouse, a tablet, a USB touchscreen). Multitouch slots are
            // not tracked; ABS_MT_POSITION_* is the same point as ABS_X/Y
            // on the single-contact devices we open.
            let absolute = d
                .supported_absolute_axes()
                .is_some_and(|a| {
                    a.contains(AbsoluteAxisCode::ABS_X) && a.contains(AbsoluteAxisCode::ABS_Y)
                })
                .then(|| d.get_abs_state().ok())
                .flatten()
                .map(|abs| {
                    let x = abs[AbsoluteAxisCode::ABS_X.0 as usize];
                    let y = abs[AbsoluteAxisCode::ABS_Y.0 as usize];
                    [(x.minimum, x.maximum), (y.minimum, y.maximum)]
                });
            let keyboard = d.supported_keys().is_some_and(|k| {
                k.contains(evdev::KeyCode::KEY_A) || k.contains(evdev::KeyCode::BTN_LEFT)
            });
            if !(pointer || absolute.is_some() || keyboard) {
                continue;
            }
            // SAFETY: fcntl on a file descriptor this process owns; the
            // flags are read and written whole.
            unsafe {
                let fd = d.as_raw_fd();
                let flags = libc::fcntl(fd, libc::F_GETFL);
                if flags >= 0 {
                    libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
            }
            devices.push((d, absolute));
        }
        Input {
            devices,
            keyboard: Keyboard::default(),
        }
    }

    /// How many devices are open.
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// Whether none opened.
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// The descriptors to poll.
    pub fn fds(&self) -> Vec<i32> {
        self.devices.iter().map(|(d, _)| d.as_raw_fd()).collect()
    }

    /// Everything that arrived since the last read.
    pub fn read(&mut self) -> Vec<InputEvent> {
        let mut out = Vec::new();
        let mut gone = Vec::new();
        for (index, (d, absolute)) in self.devices.iter_mut().enumerate() {
            let events = match d.fetch_events() {
                Ok(events) => events,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue
                }
                Err(_) => {
                    gone.push(index);
                    out.push(InputEvent::Cancel);
                    continue;
                }
            };
            let mut pointer = PointerReport::default();
            let fraction = |range: (i32, i32), v: i32| {
                let span = (range.1 - range.0).max(1) as f32;
                ((v - range.0) as f32 / span).clamp(0.0, 1.0)
            };
            for e in events {
                let summary = e.destructure();
                if !matches!(
                    summary,
                    EventSummary::RelativeAxis(
                        _,
                        RelativeAxisCode::REL_X | RelativeAxisCode::REL_Y,
                        _
                    ) | EventSummary::AbsoluteAxis(
                        _,
                        AbsoluteAxisCode::ABS_X
                            | AbsoluteAxisCode::ABS_Y
                            | AbsoluteAxisCode::ABS_MT_POSITION_X
                            | AbsoluteAxisCode::ABS_MT_POSITION_Y,
                        _
                    )
                ) {
                    pointer.flush(&mut out);
                }
                match summary {
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, v)
                    | EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_X, v) => {
                        if let Some(r) = absolute {
                            pointer.absolute.0 = Some(fraction(r[0], v));
                        }
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, v)
                    | EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_Y, v) => {
                        if let Some(r) = absolute {
                            pointer.absolute.1 = Some(fraction(r[1], v));
                        }
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_X, v) => {
                        pointer.relative.0 += v as f32;
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_Y, v) => {
                        pointer.relative.1 += v as f32;
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_WHEEL_HI_RES, v) => {
                        out.push(InputEvent::Wheel(0.0, -(v as f32) / 120.0 * LINE))
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_HWHEEL_HI_RES, v) => {
                        out.push(InputEvent::Wheel((v as f32) / 120.0 * LINE, 0.0))
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_WHEEL, v) => {
                        // A device with high-resolution wheels reports both;
                        // the coarse one is folded by the loop (see `coalesce`).
                        out.push(InputEvent::Wheel(0.0, -(v as f32) * LINE));
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_HWHEEL, v) => {
                        out.push(InputEvent::Wheel((v as f32) * LINE, 0.0))
                    }
                    EventSummary::Key(_, code, value) => {
                        let code = code.0;
                        let down = value != 0;
                        match code {
                            // BTN_LEFT (a mouse) and BTN_TOUCH (a
                            // touchscreen). Both are a primary press.
                            0x110 | 0x14a => out.push(InputEvent::Button(down)),
                            // BTN_RIGHT, BTN_MIDDLE.
                            0x111 => out.push(InputEvent::Aux(2, down)),
                            0x112 => out.push(InputEvent::Aux(4, down)),
                            _ => {
                                if let Some(event) = self.keyboard.event(code, value, None) {
                                    out.push(event);
                                }
                            }
                        }
                    }
                    EventSummary::Synchronization(_, SynchronizationCode::SYN_DROPPED, _) => {
                        out.push(InputEvent::Cancel)
                    }
                    _ => {}
                }
            }
            pointer.flush(&mut out);
        }
        for index in gone.into_iter().rev() {
            self.devices.remove(index);
        }
        coalesce(out)
    }
}

// Both axes of one evdev report reach recognition together. Separate X then Y
// callbacks could claim a horizontal swipe before the dominant vertical axis.
#[derive(Default)]
struct PointerReport {
    relative: (f32, f32),
    absolute: (Option<f32>, Option<f32>),
}
impl PointerReport {
    fn flush(&mut self, out: &mut Vec<InputEvent>) {
        let (dx, dy) = std::mem::take(&mut self.relative);
        if dx != 0. || dy != 0. {
            out.push(InputEvent::Motion(dx, dy));
        }
        let (x, y) = std::mem::take(&mut self.absolute);
        if x.is_some() || y.is_some() {
            out.push(InputEvent::Absolute(x, y));
        }
    }
}

/// A device that reports both `REL_WHEEL` and `REL_WHEEL_HI_RES` in one
/// report would scroll twice; keep the fine one.
fn coalesce(events: Vec<InputEvent>) -> Vec<InputEvent> {
    let fine = events
        .iter()
        .any(|e| matches!(e, InputEvent::Wheel(_, dy) if dy.abs() % LINE != 0.0));
    if !fine {
        return events;
    }
    events
        .into_iter()
        .filter(|e| !matches!(e, InputEvent::Wheel(dx, dy) if (dy.abs() % LINE == 0.0 && *dy != 0.0) || (dx.abs() % LINE == 0.0 && *dx != 0.0)))
        .collect()
}

/// The US evdev map: physical code, unshifted/shifted logical key.
pub(crate) fn key(code: u16, shift: bool) -> Option<(&'static str, &'static str)> {
    let (code, plain, shifted) = match code {
        1 => ("Escape", "Escape", "Escape"),
        2 => ("Digit1", "1", "!"),
        3 => ("Digit2", "2", "@"),
        4 => ("Digit3", "3", "#"),
        5 => ("Digit4", "4", "$"),
        6 => ("Digit5", "5", "%"),
        7 => ("Digit6", "6", "^"),
        8 => ("Digit7", "7", "&"),
        9 => ("Digit8", "8", "*"),
        10 => ("Digit9", "9", "("),
        11 => ("Digit0", "0", ")"),
        12 => ("Minus", "-", "_"),
        13 => ("Equal", "=", "+"),
        14 => ("Backspace", "Backspace", "Backspace"),
        15 => ("Tab", "Tab", "Tab"),
        16 => ("KeyQ", "q", "Q"),
        17 => ("KeyW", "w", "W"),
        18 => ("KeyE", "e", "E"),
        19 => ("KeyR", "r", "R"),
        20 => ("KeyT", "t", "T"),
        21 => ("KeyY", "y", "Y"),
        22 => ("KeyU", "u", "U"),
        23 => ("KeyI", "i", "I"),
        24 => ("KeyO", "o", "O"),
        25 => ("KeyP", "p", "P"),
        26 => ("BracketLeft", "[", "{"),
        27 => ("BracketRight", "]", "}"),
        28 => ("Enter", "Enter", "Enter"),
        29 => ("ControlLeft", "Control", "Control"),
        30 => ("KeyA", "a", "A"),
        31 => ("KeyS", "s", "S"),
        32 => ("KeyD", "d", "D"),
        33 => ("KeyF", "f", "F"),
        34 => ("KeyG", "g", "G"),
        35 => ("KeyH", "h", "H"),
        36 => ("KeyJ", "j", "J"),
        37 => ("KeyK", "k", "K"),
        38 => ("KeyL", "l", "L"),
        39 => ("Semicolon", ";", ":"),
        40 => ("Quote", "'", "\""),
        41 => ("Backquote", "`", "~"),
        42 => ("ShiftLeft", "Shift", "Shift"),
        43 => ("Backslash", "\\", "|"),
        44 => ("KeyZ", "z", "Z"),
        45 => ("KeyX", "x", "X"),
        46 => ("KeyC", "c", "C"),
        47 => ("KeyV", "v", "V"),
        48 => ("KeyB", "b", "B"),
        49 => ("KeyN", "n", "N"),
        50 => ("KeyM", "m", "M"),
        51 => ("Comma", ",", "<"),
        52 => ("Period", ".", ">"),
        53 => ("Slash", "/", "?"),
        54 => ("ShiftRight", "Shift", "Shift"),
        56 => ("AltLeft", "Alt", "Alt"),
        57 => ("Space", " ", " "),
        96 => ("NumpadEnter", "Enter", "Enter"),
        97 => ("ControlRight", "Control", "Control"),
        100 => ("AltRight", "Alt", "Alt"),
        102 => ("Home", "Home", "Home"),
        103 => ("ArrowUp", "ArrowUp", "ArrowUp"),
        104 => ("PageUp", "PageUp", "PageUp"),
        105 => ("ArrowLeft", "ArrowLeft", "ArrowLeft"),
        106 => ("ArrowRight", "ArrowRight", "ArrowRight"),
        107 => ("End", "End", "End"),
        108 => ("ArrowDown", "ArrowDown", "ArrowDown"),
        109 => ("PageDown", "PageDown", "PageDown"),
        110 => ("Insert", "Insert", "Insert"),
        111 => ("Delete", "Delete", "Delete"),
        // KEY_F1 is 59; F11 and F12 are 87 and 88, not contiguous with F10.
        59 => ("F1", "F1", "F1"),
        60 => ("F2", "F2", "F2"),
        61 => ("F3", "F3", "F3"),
        62 => ("F4", "F4", "F4"),
        63 => ("F5", "F5", "F5"),
        64 => ("F6", "F6", "F6"),
        65 => ("F7", "F7", "F7"),
        66 => ("F8", "F8", "F8"),
        67 => ("F9", "F9", "F9"),
        68 => ("F10", "F10", "F10"),
        87 => ("F11", "F11", "F11"),
        88 => ("F12", "F12", "F12"),
        125 => ("MetaLeft", "Meta", "Meta"),
        126 => ("MetaRight", "Meta", "Meta"),
        _ => return None,
    };
    Some((code, if shift { shifted } else { plain }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_press_repeat_and_release_keep_code_and_logical_key() {
        let mut keyboard = Keyboard::default();
        keyboard.event(42, 1, None);
        for (value, down, repeat) in [(1, true, false), (2, true, true), (0, false, false)] {
            assert_eq!(
                keyboard.event(17, value, None),
                Some(InputEvent::Key {
                    code: 17,
                    shift: true,
                    down,
                    repeat,
                })
            );
        }
        assert_eq!(keyboard.event(17, 3, None), None);
        assert_eq!(keyboard.event(0xffff, 1, None), None);
        assert_eq!(key(59, false), Some(("F1", "F1")));
        assert_eq!(key(87, false), Some(("F11", "F11")));
        assert_eq!(key(88, false), Some(("F12", "F12")));
    }
    #[test]
    fn shift_sides_and_shortcut_chords_pass() {
        let mut keyboard = Keyboard::default();
        keyboard.event(42, 1, None);
        keyboard.event(54, 1, None);
        keyboard.event(42, 0, None);
        assert!(matches!(
            keyboard.event(17, 1, None),
            Some(InputEvent::Key {
                code: 17,
                shift: true,
                ..
            })
        ));
        keyboard.event(54, 0, None);
        for modifier in [29, 97, 125, 126] {
            keyboard.event(modifier, 1, None);
            assert!(matches!(
                keyboard.event(17, 1, None),
                Some(InputEvent::Key {
                    code: 17,
                    down: true,
                    ..
                })
            ));
            assert!(matches!(
                keyboard.event(17, 0, None),
                Some(InputEvent::Key {
                    code: 17,
                    down: false,
                    ..
                })
            ));
            keyboard.event(modifier, 0, None);
        }
        assert!(matches!(
            keyboard.event(17, 1, None),
            Some(InputEvent::Key {
                code: 17,
                shift: false,
                ..
            })
        ));
        assert!(matches!(
            keyboard.event(103, 1, None),
            Some(InputEvent::Key {
                code: 103,
                shift: false,
                ..
            })
        ));
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::*;
    #[test]
    fn a_report_keeps_diagonal_intent_atomic_and_boundaries_separate() {
        let mut report = PointerReport::default();
        let mut out = vec![InputEvent::Button(true)];
        report.relative.0 += 10.;
        report.relative.1 += 30.;
        report.flush(&mut out);
        report.absolute.0 = Some(0.2);
        report.absolute.1 = Some(0.3);
        report.flush(&mut out);
        out.push(InputEvent::Button(false));
        report.flush(&mut out);
        assert_eq!(
            out,
            vec![
                InputEvent::Button(true),
                InputEvent::Motion(10., 30.),
                InputEvent::Absolute(Some(0.2), Some(0.3)),
                InputEvent::Button(false)
            ]
        );
    }
}
