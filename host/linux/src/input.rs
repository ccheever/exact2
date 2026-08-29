//! Input: evdev, read directly — pointer motion, buttons, wheels, keys —
//! with a US keymap for typing. No libinput (nothing to link), so no
//! pointer acceleration, touchpad gestures, or hotplug: the trades LLP 1015
//! §7 declares.
//!
//! @ref LLP 1015 §6

#![allow(unsafe_code)]

use evdev::{Device, EventSummary, EventType, RelativeAxisCode};
use std::os::fd::AsRawFd;

/// One thing the user did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    /// The pointer moved by (dx, dy) device pixels.
    Motion(f32, f32),
    /// The primary button went down (`true`) or up.
    Button(bool),
    /// A wheel: (dx, dy) in points, the web's sign (a positive `dy` scrolls
    /// down).
    Wheel(f32, f32),
    /// A key went down: the character it types, or a control key.
    Key(Key),
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

/// Every device that points or types.
pub struct Input {
    devices: Vec<Device>,
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
            let keyboard = d.supported_keys().is_some_and(|k| {
                k.contains(evdev::KeyCode::KEY_A) || k.contains(evdev::KeyCode::BTN_LEFT)
            });
            if !(pointer || keyboard) {
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
            devices.push(d);
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
        self.devices.iter().map(|d| d.as_raw_fd()).collect()
    }

    /// Everything that arrived since the last read.
    pub fn read(&mut self) -> Vec<InputEvent> {
        let mut out = Vec::new();
        for d in &mut self.devices {
            let Ok(events) = d.fetch_events() else {
                continue;
            };
            for e in events {
                match e.destructure() {
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
                            0x110 => out.push(InputEvent::Button(down)),
                            42 | 54 => self.shift = down,
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
