//! The desktop modules' host services on Windows (LLP 1133.025 D11 item 2,
//! `exact_linux::desktop`): hot keys by `RegisterHotKey`, attention by
//! `FlashWindowEx`, and the login item as a development build's `Run` value.
//!
//! - **Hot keys** are registered with a null window, so `WM_HOTKEY` is
//!   posted to the thread's queue, where the event loop's message hook
//!   hears it ([RegisterHotKey](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-registerhotkey)).
//!   One registration per chord serves every reader of it; a press raises
//!   each reader's event as an activation (D11 item 3). `MOD_NOREPEAT`
//!   keeps a held chord to one press. A modifier pair needs a low-level
//!   keyboard hook, which is not built: it answers `not-allowed`.
//! - **Attention** flashes the taskbar button until the window comes to
//!   the front, or three times for an informational request
//!   ([FlashWindowEx](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-flashwindowex)).
//!   A hidden window has no taskbar button, so nothing is asked.
//! - **The login item** is a value under `HKCU\…\CurrentVersion\Run` named
//!   for the app, whose command line carries `--login` (and `hidden`);
//!   Settings › Apps › Startup turning it off writes `StartupApproved\Run`,
//!   which reads `requires-approval`. An installed build's startup task
//!   (Q13) is not built; this is what every build uses until it is.

#[cfg(windows)]
pub(super) use imp::Desktop;

/// `aria-keyshortcuts` modifier names and their `MOD_*` bits.
const MODIFIERS: [(&str, u32); 4] = [("Alt", 1), ("Control", 2), ("Shift", 4), ("Meta", 8)];

/// A chord as `RegisterHotKey` takes it, `(MOD_* bits, virtual key)`, or
/// why it is not one: `None` for a key this table has no code for.
#[cfg_attr(not(windows), allow(dead_code))]
fn chord(keys: &str) -> Result<(u32, u32), &'static str> {
    let parts: Vec<&str> = keys.split('+').collect();
    let (key, modifiers) = parts.split_last().ok_or("invalid")?;
    if let [a, b] = parts[..] {
        if a == b && MODIFIERS.iter().any(|(m, _)| *m == a) {
            return Err("not-allowed");
        }
    }
    let mut bits = 0;
    for m in modifiers {
        let (_, bit) = MODIFIERS.iter().find(|(n, _)| n == m).ok_or("invalid")?;
        bits |= bit;
    }
    Ok((bits, virtual_key(key).ok_or("invalid")?))
}

/// The virtual-key code of an `aria-keyshortcuts` key name: a letter or
/// digit, F1–F24, or a named key (`KeyboardEvent.key`'s names).
fn virtual_key(key: &str) -> Option<u32> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c
            .is_ascii_alphanumeric()
            .then(|| c.to_ascii_uppercase() as u32);
    }
    if let Some(n) = key.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&n).then(|| 0x6F + n);
    }
    Some(match key {
        "Space" | " " => 0x20,
        "Enter" => 0x0D,
        "Escape" => 0x1B,
        "Tab" => 0x09,
        "Backspace" => 0x08,
        "Delete" => 0x2E,
        "Insert" => 0x2D,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "ArrowLeft" => 0x25,
        "ArrowUp" => 0x26,
        "ArrowRight" => 0x27,
        "ArrowDown" => 0x28,
        _ => return None,
    })
}

/// The command line a `Run` value starts the app with.
#[cfg_attr(not(windows), allow(dead_code))]
fn run_value(exe: &str, hidden: bool) -> String {
    format!("\"{exe}\" --login{}", if hidden { " hidden" } else { "" })
}

