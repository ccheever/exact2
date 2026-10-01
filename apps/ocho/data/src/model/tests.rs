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

fn with_session_tab(w: &mut Workspace, machine: &str, session: &str) {
    w.open_terminal_tab(
        format!("{machine}:{session}:false"),
        session.into(),
        vec!["attach".into()],
        vec!["attach".into()],
        Some((machine.into(), session.into(), false)),
    );
    w.take_jobs();
}

fn answer(w: &mut Workspace, id: u64, stdout: &str) {
    w.apply_io(&serde_json::json!({"replies": [
        {"id": id, "kind": "secrets", "status": 0, "stdout": stdout, "stderr": ""}
    ]}));
}

#[test]
fn secrets_toggle_reads_the_list_and_a_value_goes_only_to_stdin() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    w.execute(Command::ToggleSecrets);
    let jobs = w.take_jobs();
    assert_eq!(jobs[0].argv, ["secret", "list", "m1", "s1"]);
    answer(
        &mut w,
        jobs[0].id,
        r#"[{"id":"r1","name":"API_KEY","reason":"why","status":"pending"}]"#,
    );
    assert_eq!(w.secrets_view()["pending"], "1 pending");
    w.dispatch(Event::Press("secret-open-r1".into()));
    w.dispatch(Event::Input("secret-demo-input".into(), "hunter2".into()));
    let view = w.secrets_view().to_string();
    assert!(
        !view.contains("hunter2"),
        "the value never reaches the view"
    );
    w.dispatch(Event::Key {
        name: "Enter".into(),
        mods: String::new(),
        at: String::new(),
    });
    let jobs = w.take_jobs();
    let provide = jobs.iter().find(|j| j.argv[1] == "provide").unwrap();
    assert_eq!(
        provide.argv,
        ["secret", "provide", "m1", "s1", "r1", "--ttl", "3600s"]
    );
    assert_eq!(provide.stdin, "hunter2");
    answer(&mut w, provide.id, "");
    let again = w.take_jobs();
    assert_eq!(again[0].argv, ["secret", "list", "m1", "s1"]);
}

#[test]
fn a_request_on_another_attached_session_raises_an_alert_that_opens_it() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    with_session_tab(&mut w, "m2", "s2");
    w.dispatch(Event::Tick(1000.0, 0.0, 0.0));
    let jobs = w.take_jobs();
    let s1 = jobs.iter().find(|j| j.argv[3] == "s1").unwrap().id;
    answer(
        &mut w,
        s1,
        r#"[{"id":"r9","name":"NPM_TOKEN","reason":"publish","status":"pending"}]"#,
    );
    assert_eq!(w.secrets_view()["alert"]["name"], "NPM_TOKEN");
    w.dispatch(Event::Press("secret-alert-open".into()));
    assert_eq!(w.active_source(), Some(("m1".into(), "s1".into())));
    assert!(w.secrets.open);
    assert_eq!(w.secrets_view()["alert"]["visible"], false);
}

#[test]
fn the_palette_offers_secrets_only_over_a_session_tab() {
    let mut w = ws();
    w.execute(Command::Palette);
    let offered = |w: &Workspace| {
        w.palette_items()
            .iter()
            .any(|(info, _)| info.command == Command::ToggleSecrets)
    };
    assert!(!offered(&w));
    with_session_tab(&mut w, "m1", "s1");
    w.execute(Command::Palette);
    assert!(offered(&w));
}

#[test]
fn a_secret_link_in_a_terminal_opens_its_pending_request() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    w.dispatch(Event::Press("terminal:secret:ocho://secret/API_KEY".into()));
    let jobs = w.take_jobs();
    assert_eq!(jobs[0].argv, ["secret", "link", "m1", "s1", "API_KEY"]);
    // A printed link cannot name another session or carry a value.
    w.dispatch(Event::Press("terminal:secret:ocho://secret/A/B".into()));
    assert!(w.take_jobs().is_empty());
}
