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

#[test]
fn a_key_an_input_handled_does_not_run_again_when_it_bubbles() {
    let mut w = ws();
    w.dispatch(Event::Press("key:meta+p".into()));
    assert!(matches!(w.overlay, Overlay::Palette(_)));
    w.dispatch(Event::Key {
        name: "ArrowDown".into(),
        mods: String::new(),
        at: "overlay".into(),
    });
    w.dispatch(Event::Key {
        name: "ArrowDown".into(),
        mods: String::new(),
        at: String::new(),
    });
    match &w.overlay {
        Overlay::Palette(p) => assert_eq!(p.index, 1, "one step, not two"),
        other => panic!("{other:?}"),
    }
    // A window key with no input in between still runs.
    w.dispatch(Event::Key {
        name: "ArrowDown".into(),
        mods: String::new(),
        at: String::new(),
    });
    match &w.overlay {
        Overlay::Palette(p) => assert_eq!(p.index, 2),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_theme_picker_lists_each_bundled_theme_once() {
    let mut w = ws();
    w.open_themes(false);
    w.theme_files_arrived("[]");
    w.open_themes(false);
    match &w.overlay {
        Overlay::Themes(p) => {
            let names: Vec<_> = p.themes.iter().map(|t| t.theme.name.clone()).collect();
            assert_eq!(names, ["Ocho Dark", "Ocho Light"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_debounced_conversation_search_runs_on_the_next_tick() {
    let mut w = ws();
    w.dispatch(Event::Tick(1000.0, 0.0, 0.0));
    w.open_conversations();
    let index = w.take_jobs().last().unwrap().id;
    w.apply_io(&serde_json::json!({"replies": [
        {"id": index, "kind": "topics", "status": 0, "stdout": "{\"sessions\":1}", "stderr": ""}
    ]}));
    w.dispatch(Event::Input("query".into(), "devices".into()));
    w.take_jobs();
    w.dispatch(Event::Tick(2000.0, 0.0, 0.0));
    let job = w.take_jobs();
    assert!(
        job.iter()
            .any(|j| j.argv.contains(&"query".to_string())
                && j.argv.contains(&"devices".to_string())),
        "{job:?}"
    );
}