/// The launch the login item made, from the process's arguments.
#[cfg_attr(not(windows), allow(dead_code))]
fn login_launch(args: &[String]) -> Option<bool> {
    let at = args.iter().position(|a| a == "--login")?;
    Some(args.get(at + 1).is_some_and(|a| a == "hidden"))
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use exact_linux::desktop::{Login, Refusal, Services};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use windows_sys::Win32::{
        Foundation::{GetLastError, HWND},
        System::Registry::{
            RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
            RRF_RT_REG_BINARY, RRF_RT_REG_SZ,
        },
        UI::{
            Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT},
            WindowsAndMessaging::{
                FlashWindowEx, GetForegroundWindow, IsWindowVisible, FLASHWINFO, FLASHW_TIMERNOFG,
                FLASHW_TRAY, MSG, WM_HOTKEY,
            },
        },
    };

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    /// `RegisterHotKey` fails so when another app holds the chord.
    const ERROR_HOTKEY_ALREADY_REGISTERED: u32 = 1409;

    struct Reader {
        session: String,
        id: String,
        extension: &'static str,
        event: &'static str,
    }
    struct Chord {
        keys: String,
        atom: i32,
        readers: Vec<Reader>,
    }

    /// The host's one hot-key table, taskbar button and launch.
    pub(crate) struct Desktop {
        name: String,
        hwnd: Cell<isize>,
        chords: RefCell<Vec<Chord>>,
        next: Cell<i32>,
        launch: Option<bool>,
        /// Wakes the event loop, whose next turn raises a press.
        wake: RefCell<Option<Box<dyn Fn()>>>,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    impl Desktop {
        /// The services for app `name`.
        pub(crate) fn install(name: &str) -> Rc<Desktop> {
            let args: Vec<String> = std::env::args().collect();
            let desktop = Rc::new(Desktop {
                name: name.to_owned(),
                hwnd: Cell::new(0),
                chords: RefCell::new(Vec::new()),
                next: Cell::new(1),
                launch: super::login_launch(&args),
                wake: RefCell::new(None),
            });
            exact_linux::desktop::install(desktop.clone());
            desktop
        }

        /// Hear thread hot keys from the loop's shared message hook.
        pub(crate) fn message(&self, message: *const std::ffi::c_void) {
            let message = unsafe { &*(message as *const MSG) };
            if message.message == WM_HOTKEY && message.hwnd.is_null() {
                self.pressed(message.wParam as i32);
            }
        }

        /// The window whose taskbar button attention flashes.
        pub(crate) fn set_window(&self, hwnd: isize) {
            self.hwnd.set(hwnd);
        }

        /// What wakes the event loop once a press is heard.
        pub(crate) fn set_wake(&self, wake: impl Fn() + 'static) {
            *self.wake.borrow_mut() = Some(Box::new(wake));
        }

        fn pressed(&self, atom: i32) {
            let chords = self.chords.borrow();
            let Some(chord) = chords.iter().find(|c| c.atom == atom) else {
                return;
            };
            for r in &chord.readers {
                let event = exact_linux::package::json!({ "id": r.id, "keys": chord.keys });
                let args = exact_linux::package::json!([r.id, event]).to_string();
                exact_linux::desktop::activate(r.extension, r.event, args);
            }
            drop(chords);
            if let Some(wake) = &*self.wake.borrow() {
                wake();
            }
        }

        /// Registers `keys` under `atom`, or says why not.
        fn register(&self, keys: &str, atom: i32) -> Result<(), Refusal> {
            let (modifiers, key) = super::chord(keys)?;
            let ok = unsafe {
                RegisterHotKey(std::ptr::null_mut(), atom, modifiers | MOD_NOREPEAT, key)
            };
            if ok != 0 {
                return Ok(());
            }
            Err(match unsafe { GetLastError() } {
                ERROR_HOTKEY_ALREADY_REGISTERED => "in-use",
                _ => "reserved",
            })
        }

        fn read_string(&self, key: &str) -> Option<String> {
            let (key, name) = (wide(key), wide(&self.name));
            let mut buffer = [0u16; 2048];
            let mut size = (buffer.len() * 2) as u32;
            let status = unsafe {
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    name.as_ptr(),
                    RRF_RT_REG_SZ,
                    std::ptr::null_mut(),
                    buffer.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            (status == 0)
                .then(|| String::from_utf16_lossy(&buffer[..(size as usize / 2).saturating_sub(1)]))
        }

        /// Whether Settings › Apps › Startup turned the value off: the
        /// first byte of its `StartupApproved` record is odd.
        fn turned_off(&self) -> bool {
            let (key, name) = (wide(APPROVED), wide(&self.name));
            let mut record = [0u8; 12];
            let mut size = record.len() as u32;
            let status = unsafe {
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    name.as_ptr(),
                    RRF_RT_REG_BINARY,
                    std::ptr::null_mut(),
                    record.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            status == 0 && size > 0 && record[0] & 1 == 1
        }
    }

    impl Services for Desktop {
        fn register_hotkey(
            &self,
            session: &str,
            id: &str,
            keys: &str,
            extension: &'static str,
            event: &'static str,
        ) -> Result<(), Refusal> {
            let reader = Reader {
                session: session.into(),
                id: id.into(),
                extension,
                event,
            };
            let mut chords = self.chords.borrow_mut();
            if let Some(chord) = chords.iter_mut().find(|c| c.keys == keys) {
                if !chord
                    .readers
                    .iter()
                    .any(|r| r.session == session && r.id == id)
                {
                    chord.readers.push(reader);
                }
                return Ok(());
            }
            let atom = self.next.get();
            self.register(keys, atom)?;
            self.next.set(atom + 1);
            chords.push(Chord {
                keys: keys.into(),
                atom,
                readers: vec![reader],
            });
            Ok(())
        }

        fn unregister_hotkey(&self, session: &str, id: &str, keys: &str) {
            let mut chords = self.chords.borrow_mut();
            for chord in chords.iter_mut().filter(|c| c.keys == keys) {
                chord
                    .readers
                    .retain(|r| !(r.session == session && r.id == id));
                if chord.readers.is_empty() {
                    unsafe { UnregisterHotKey(std::ptr::null_mut(), chord.atom) };
                }
            }
            chords.retain(|c| !c.readers.is_empty());
        }

        fn check_hotkey(&self, keys: &str) -> Result<(), Refusal> {
            if self.chords.borrow().iter().any(|c| c.keys == keys) {
                return Ok(());
            }
            // An atom of its own, given back at once.
            let atom = 0xBFFF;
            self.register(keys, atom)?;
            unsafe { UnregisterHotKey(std::ptr::null_mut(), atom) };
            Ok(())
        }

        fn attention(&self, critical: bool) -> Result<(), String> {
            let hwnd = self.hwnd.get() as HWND;
            if hwnd.is_null() || unsafe { IsWindowVisible(hwnd) } == 0 {
                return Err("the window is hidden, so it has no taskbar button to flash".into());
            }
            if unsafe { GetForegroundWindow() } == hwnd {
                return Err("the window is in front".into());
            }
            let info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd,
                dwFlags: FLASHW_TRAY | if critical { FLASHW_TIMERNOFG } else { 0 },
                uCount: if critical { 0 } else { 3 },
                dwTimeout: 0,
            };
            unsafe { FlashWindowEx(&info) };
            Ok(())
        }

        fn login(&self) -> Login {
            match self.read_string(RUN) {
                None => Login {
                    state: "disabled",
                    hidden: false,
                },
                Some(value) => Login {
                    state: if self.turned_off() {
                        "requires-approval"
                    } else {
                        "enabled"
                    },
                    hidden: value.ends_with(" --login hidden"),
                },
            }
        }

        fn set_login(&self, register: Option<bool>) -> Result<Login, String> {
            let (key, name) = (wide(RUN), wide(&self.name));
            let status = match register {
                Some(hidden) => {
                    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                    let value = wide(&super::run_value(&exe.to_string_lossy(), hidden));
                    unsafe {
                        RegSetKeyValueW(
                            HKEY_CURRENT_USER,
                            key.as_ptr(),
                            name.as_ptr(),
                            REG_SZ,
                            value.as_ptr().cast(),
                            (value.len() * 2) as u32,
                        )
                    }
                }
                None => match unsafe {
                    RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr())
                } {
                    // ERROR_FILE_NOT_FOUND: there was none to remove.
                    2 => 0,
                    status => status,
                },
            };
            if status != 0 {
                return Err(format!(
                    "the Run value could not be written (error {status})"
                ));
            }
            Ok(self.login())
        }

        fn login_launch(&self) -> Option<bool> {
            self.launch
        }

        fn open_login_settings(&self) -> Result<(), String> {
            crate::url::open(self.hwnd.get(), "ms-settings:startupapps")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{chord, login_launch, run_value};

    #[test]
    fn a_chord_is_its_modifier_bits_and_virtual_key() {
        assert_eq!(chord("Meta+Shift+2"), Ok((8 | 4, 0x32)));
        assert_eq!(chord("Control+Alt+k"), Ok((2 | 1, 0x4B)));
        assert_eq!(chord("F12"), Ok((0, 0x7B)));
        assert_eq!(chord("Control+ArrowUp"), Ok((2, 0x26)));
        assert_eq!(chord("Shift+Shift"), Err("not-allowed"));
        assert_eq!(chord("Hyper+K"), Err("invalid"));
        assert_eq!(chord("Control+Semicolon"), Err("invalid"));
    }

    #[test]
    fn the_run_value_carries_the_login_launch() {
        let value = run_value(r"C:\Apps\Excord.exe", true);
        assert_eq!(value, r#""C:\Apps\Excord.exe" --login hidden"#);
        let args: Vec<String> = value.split(' ').map(str::to_owned).collect();
        assert_eq!(login_launch(&args), Some(true));
        assert_eq!(login_launch(&["x".into(), "--login".into()]), Some(false));
        assert_eq!(login_launch(&["x".into()]), None);
    }
}
