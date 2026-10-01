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
    w.theme_files_arrived("[]", false);
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

#[test]
fn option_command_t_toggles_the_transcript_on_a_session_tab() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    w.dispatch(Event::Press("key:meta+alt+t".into()));
    assert!(w.transcript_visible());
    let jobs = w.take_jobs();
    assert!(jobs.iter().any(|j| j.argv[0] == "transcript"), "{jobs:?}");
}

fn key(w: &mut Workspace, name: &str) {
    w.dispatch(Event::Key {
        name: name.into(),
        mods: String::new(),
        at: String::new(),
    });
}

#[test]
fn transcript_keys_scroll_by_target_and_send_through_the_terminal() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    w.execute(Command::ToggleTranscript);
    let read = w
        .take_jobs()
        .into_iter()
        .find(|j| j.argv[0] == "transcript")
        .unwrap();
    w.apply_io(&serde_json::json!({"replies": [{"id": read.id, "kind": "transcript", "status": 0,
        "stdout": r#"{"revision":"r1","entries":[{"kind":"user","text":"hi"},{"kind":"assistant","text":"**hello**"}]}"#,
        "stderr": ""}]}));
    let top = |w: &Workspace| w.transcript_view()["scrollTop"].as_f64().unwrap();
    assert_eq!(top(&w), 1.0e9, "a fresh transcript starts at the end");
    w.dispatch(Event::Scrolled("transcript".into(), 0.0, 400.0));
    key(&mut w, "j");
    assert_eq!(top(&w), 460.0);
    key(&mut w, "k");
    assert_eq!(top(&w), 340.0);
    key(&mut w, "Home");
    assert_eq!(top(&w), 0.0);
    let entries = w.transcript_view()["entries"].clone();
    assert_eq!(entries[1]["blocks"][0]["runs"][0]["bold"], true);
    assert_eq!(entries[1]["blocks"][0]["id"], "b0");
    // i opens the composer; Enter sends through the terminal.
    key(&mut w, "i");
    w.dispatch(Event::Input("transcript-input".into(), "ship it".into()));
    key(&mut w, "Enter");
    let submit = w
        .take_jobs()
        .into_iter()
        .find(|j| j.argv[0] == "submit-terminal")
        .unwrap();
    assert_eq!(submit.argv[1], "m1:s1:false");
    assert_eq!(submit.stdin, "ship it");
    w.apply_io(&serde_json::json!({"replies": [{"id": submit.id, "kind": "host", "status": 0, "stdout": "sent", "stderr": ""}]}));
    let view = w.transcript_view();
    assert_eq!(view["draft"], "");
    assert_eq!(view["entries"].as_array().unwrap().len(), 3);
    // Esc leaves the composer, a second Esc leaves transcript mode.
    key(&mut w, "Escape");
    key(&mut w, "Escape");
    assert!(!w.transcript_visible());
}

#[test]
fn a_retryable_transport_reconnects_in_place_and_the_strip_says_so() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    w.dispatch(Event::Tick(1000.0, 0.0, 0.0));
    w.take_jobs();
    w.dispatch(Event::Press(
        r#"terminal:connection:{"transport":"ssh","state":"disconnected","retryable":true}"#.into(),
    ));
    let strip = w.connection_strip(false);
    assert_eq!(strip["stripLabel"], "Disconnected");
    assert_eq!(strip["stripRetry"], true);
    assert_eq!(w.tab_chrome()["overlay"], "disconnected");
    w.dispatch(Event::Press("tab:retry".into()));
    let jobs = w.take_jobs();
    assert_eq!(jobs[0].argv, ["retry-connection", "m1:s1:false"]);
    let strip = w.connection_strip(false);
    assert_eq!(strip["stripLabel"], "Reconnecting…");
    assert_eq!(strip["stripRetry"], false);
    w.dispatch(Event::Tick(4000.0, 0.0, 0.0));
    assert_eq!(
        w.connection_strip(false)["stripDetail"],
        "3s · SSH input resumes when attached"
    );
    w.dispatch(Event::Press(
        r#"terminal:connection:{"transport":"ssh","state":"connected"}"#.into(),
    ));
    assert_eq!(w.connection_strip(false)["strip"], "");
}

#[test]
fn an_exit_status_is_kept_and_reconnect_clears_it() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    w.dispatch(Event::Press("terminal:exited:255".into()));
    assert_eq!(w.exit_codes.get("m1:s1:false"), Some(&255));
    w.execute(Command::ReconnectTab);
    assert!(w.exit_codes.is_empty());
    assert!(!w.exited.contains("m1:s1:false"));
    let jobs = w.take_jobs();
    assert!(jobs
        .iter()
        .any(|j| j.argv == ["reconnect-terminal", "m1:s1:false"]));
}

