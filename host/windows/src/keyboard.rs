//! Reconcile keys whose release Windows may consume in a system command or menu.
use exact_linux::Presenter;
use exact_runner::DataSource;

#[allow(unsafe_code)]
pub(super) fn sync<D: DataSource>(presenter: &mut Presenter<D>) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    for (code, key) in [
        ("ShiftLeft", VK_LSHIFT),
        ("ShiftRight", VK_RSHIFT),
        ("ControlLeft", VK_LCONTROL),
        ("ControlRight", VK_RCONTROL),
        ("AltLeft", VK_LMENU),
        ("AltRight", VK_RMENU),
        ("MetaLeft", VK_LWIN),
        ("MetaRight", VK_RWIN),
    ] {
        // Read the state synchronized with this thread's input queue.
        presenter.hold_modifier(code, unsafe { GetKeyState(i32::from(key)) } < 0);
    }
}
