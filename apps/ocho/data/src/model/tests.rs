//! The dispatch seam: presses, chords and keys reach the commands.

use super::*;
use crate::keymap::{resolve, Keystroke, Resolution, Scope};

fn ws() -> Workspace {
    let mut ws = Workspace::new();
    ws.loaded = true;
    ws
}

#[test]
fn chords_from_the_shortcut_buttons_open_what_they_name() {
    let ks = Keystroke::new(",", "meta");
    assert!(ks.meta, "meta parsed: {ks:?}");
    assert!(
        matches!(
            resolve(&[Scope::Global], &ks, None),
            Resolution::Command(crate::palette::Command::Settings)
        ),
        "{:?}",
        resolve(&[Scope::Global], &ks, None)
    );
    let mut w = ws();
    w.dispatch(Event::Press("key:meta+,".into()));
    assert!(matches!(w.overlay, Overlay::Settings(_)), "{:?}", w.overlay);
    let mut w = ws();
    w.dispatch(Event::Press("key:meta+p".into()));
    assert!(matches!(w.overlay, Overlay::Palette(_)));
    let mut w = ws();
    w.dispatch(Event::Press("key:meta+/".into()));
    assert!(matches!(w.overlay, Overlay::Help));
    let mut w = ws();
    w.dispatch(Event::Press("key:meta+shift+e".into()));
    assert!(w.nav);
}

#[test]
fn bare_keys_on_the_manager_move_and_open() {
    let mut w = ws();
    w.dispatch(Event::Key {
        name: "?".into(),
        mods: String::new(),
        at: String::new(),
    });
    assert!(matches!(w.overlay, Overlay::Help), "{:?}", w.overlay);
    w.dispatch(Event::Key {
        name: "Escape".into(),
        mods: String::new(),
        at: String::new(),
    });
    assert!(matches!(w.overlay, Overlay::None));
    w.dispatch(Event::Key {
        name: "2".into(),
        mods: String::new(),
        at: String::new(),
    });
    assert_eq!(w.page, Page::Sessions);
    w.dispatch(Event::Key {
        name: "1".into(),
        mods: String::new(),
        at: String::new(),
    });
    assert_eq!(w.page, Page::Machines);
}