#[test]
fn reconnect_hints_match_the_exit_banner() {
    let mut w = ws();
    with_session_tab(&mut w, "m1", "s1");
    w.dispatch(Event::Press("terminal:exited:0".into()));
    let buttons = w.tab_view_exit_buttons();
    let ids: Vec<_> = buttons
        .iter()
        .map(|b| b["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, ["reconnect", "close", "manager"]);
    assert_eq!(buttons[0]["hint"], w.hint(Command::ReconnectTab));
    assert_eq!(buttons[2]["hint"], w.hint(Command::SelectTab(0)));
}

#[test]
fn an_exited_managed_tab_is_assessed_and_its_card_stays_on_the_tab() {
    let mut w = ws();
    w.apply_feed(&serde_json::json!({"events": [{"type": "machine", "machine": {
        "id": "m1", "name": "box", "local": true,
        "last": {"live_inventory": true, "sessions": [{"id": "s1", "title": "t", "provider": "claude",
            "managed": true, "state": "exited", "pid": 0, "tmux_pane": "%1"}]}}}]}));
    with_session_tab(&mut w, "m1", "s1");
    w.dispatch(Event::Press("terminal:exited:1".into()));
    let jobs = w.take_jobs();
    let assess = jobs
        .iter()
        .find(|j| j.argv[0] == "recovery")
        .expect("assessed");
    assert_eq!(assess.argv, ["recovery", "m1", "s1"]);
    w.apply_io(&serde_json::json!({"replies": [{"id": assess.id, "kind": "recovery", "status": 0,
        "stdout": r#"{"status":"agent-uncertain","reason":"why","launch":{"provider":"claude","resume":"r"}}"#,
        "stderr": ""}]}));
    assert_eq!(w.tab_chrome()["overlay"], "recovery");
    w.dispatch(Event::Press("tab:recovery-details".into()));
    assert_eq!(w.tabs.active, 1, "the card's buttons act on the tab");
    assert_eq!(w.tab_chrome()["overlayReason"], "why");
    w.dispatch(Event::Press("tab:recovery-fork".into()));
    assert!(matches!(
        w.overlay,
        Overlay::Confirm {
            action: super::exec::ConfirmAction::ForkRecovery,
            ..
        }
    ));
}

#[test]
fn the_docked_panel_opens_in_the_sessions_folder_and_closes() {
    let mut w = ws();
    w.apply_feed(&serde_json::json!({"events": [{"type": "machine", "machine": {
        "id": "m1", "name": "box", "local": true,
        "last": {"live_inventory": true, "sessions": [{"id": "s1", "title": "t", "provider": "codex",
            "managed": true, "state": "idle", "pid": 4, "cwd": "~/src"}]}}}]}));
    with_session_tab(&mut w, "m1", "s1");
    w.execute(Command::ToggleTerminalPanel);
    let view = w.panel_view();
    assert_eq!(view["panelKey"], "m1:s1:false#panel");
    assert_eq!(view["panelLabel"], "Terminal · ~/src");
    assert_eq!(
        view["panelArgv"],
        serde_json::json!(["connect", "m1", "--cwd", "~/src", "--persistent", "s1"])
    );
    // A click on the agent takes typing back; FocusTerminalPanel gives it again.
    w.dispatch(Event::Press("terminal:focus".into()));
    assert_eq!(w.panel_view()["panelFocused"], false);
    w.execute(Command::FocusTerminalPanel);
    assert_eq!(w.panel_view()["panelFocused"], true);
    // Dragging the handle up makes it taller, within the window.
    w.dispatch(Event::Tick(0.0, 1200.0, 800.0));
    w.dispatch(Event::Pan("panel-resize".into(), 0.0, -100.0));
    assert_eq!(w.panel_height, 360.0);
    w.dispatch(Event::Pan("panel-resize".into(), 0.0, -1000.0));
    assert_eq!(w.panel_height, 640.0);
    // The shell exiting takes the panel with it.
    w.take_jobs();
    w.dispatch(Event::Press("panel:exited:0".into()));
    assert_eq!(w.panel_view()["panelOpen"], false);
    assert!(w
        .take_jobs()
        .iter()
        .any(|j| j.argv == ["close-terminal", "m1:s1:false#panel"]));
}

#[test]
fn pair_phone_prefers_the_running_server_and_falls_back_to_describe() {
    let mut w = ws();
    w.execute(Command::PairPhone);
    let jobs = w.take_jobs();
    let running = jobs
        .iter()
        .find(|j| j.argv == ["serve-running"])
        .unwrap()
        .id;
    assert!(jobs
        .iter()
        .any(|j| j.argv == ["serve", "--describe", "--no-qr"]));
    w.apply_io(&serde_json::json!({"replies": [{"id": running, "kind": "host", "status": 0, "stdout": "", "stderr": ""}]}));
    let describe = w
        .take_jobs()
        .into_iter()
        .find(|j| j.argv[0] == "serve")
        .unwrap();
    w.apply_io(
        &serde_json::json!({"replies": [{"id": describe.id, "kind": "serve", "status": 0,
        "stdout": r#"{"relay":"r.dev","name":"mac","qr":["101","010"]}"#, "stderr": ""}]}),
    );
    let view = w.phone_view();
    assert_eq!(view["address"], "mac · r.dev");
    assert_eq!(view["qr"][0]["cells"][0]["dark"], true);
    assert_eq!(
        view["note"],
        "The server this Mac started is not running; pairing uses the saved token and the relay."
    );
    key(&mut w, "Escape");
    assert!(matches!(w.overlay, Overlay::None));
}

#[test]
fn a_menu_bar_item_runs_its_command() {
    let mut w = ws();
    w.apply_feed(&serde_json::json!({"events": [{"type": "menu", "command": "palette"}]}));
    assert!(matches!(w.overlay, Overlay::Palette(_)));
    w.apply_feed(&serde_json::json!({"events": [{"type": "menu", "command": "tab-0"}]}));
    assert_eq!(w.tabs.active, 0);
}
