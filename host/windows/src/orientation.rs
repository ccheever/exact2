//! `orientationlock` on Windows (LLP 1133.029 D8): the process's
//! [`SetDisplayAutoRotationPreferences`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setdisplayautorotationpreferences),
//! which a convertible or a tablet follows when it turns and a desktop
//! monitor never needs. The presenter tells the session's lock when it
//! changes; `natural` is the panel's own landscape or portrait and `current`
//! the display's orientation now, both read with `EnumDisplaySettingsW`. The
//! readback is `GetDisplayAutoRotationPreferences`.
#![allow(unsafe_code)]

use exact_linux::Presenter;
use exact_runner::orientation::{conflict_line, Session, ORIENTATIONS};
use exact_runner::DataSource;
use windows_sys::Win32::Devices::Display::{
    GetDisplayAutoRotationPreferences, SetDisplayAutoRotationPreferences, ORIENTATION_PREFERENCE,
    ORIENTATION_PREFERENCE_LANDSCAPE, ORIENTATION_PREFERENCE_LANDSCAPE_FLIPPED,
    ORIENTATION_PREFERENCE_NONE, ORIENTATION_PREFERENCE_PORTRAIT,
    ORIENTATION_PREFERENCE_PORTRAIT_FLIPPED,
};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};

/// Each orientation bit's preference, in [`ORIENTATIONS`]' order.
const PREFERENCES: [ORIENTATION_PREFERENCE; 4] = [
    ORIENTATION_PREFERENCE_PORTRAIT,
    ORIENTATION_PREFERENCE_PORTRAIT_FLIPPED,
    ORIENTATION_PREFERENCE_LANDSCAPE,
    ORIENTATION_PREFERENCE_LANDSCAPE_FLIPPED,
];

/// The primary display's orientation now and its panel's own, as bits:
/// `DMDO_DEFAULT` is the panel's own way up, and each step turns it 90°.
fn display() -> Option<(u8, u8)> {
    // SAFETY: a zeroed DEVMODEW with its size set is what the call fills.
    let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
    mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    // SAFETY: a null device name is the primary display; `mode` is valid.
    if unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) } == 0 {
        return None;
    }
    // SAFETY: a display's settings fill the display arm of the union.
    let turns = unsafe { mode.Anonymous1.Anonymous2.dmDisplayOrientation } & 3;
    let wide = mode.dmPelsWidth >= mode.dmPelsHeight;
    // An odd number of turns swaps the panel's axis.
    let landscape_panel = wide == (turns % 2 == 0);
    let order: [u8; 4] = if landscape_panel {
        [0b0100, 0b0001, 0b1000, 0b0010]
    } else {
        [0b0001, 0b0100, 0b0010, 0b1000]
    };
    Some((order[turns as usize], order[0]))
}

/// Apply the session's lock when it changed, and report what was asked.
pub(crate) fn apply<D: DataSource>(p: &mut Presenter<D>) {
    let Some(session) = p.orientation_lock() else {
        return;
    };
    let (asked, conflict, why) = match &session {
        Session::None | Session::Conflict(_) => (ORIENTATION_PREFERENCE_NONE, false, None),
        Session::Held(lock) => {
            let (now, natural) = display().unwrap_or((0b1111, 0b1111));
            let set = lock.types
                & if lock.natural { natural } else { 0b1111 }
                & if lock.current { now } else { 0b1111 };
            if set == 0 {
                p.host_mut().runner_mut().log(conflict_line(&lock.by));
                (ORIENTATION_PREFERENCE_NONE, true, Some("conflict"))
            } else {
                let preference = (0..4)
                    .filter(|i| set & (1 << i) != 0)
                    .fold(ORIENTATION_PREFERENCE_NONE, |p, i| p | PREFERENCES[i]);
                (preference, false, None)
            }
        }
    };
    // SAFETY: plain calls on this process's preference.
    let set = unsafe { SetDisplayAutoRotationPreferences(asked) } != 0;
    let mut held = ORIENTATION_PREFERENCE_NONE;
    // SAFETY: `held` is a valid out parameter.
    let read = unsafe { GetDisplayAutoRotationPreferences(&mut held) } != 0;
    let names: Vec<String> = (0..4)
        .filter(|i| held & PREFERENCES[*i] != 0)
        .map(|i| format!("\"{}\"", ORIENTATIONS[i]))
        .collect();
    let why = why.or((!set).then_some("SetDisplayAutoRotationPreferences failed"));
    let applied = format!(
        "{{\"asked\":{asked},\"preference\":{},\"held\":{},\"why\":{}}}",
        if read {
            format!("[{}]", names.join(","))
        } else {
            "null".into()
        },
        set && asked != ORIENTATION_PREFERENCE_NONE,
        why.map_or("null".into(), |w| format!("\"{w}\"")),
    );
    p.orientation_applied(applied, conflict);
}
