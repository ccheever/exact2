//! Input: evdev, read directly — pointer motion, buttons, wheels, keys —
//! with a US keymap for typing. No libinput (nothing to link), so no
//! pointer acceleration, touchpad gestures, or hotplug: the trades LLP 1015
//! §7 declares.
//!
//! @ref LLP 1015 §6

#![allow(unsafe_code)]

use evdev::{AbsoluteAxisCode, Device, EventSummary, EventType, RelativeAxisCode};
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
    /// A wheel: (dx, dy) in points, the web's sign (a positive `dy` scrolls
    /// down).
    Wheel(f32, f32),
    /// A key went down: the character it types, or a control key.
    Key(Key),
    /// Space/Enter press edges for focused Contract controls.
    Activation {
        /// Linux input-event key code (28 for Enter, 57 for Space).
        code: u16,
        /// Whether the key is pressed.
        down: bool,
    },
}

/// A key that matters to an input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    /// A character.
    Char(char),
    /// Backspace.
    Backspace,
    /// Escape.
    Escape,
    /// Enter.
    Enter,
}

/// Points per wheel notch — the browser's tick.
pub const LINE: f32 = 40.0;

/// An absolute device's (min, max) per axis, x then y.
type Ranges = [(i32, i32); 2];

/// Every device that points or types.
pub struct Input {
    devices: Vec<(Device, Option<Ranges>)>,
    shift: bool,
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
            shift: false,
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
        for (d, absolute) in &mut self.devices {
            let Ok(events) = d.fetch_events() else {
                continue;
            };
            let fraction = |range: (i32, i32), v: i32| {
                let span = (range.1 - range.0).max(1) as f32;
                ((v - range.0) as f32 / span).clamp(0.0, 1.0)
            };
            for e in events {
                match e.destructure() {
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, v)
                    | EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_X, v) => {
                        if let Some(r) = absolute {
                            out.push(InputEvent::Absolute(Some(fraction(r[0], v)), None));
                        }
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, v)
                    | EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_Y, v) => {
                        if let Some(r) = absolute {
                            out.push(InputEvent::Absolute(None, Some(fraction(r[1], v))));
                        }
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_X, v) => {
                        out.push(InputEvent::Motion(v as f32, 0.0))
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_Y, v) => {
                        out.push(InputEvent::Motion(0.0, v as f32))
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
                            42 | 54 => self.shift = down,
                            28 | 57 => out.push(InputEvent::Activation { code, down }),
                            _ if down => {
                                if let Some(k) = key(code, self.shift) {
                                    out.push(InputEvent::Key(k));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        coalesce(out)
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

/// The US keymap for the evdev key codes that type.
pub fn key(code: u16, shift: bool) -> Option<Key> {
    let pair = |a: char, b: char| Some(Key::Char(if shift { b } else { a }));
    let letter = |c: char| Some(Key::Char(if shift { c.to_ascii_uppercase() } else { c }));
    match code {
        1 => Some(Key::Escape),
        2 => pair('1', '!'),
        3 => pair('2', '@'),
        4 => pair('3', '#'),
        5 => pair('4', '$'),
        6 => pair('5', '%'),
        7 => pair('6', '^'),
        8 => pair('7', '&'),
        9 => pair('8', '*'),
        10 => pair('9', '('),
        11 => pair('0', ')'),
        12 => pair('-', '_'),
        13 => pair('=', '+'),
        14 => Some(Key::Backspace),
        16 => letter('q'),
        17 => letter('w'),
        18 => letter('e'),
        19 => letter('r'),
        20 => letter('t'),
        21 => letter('y'),
        22 => letter('u'),
        23 => letter('i'),
        24 => letter('o'),
        25 => letter('p'),
        26 => pair('[', '{'),
        27 => pair(']', '}'),
        28 => Some(Key::Enter),
        30 => letter('a'),
        31 => letter('s'),
        32 => letter('d'),
        33 => letter('f'),
        34 => letter('g'),
        35 => letter('h'),
        36 => letter('j'),
        37 => letter('k'),
        38 => letter('l'),
        39 => pair(';', ':'),
        40 => pair('\'', '"'),
        41 => pair('`', '~'),
        43 => pair('\\', '|'),
        44 => letter('z'),
        45 => letter('x'),
        46 => letter('c'),
        47 => letter('v'),
        48 => letter('b'),
        49 => letter('n'),
        50 => letter('m'),
        51 => pair(',', '<'),
        52 => pair('.', '>'),
        53 => pair('/', '?'),
        57 => Some(Key::Char(' ')),
        _ => None,
    }
}
