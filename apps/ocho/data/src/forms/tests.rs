use std::collections::HashMap;

use super::submit::Submission;
use super::*;
use crate::model::Page;
use crate::picker::PopupAction;
use crate::shapes::{self, Shape};
use crate::types::{Session, Snapshot};

fn acct(name: &str, provider: &str, status: &str) -> Account {
    Account {
        name: name.into(),
        email: format!("{name}@x"),
        provider: provider.into(),
        shared: true,
        status: status.into(),
        ..Default::default()
    }
}

fn mach(id: &str, sessions: Vec<Session>) -> Machine {
    Machine {
        id: id.into(),
        name: id.to_uppercase(),
        last: Some(Snapshot {
            home: "/home/me".into(),
            sessions,
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn limited(provider: &str) -> Session {
    Session {
        id: "s1".into(),
        provider: provider.into(),
        account: "codex:a@x".into(),
        cwd: "/srv/app".into(),
        native_id: "thread-1".into(),
        title: "Fix the build".into(),
        state: "limited".into(),
        pid: 42,
        ..Default::default()
    }
}

/// Two machines (laptop with a Codex default mode, studio), three connected
/// accounts (Codex a and b, Claude c), one disconnected, one profile.
fn state() -> State {
    let mut laptop = mach("laptop", vec![limited("codex")]);
    laptop.permissions.insert("codex".into(), "yolo".into());
    let mut presets = HashMap::new();
    presets.insert(
        "Research".to_string(),
        LaunchProfile {
            machine_id: "studio".into(),
            cwd: "~/research".into(),
            shortcut: 1,
            ..Default::default()
        },
    );
    State {
        machines: vec![laptop, mach("studio", vec![])],
        accounts: vec![
            acct("a", "codex", "connected"),
            acct("b", "codex", "connected"),
            acct("off", "codex", "disconnected"),
            acct("c", "claude", "connected"),
        ],
        presets,
    }
}

fn item(st: &State, session: Session) -> SessionItem {
    SessionItem {
        machine: st.machines[0].clone(),
        session,
    }
}

fn values(form: &Form) -> Vec<&str> {
    form.fields.iter().map(|f| f.value.as_str()).collect()
}

fn labels(form: &Form) -> Vec<&str> {
    form.fields.iter().map(|f| f.label).collect()
}

fn keys(json: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
    out.sort();
    out
}

fn shape_keys(shape: &Shape) -> Vec<String> {
    let Shape::Record(fields) = shape else {
        panic!("not a record");
    };
    let mut out: Vec<String> = fields.iter().map(|(n, _)| n.to_string()).collect();
    out.sort();
    out
}

#[test]
fn handoff_only_offers_connected_same_provider_accounts() {
    let accounts = vec![
        acct("limited", "codex", "connected"),
        acct("ready", "codex", "connected"),
        acct("offline", "codex", "disconnected"),
        acct("claude", "claude", "connected"),
    ];
    let source = Session {
        provider: "codex".into(),
        account: "codex:limited@x".into(),
        ..Default::default()
    };
    assert_eq!(
        handoff_account_choices(&accounts, &source),
        vec!["codex:ready@x"]
    );
    assert!(offers_account_switch("codex"));
    assert!(!offers_account_switch("claude"));
}

#[test]
fn field_kinds_and_placeholders_follow_labels() {
    assert_eq!(field("Machine", "").kind, FieldKind::Choice);
    assert_eq!(field("Shortcut", "").kind, FieldKind::Choice);
    assert_eq!(field(PERMISSIONS_FIELD, "").kind, FieldKind::Choice);
    assert_eq!(field(CLAUDE_PERMISSIONS_FIELD, "").kind, FieldKind::Choice);
    assert_eq!(field("Directory", "").kind, FieldKind::Directory);
    assert_eq!(field(MODEL_FIELD, "").kind, FieldKind::Model);
    assert_eq!(field(EFFORT_FIELD, "").kind, FieldKind::Text);
    assert_eq!(field("Name", "").kind, FieldKind::Text);
    assert_eq!(field_placeholder("Directory"), "~/path/to/project");
    assert_eq!(field_placeholder("Friendly name"), "Enter a name…");
    assert_eq!(field_placeholder("SSH destination"), "user@hostname");
    assert_eq!(field_placeholder(EFFORT_FIELD), "Provider default");
    assert_eq!(field_placeholder("Whatever"), "Enter a value…");
}

#[test]
fn constructors_carry_the_inventory_fields() {
    let st = state();
    let m = Form::machine();
    assert_eq!(
        (m.kind, m.title, m.submit_label()),
        (FormKind::Machine, "Add machine", "Add machine")
    );
    assert_eq!(
        labels(&m),
        [
            "SSH destination",
            "Friendly name",
            "First connection",
            "SSH arguments (optional)"
        ]
    );
    assert_eq!(values(&m)[2], "existing SSH");

    let mut machine = st.machines[0].clone();
    machine.notes = "desk".into();
    machine.tags = vec!["a".into(), "b".into()];
    let e = Form::machine_edit(&machine);
    assert_eq!(
        (e.title, e.submit_label()),
        ("Edit machine", "Save machine")
    );
    assert_eq!(
        values(&e),
        ["LAPTOP", "desk", "a,b", "yolo", "", "", "", ""]
    );
    assert_eq!(
        labels(&e)[3..],
        [
            CODEX_PERMISSIONS_FIELD,
            CLAUDE_PERMISSIONS_FIELD,
            OPENCODE_PERMISSIONS_FIELD,
            ANTIGRAVITY_PERMISSIONS_FIELD,
            GROK_PERMISSIONS_FIELD
        ]
    );
    assert!(e.fields[3..].iter().all(|f| f.kind == FieldKind::Choice));
    assert_eq!(e.editing.as_deref(), Some("laptop"));
    assert!(!e.keeps_draft());

    let a = Form::account();
    assert_eq!((a.title, a.submit_label()), ("Add account", "Sign in"));
    assert_eq!(values(&a), ["codex", "Provider sign-in"]);

    let f = Form::folder(None);
    assert_eq!((f.title, values(&f)[0]), ("New folder", "New folder"));
    let f = Form::folder(Some(("folder:1", "Work")));
    assert_eq!((f.title, values(&f)[0]), ("Rename folder", "Work"));
    assert_eq!(f.editing.as_deref(), Some("folder:1"));

    let l = Form::label(item(&st, limited("codex")));
    assert_eq!((l.title, l.submit_label()), ("Label session", "Save label"));
    assert_eq!(values(&l), ["Fix the build"]);

    let r = Form::resume(item(&st, limited("codex")));
    assert_eq!(r.title, "Resume native conversation");
    assert_eq!(
        values(&r),
        ["laptop", "codex", "codex:a@x", "/srv/app", "thread-1"]
    );
    assert_eq!(
        labels(&r),
        [
            "Machine",
            "Provider",
            "Account",
            "Directory",
            "Native conversation"
        ]
    );

    let p = Form::profile(None, &st).unwrap();
    assert_eq!((p.title, p.submit_label()), ("New profile", "Save profile"));
    assert_eq!(labels(&p).len(), 10);
    // Shortcut 1 is taken by Research; the first machine, Codex, its default account, home.
    assert_eq!(
        values(&p),
        ["", "2", "laptop", "codex", "codex:a@x", "~", "", "", "", ""]
    );
    let existing = NamedProfile {
        name: "Research".into(),
        profile: st.presets["Research"].clone(),
    };
    let p = Form::profile(Some(existing), &st).unwrap();
    assert_eq!(p.title, "Edit profile");
    assert_eq!(values(&p)[..3], ["Research", "1", "studio"]);
    assert_eq!(p.editing.as_deref(), Some("Research"));
    let empty = State::default();
    assert_eq!(
        Form::profile(None, &empty).unwrap_err(),
        "Add a machine before creating a profile"
    );
}

#[test]
fn session_forms_check_their_preconditions() {
    let st = state();
    let sw = Form::switch_account(item(&st, limited("codex")), &st).unwrap();
    assert_eq!(sw.title, "Resume with a different account");
    assert_eq!(values(&sw), ["codex:b@x"]);
    assert_eq!(sw.choices(&st, "Account"), ["codex:b@x"]);
    let err = Form::switch_account(item(&st, limited("claude")), &st).unwrap_err();
    assert!(err.starts_with("Only Codex sessions"));
    let mut running = limited("codex");
    running.state = "working".into();
    let err = Form::switch_account(item(&st, running), &st).unwrap_err();
    assert!(err.starts_with("Wait for the current turn"));
    let mut lonely = st.clone();
    lonely.accounts.truncate(1);
    let err = Form::switch_account(item(&st, limited("codex")), &lonely).unwrap_err();
    assert!(err.starts_with("Connect another Codex account"));

    let h = Form::handoff(item(&st, limited("codex")), &st).unwrap();
    assert_eq!(
        (h.title, h.submit_label()),
        ("Hand off to another agent", "Hand off")
    );
    let mut idle = limited("codex");
    idle.state = "idle".into();
    let err = Form::handoff(item(&st, idle), &st).unwrap_err();
    assert!(err.starts_with("Only a session stopped by a usage limit"));
    let err = Form::handoff(item(&st, limited("claude")), &lonely).unwrap_err();
    assert_eq!(
        err,
        "Connect another Claude account before handing this session off."
    );
    assert_eq!(
        values(&Form::handoff(item(&st, limited("claude")), &st).unwrap()),
        ["claude:c@x"]
    );

    let m = Form::migrate(item(&st, limited("codex")), &st).unwrap();
    assert_eq!(m.title, "Move session to another machine");
    assert_eq!(values(&m), ["studio", ""]);
    let mut fresh = limited("codex");
    fresh.native_id.clear();
    let err = Form::migrate(item(&st, fresh), &st).unwrap_err();
    assert!(err.starts_with("This session has no native conversation"));
    let mut one = st.clone();
    one.machines.truncate(1);
    let err = Form::migrate(item(&st, limited("codex")), &one).unwrap_err();
    assert_eq!(err, "Add a second machine before moving a session.");
}

#[test]
fn label_tab_takes_the_fleet_record_or_the_tabs_own_label() {
    let st = state();
    let known = Form::label_tab(&st, "laptop", "s1", "whatever · LAPTOP");
    assert_eq!(values(&known), ["Fix the build"]);
    let gone = Form::label_tab(&st, "laptop", "s9", "Old work · LAPTOP [view]");
    assert_eq!(values(&gone), ["Old work"]);
    let target = gone.target.as_ref().unwrap();
    assert_eq!(
        (target.machine.name.as_str(), target.session.id.as_str()),
        ("LAPTOP", "s9")
    );
    let unknown = Form::label_tab(&st, "nowhere", "s9", "Plain");
    assert_eq!(unknown.target.as_ref().unwrap().machine.name, "nowhere");
}

#[test]
fn keys_move_cycle_open_submit_and_close() {
    let st = state();
    let mut p = Form::profile(None, &st).unwrap();
    let k = |form: &mut Form, key: &str, mods: Mods| form.key(key, &mods, &st);
    assert_eq!(p.focus, 0);
    assert_eq!(k(&mut p, "Tab", Mods::NONE), FormOutcome::None);
    assert_eq!(p.focus, 1);
    assert_eq!(k(&mut p, "j", Mods::NONE), FormOutcome::None);
    assert_eq!(p.focus, 2);
    assert_eq!(k(&mut p, "k", Mods::NONE), FormOutcome::None);
    assert_eq!(p.focus, 1);
    assert_eq!(k(&mut p, "ArrowDown", Mods::NONE), FormOutcome::None);
    assert_eq!(k(&mut p, "ArrowUp", Mods::NONE), FormOutcome::None);
    assert_eq!(k(&mut p, "Tab", Mods::SHIFT), FormOutcome::None);
    assert_eq!(p.focus, 0);
    assert_eq!(k(&mut p, "Tab", Mods::SHIFT), FormOutcome::None);
    assert_eq!(p.focus, 0);
    // Typing on a text field belongs to the input; j is text there.
    assert_eq!(k(&mut p, "j", Mods::NONE), FormOutcome::Ignored);
    assert_eq!(k(&mut p, "Enter", Mods::SHIFT), FormOutcome::Ignored);
    assert_eq!(k(&mut p, "Enter", Mods::NONE), FormOutcome::None);
    assert_eq!(p.focus, 1);
    // Shortcut: ← / → and h / l cycle round, Space opens the dropdown.
    assert_eq!(k(&mut p, "ArrowRight", Mods::NONE), FormOutcome::None);
    assert_eq!(p.value("Shortcut"), "3");
    assert_eq!(k(&mut p, "h", Mods::NONE), FormOutcome::None);
    assert_eq!(k(&mut p, "ArrowLeft", Mods::NONE), FormOutcome::None);
    assert_eq!(p.value("Shortcut"), "1");
    assert_eq!(k(&mut p, "l", Mods::NONE), FormOutcome::None);
    assert_eq!(p.value("Shortcut"), "2");
    assert_eq!(k(&mut p, " ", Mods::NONE), FormOutcome::OpenChoice(1));
    // Stray typing on a choice is swallowed, never leaked.
    assert_eq!(k(&mut p, "x", Mods::NONE), FormOutcome::None);
    assert_eq!(k(&mut p, "Backspace", Mods::NONE), FormOutcome::None);
    assert_eq!(p.value("Shortcut"), "2");
    assert_eq!(k(&mut p, "Escape", Mods::NONE), FormOutcome::Close);
    p.focus = 9;
    assert_eq!(k(&mut p, "Enter", Mods::NONE), FormOutcome::Submit);
    assert_eq!(k(&mut p, "Tab", Mods::NONE), FormOutcome::None);
    assert_eq!(p.focus, 9);
    let mut label = Form::label(item(&st, limited("codex")));
    assert_eq!(k(&mut label, "Enter", Mods::NONE), FormOutcome::Submit);
    assert_eq!(k(&mut label, "Escape", Mods::NONE), FormOutcome::Close);
}

#[test]
fn a_provider_change_resets_what_depends_on_it() {
    let st = state();
    let mut p = Form::profile(None, &st).unwrap();
    p.input(6, "gpt-6");
    p.input(7, "high");
    p.input(8, "yolo");
    p.focus = 3;
    assert_eq!(p.key("ArrowRight", &Mods::NONE, &st), FormOutcome::None);
    assert_eq!(p.value("Provider"), "claude");
    assert_eq!(p.value("Account"), "claude:c@x");
    assert_eq!(p.value(MODEL_FIELD), "");
    assert_eq!(p.value(EFFORT_FIELD), "");
    assert_eq!(p.value(PERMISSIONS_FIELD), "");
    assert_eq!(
        p.choices(&st, PERMISSIONS_FIELD),
        [
            "",
            "manual",
            "acceptEdits",
            "auto",
            "dontAsk",
            "plan",
            "bypassPermissions"
        ]
    );
    assert_eq!(p.choices(&st, "Account"), ["", "claude:c@x"]);
    // The dropdown path runs the same cascade, only on an actual change.
    p.input(8, "plan");
    p.apply_choice(3, "claude", &st);
    assert_eq!(p.value(PERMISSIONS_FIELD), "plan");
    p.apply_choice(3, "opencode", &st);
    assert_eq!(p.value(PERMISSIONS_FIELD), "");
    assert_eq!(p.value("Account"), "");
    assert_eq!(p.choices(&st, "Account"), [""]);

    // Authentication is typed, as in the source; a provider change resets it.
    let mut a = Form::account();
    a.input(1, "API key");
    assert_eq!(a.value("Authentication"), "API key");
    a.focus = 0;
    assert_eq!(a.choices(&st, "Provider"), crate::launch::PROVIDERS);
    a.key("ArrowLeft", &Mods::NONE, &st);
    assert_eq!(
        a.value("Provider"),
        *crate::launch::PROVIDERS.last().unwrap()
    );
    assert_eq!(a.value("Authentication"), "Provider sign-in");
    // Only Codex and Claude sign in with an API key.
    for provider in ["opencode", "antigravity", "grok"] {
        a.apply_choice(0, provider, &st);
        assert_eq!(a.choices(&st, "Authentication"), ["Provider sign-in"]);
    }
    a.apply_choice(0, "codex", &st);
    a.key("ArrowRight", &Mods::NONE, &st);
    assert_eq!(a.value("Provider"), "claude");
    assert_eq!(
        a.choices(&st, "Authentication"),
        ["Provider sign-in", "API key"]
    );
}

#[test]
fn choices_and_labels_read_as_people_do() {
    let st = state();
    let p = Form::profile(None, &st).unwrap();
    assert_eq!(p.choices(&st, "Shortcut").len(), 10);
    assert_eq!(p.choices(&st, "Machine"), ["laptop", "studio"]);
    assert_eq!(
        p.choices(&st, "Account"),
        ["", "codex:a@x", "codex:b@x", "codex:off@x"]
    );
    assert_eq!(p.choices(&st, "Name"), Vec::<String>::new());
    assert_eq!(
        p.choices(&st, PERMISSIONS_FIELD),
        ["", "read-only", "auto", "full-access", "yolo"]
    );
    assert_eq!(choice_label(&st, "Shortcut", "0"), "None");
    assert_eq!(choice_label(&st, "Shortcut", "5"), "⌘⌥5");
    assert_eq!(choice_label(&st, "Machine", "laptop"), "LAPTOP");
    assert_eq!(choice_label(&st, "Machine", "gone"), "gone");
    assert_eq!(choice_label(&st, "Provider", "opencode"), "OpenCode");
    assert_eq!(
        choice_label(&st, "Account", ""),
        "Use machine’s existing login"
    );
    assert_eq!(choice_label(&st, "Account", "codex:a@x"), "a@x");
    assert_eq!(choice_label(&st, "Account", "x"), "x");
    assert_eq!(choice_label(&st, PERMISSIONS_FIELD, ""), "Machine default");
    assert_eq!(
        choice_label(&st, CODEX_PERMISSIONS_FIELD, ""),
        "Provider default"
    );
    assert_eq!(choice_label(&st, CLAUDE_PERMISSIONS_FIELD, "plan"), "Plan");
    assert_eq!(
        choice_label(&st, GROK_PERMISSIONS_FIELD, ""),
        "Provider default"
    );
    assert_eq!(
        choice_label(&st, ANTIGRAVITY_PERMISSIONS_FIELD, "plan"),
        "Plan"
    );
    assert_eq!(choice_label(&st, "Provider", "grok"), "Grok");
    assert_eq!(
        choice_label(&st, "First connection", "password"),
        "password"
    );
    let e = Form::machine_edit(&st.machines[0]);
    assert_eq!(
        e.choices(&st, OPENCODE_PERMISSIONS_FIELD),
        ["", "ask", "allow"]
    );
    let items = e.popup_items(3, &st);
    assert_eq!(items.len(), 5);
    assert_eq!(items[0].label, "Provider default");
    assert_eq!(items[4].checked, Some(true));
    assert_eq!(items[4].action, PopupAction::Choice(3, "yolo".into()));
    assert!(e.popup_items(9, &st).is_empty());
}

#[test]
fn presses_follow_the_views_ids() {
    let st = state();
    let mut p = Form::profile(None, &st).unwrap();
    assert_eq!(p.press("field:4", &st), FormOutcome::None);
    assert_eq!(p.focus, 4);
    assert_eq!(p.press("choice:3", &st), FormOutcome::OpenChoice(3));
    assert_eq!(p.focus, 3);
    assert_eq!(p.press("choice:4:b@x", &st), FormOutcome::None);
    assert_eq!(p.value("Account"), "codex:b@x");
    assert_eq!(p.press("choice:4:codex:a@x", &st), FormOutcome::None);
    assert_eq!(p.value("Account"), "codex:a@x");
    assert_eq!(p.press("choice:3:Claude", &st), FormOutcome::None);
    assert_eq!(p.value("Provider"), "claude");
    assert_eq!(p.value("Account"), "claude:c@x");
    assert_eq!(p.press("choice:99:x", &st), FormOutcome::Ignored);
    assert_eq!(p.press("button:submit", &st), FormOutcome::Submit);
    assert_eq!(p.press("button:cancel", &st), FormOutcome::Close);
    assert_eq!(p.press("nothing", &st), FormOutcome::Ignored);
}

#[test]
fn folder_completion_follows_the_focused_directory() {
    let st = state();
    let mut m = Form::migrate(item(&st, limited("codex")), &st).unwrap();
    assert_eq!(m.wanted_directories(), None);
    m.key("Tab", &Mods::NONE, &st);
    assert!(m.directory_focused());
    assert_eq!(
        m.wanted_directories(),
        Some(("studio".into(), String::new()))
    );
    let v = m.view(&st);
    assert_eq!(v["status"], "Looking up folders…");
    assert_eq!(v["fields"][1]["kind"], "path");
    assert_eq!(v["focusId"], "field-1");
    m.directories_requested();
    assert_eq!(m.wanted_directories(), None);
    // Tab while loading waits rather than moving on.
    assert_eq!(m.key("Tab", &Mods::NONE, &st), FormOutcome::None);
    assert_eq!(m.focus, 1);
    m.input(1, "~/s");
    assert_eq!(
        m.wanted_directories(),
        Some(("studio".into(), "~/s".into()))
    );
    m.set_directories("studio", "", Ok(DirectoryMatches::default()));
    assert!(m.directory.loading);
    let matches = DirectoryMatches {
        paths: vec!["~/srv/".into(), "~/src/".into()],
        truncated: true,
    };
    m.set_directories("studio", "~/s", Ok(matches));
    assert_eq!(m.wanted_directories(), None);
    let v = m.view(&st);
    assert_eq!(
        v["status"],
        "More folders available; keep typing to narrow results"
    );
    let rows = v["fields"][1]["suggestions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], "dir:0");
    assert_eq!(rows[0]["selected"], true);
    assert_eq!(keys(&rows[0]), shape_keys(&shapes::PICKER_ROW));
    m.key("n", &Mods::CTRL, &st);
    assert_eq!(m.directory.index, 1);
    m.key("p", &Mods::CTRL, &st);
    m.key("p", &Mods::CTRL, &st);
    assert_eq!(m.directory.index, 1);
    assert_eq!(m.key("Tab", &Mods::NONE, &st), FormOutcome::None);
    assert_eq!(m.value("Directory"), "~/src/");
    assert_eq!(m.focus, 1);
    assert_eq!(
        m.wanted_directories(),
        Some(("studio".into(), "~/src/".into()))
    );
    m.set_directories("studio", "~/src/", Err("offline".into()));
    let v = m.view(&st);
    assert_eq!(v["status"], "Folders: offline · Enter keeps your path");
    assert_eq!(v["statusError"], true);
    m.set_directories("studio", "~/src/", Ok(DirectoryMatches::default()));
    m.directory.error.clear();
    assert_eq!(
        m.view(&st)["status"],
        "No matching folders · Enter keeps your path"
    );
    m.press("dir:0", &st);
    assert_eq!(m.value("Directory"), "~/src/");
    // Leaving the field drops the matches; ⇧Tab moves even with suggestions.
    m.key("Tab", &Mods::SHIFT, &st);
    assert_eq!(m.focus, 0);
    assert!(m.directory.paths.is_empty());
    assert_eq!(m.wanted_directories(), None);
}

#[test]
fn the_model_catalog_loads_when_its_field_is_focused() {
    let st = state();
    let mut p = Form::profile(None, &st).unwrap();
    assert_eq!(p.wanted_catalog(&st), None);
    p.press("field:6", &st);
    let key = p.wanted_catalog(&st).unwrap();
    assert_eq!(
        (
            key.machine.as_str(),
            key.provider.as_str(),
            key.account.as_str(),
            key.cwd.as_str()
        ),
        ("laptop", "codex", "codex:a@x", "~")
    );
    assert_eq!(p.view(&st)["status"], "Loading models from LAPTOP…");
    p.catalog_requested(&key);
    assert_eq!(p.wanted_catalog(&st), None);
    let stale = CatalogKey {
        cwd: "/elsewhere".into(),
        ..key.clone()
    };
    p.set_models(&stale, Ok(vec![ModelOption::default()]));
    assert!(p.models.loading);
    let gpt = ModelOption {
        id: "gpt-6".into(),
        name: "GPT-6".into(),
        description: "big".into(),
        default: true,
    };
    p.set_models(&key, Ok(vec![gpt]));
    let v = p.view(&st);
    assert_eq!(v["status"], "Models from LAPTOP");
    let rows = v["fields"][6]["suggestions"].as_array().unwrap();
    assert_eq!(rows[0]["label"], "Native default");
    assert_eq!(rows[0]["selected"], true);
    assert_eq!(rows[1]["id"], "model-row:1");
    assert_eq!(rows[1]["label"], "GPT-6 · provider default");
    assert_eq!(rows[1]["detail"], "big");
    // Refocusing does not ask again; ⌃R does, keeping the rows meanwhile.
    p.key("Tab", &Mods::NONE, &st);
    assert!(p.view(&st)["fields"][6]["suggestions"]
        .as_array()
        .unwrap()
        .is_empty());
    p.key("Tab", &Mods::SHIFT, &st);
    assert_eq!(p.wanted_catalog(&st), None);
    assert_eq!(p.models.options.len(), 1);
    p.key("r", &Mods::CTRL, &st);
    assert_eq!(p.wanted_catalog(&st), Some(key.clone()));
    assert_eq!(p.models.options.len(), 1);
    p.set_models(&key, Err("offline".into()));
    let v = p.view(&st);
    assert_eq!(
        v["status"],
        "Models: offline · Use native default or type a model ID · Ctrl+R retry"
    );
    assert_eq!(v["statusError"], true);
    // ‹ › and the rows choose; a typed ID shows as custom.
    p.set_models(
        &key,
        Ok(vec![ModelOption {
            id: "gpt-6".into(),
            ..Default::default()
        }]),
    );
    p.key("ArrowRight", &Mods::NONE, &st);
    assert_eq!(p.value(MODEL_FIELD), "gpt-6");
    p.press("model-next", &st);
    assert_eq!(p.value(MODEL_FIELD), "");
    p.press("model-prev", &st);
    assert_eq!(p.value(MODEL_FIELD), "gpt-6");
    p.press("model-row:0", &st);
    assert_eq!(p.value(MODEL_FIELD), "");
    p.input(6, "custom-x");
    let rows = p.view(&st)["fields"][6]["suggestions"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(rows, 3);
    assert_eq!(p.model_options()[2].name, "custom-x (custom)");
    // A machine or account the fleet lacks never asks.
    p.apply_choice(2, "gone", &st);
    assert_eq!(p.wanted_catalog(&st), None);
    assert!(p.view(&st)["status"]
        .as_str()
        .unwrap()
        .starts_with("Models: machine gone not found"));
    p.apply_choice(2, "laptop", &st);
    p.apply_choice(4, "nobody", &st);
    assert_eq!(p.wanted_catalog(&st), None);
    assert!(p.view(&st)["status"]
        .as_str()
        .unwrap()
        .starts_with("Models: account nobody not found"));
}

#[test]
fn submits_build_each_kinds_argv() {
    let st = state();
    let mut m = Form::machine();
    m.input(0, "me@host");
    m.input(2, "password");
    m.input(3, "-p 2222");
    let Submission::Terminal {
        key,
        title,
        argv,
        reconnect,
        session,
        replaces,
    } = m.submit(&st, "r1").unwrap()
    else {
        panic!()
    };
    assert_eq!(key, "add:r1");
    assert_eq!(title, "Add me@host");
    assert_eq!(
        argv,
        [
            "add",
            "me@host",
            "--name",
            "",
            "--install",
            "--key",
            "--ssh-args",
            "-p 2222",
            "--password"
        ]
    );
    assert_eq!(reconnect, argv);
    assert_eq!((session, replaces), (None, None));
    m.input(1, "Box");
    m.input(2, "existing SSH");
    let Submission::Terminal { title, argv, .. } = m.submit(&st, "r1").unwrap() else {
        panic!()
    };
    assert_eq!(title, "Add Box");
    assert_eq!(argv.len(), 8);

    let mut a = Form::account();
    let Submission::Terminal {
        key, title, argv, ..
    } = a.submit(&st, "r2").unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (key.as_str(), title.as_str()),
        ("login:r2", "Sign in · Codex")
    );
    assert_eq!(argv, ["accounts", "add", "--provider", "codex"]);
    a.input(1, "API key");
    let Submission::Terminal { argv, .. } = a.submit(&st, "r2").unwrap() else {
        panic!()
    };
    assert_eq!(argv[4], "--api-key");
    for provider in ["opencode", "antigravity", "grok"] {
        a.input(0, provider);
        let Submission::Terminal { argv, title, .. } = a.submit(&st, "r2").unwrap() else {
            panic!()
        };
        assert_eq!(argv, ["accounts", "add", "--provider", provider]);
        assert!(title.starts_with("Sign in · "));
    }
    a.input(0, "grok");
    let Submission::Terminal { title, .. } = a.submit(&st, "r2").unwrap() else {
        panic!()
    };
    assert_eq!(title, "Sign in · Grok");

    let r = Form::recovery_backup();
    assert_eq!(
        (r.title, r.submit_label()),
        ("Back up recovery key", "Save recovery key")
    );
    assert_eq!(labels(&r), ["Save outside Ocho state"]);
    assert_eq!(
        r.submit(&st, "r2").unwrap(),
        Submission::Cli {
            argv: vec![
                "cloud".into(),
                "export-recovery".into(),
                "~/Desktop/ocho-recovery.json".into()
            ],
            done: "Recovery key backed up. Keep the file somewhere safe.".into(),
            page: None,
            reveal: None,
        }
    );
    assert!(!r.keeps_draft());

    let mut l = Form::label(item(&st, limited("codex")));
    l.input(0, "Ship it");
    assert_eq!(
        l.submit(&st, "r3").unwrap(),
        Submission::Cli {
            argv: vec![
                "label".into(),
                "laptop".into(),
                "s1".into(),
                "Ship it".into()
            ],
            done: "Label saved".into(),
            page: None,
            reveal: None,
        }
    );

    let f = Form::folder(Some(("folder:1", "Work")));
    assert_eq!(
        f.submit(&st, "r4").unwrap(),
        Submission::Folder {
            key: Some("folder:1".into()),
            title: "Work".into()
        }
    );
    assert!(matches!(
        Form::folder(None).submit(&st, "r4").unwrap(),
        Submission::Folder { key: None, .. }
    ));

    let mut mg = Form::migrate(item(&st, limited("codex")), &st).unwrap();
    mg.input(1, " /srv/app ");
    let Submission::Terminal {
        key,
        title,
        argv,
        reconnect,
        session,
        replaces,
    } = mg.submit(&st, "r5").unwrap()
    else {
        panic!()
    };
    assert_eq!(key, "studio:r5:false");
    assert_eq!(title, "Fix the build · STUDIO");
    assert_eq!(
        argv,
        [
            "migrate",
            "laptop",
            "s1",
            "studio",
            "--request-id",
            "r5",
            "--yes",
            "--attach",
            "--cwd",
            "/srv/app"
        ]
    );
    assert_eq!(reconnect, ["attach", "studio", "r5"]);
    assert_eq!(session, Some(("studio".into(), "r5".into(), false)));
    assert_eq!(replaces, Some(("laptop".into(), "s1".into())));
    mg.apply_choice(0, "laptop", &st);
    assert_eq!(
        mg.submit(&st, "r5").unwrap_err(),
        "Choose a different machine to move this session to."
    );

    let sw = Form::switch_account(item(&st, limited("codex")), &st).unwrap();
    let Submission::Cli {
        argv,
        done,
        page,
        reveal,
    } = sw.submit(&st, "r6").unwrap()
    else {
        panic!()
    };
    assert_eq!(
        argv,
        [
            "switch-account",
            "laptop",
            "s1",
            "--account",
            "codex:b@x",
            "--yes"
        ]
    );
    assert_eq!(done, "Session resumed under codex:b@x");
    assert_eq!((page, reveal), (None, Some(("laptop".into(), "s1".into()))));
    let mut app_server = limited("codex");
    app_server.codex_socket = "/tmp/codex.sock".into();
    let sw = Form::switch_account(item(&st, app_server), &st).unwrap();
    let Submission::Terminal {
        key,
        title,
        argv,
        replaces,
        ..
    } = sw.submit(&st, "r7").unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (key.as_str(), title.as_str()),
        ("laptop:r7:false", "Fix the build · LAPTOP")
    );
    assert_eq!(
        argv,
        [
            "switch-account",
            "laptop",
            "s1",
            "--account",
            "codex:b@x",
            "--request-id",
            "r7",
            "--yes",
            "--attach"
        ]
    );
    assert_eq!(replaces, Some(("laptop".into(), "s1".into())));
    let mut none = sw.clone();
    none.fields[0].value.clear();
    assert_eq!(
        none.submit(&st, "r7").unwrap_err(),
        "Choose another Codex account."
    );

    let h = Form::handoff(item(&st, limited("codex")), &st).unwrap();
    let Submission::Terminal {
        key,
        title,
        argv,
        reconnect,
        replaces,
        ..
    } = h.submit(&st, "r8").unwrap()
    else {
        panic!()
    };
    assert_eq!(key, "laptop:r8:false");
    assert_eq!(title, "Take over: Fix the build · LAPTOP");
    assert_eq!(
        argv,
        [
            "handoff",
            "laptop",
            "s1",
            "--account",
            "codex:b@x",
            "--request-id",
            "r8",
            "--yes",
            "--attach"
        ]
    );
    assert_eq!(reconnect, ["attach", "laptop", "r8"]);
    assert_eq!(replaces, Some(("laptop".into(), "s1".into())));

    let mut remote = limited("claude");
    remote.claude_remote = true;
    let r = Form::resume(item(&st, remote));
    let Submission::Terminal {
        key,
        title,
        argv,
        reconnect,
        session,
        ..
    } = r.submit(&st, "r9").unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (key.as_str(), title.as_str()),
        ("laptop:r9:false", "Resume · Claude · LAPTOP")
    );
    assert_eq!(
        argv,
        [
            "run",
            "laptop",
            "--request-id",
            "r9",
            "--provider",
            "claude",
            "--account",
            "codex:a@x",
            "--cwd",
            "/srv/app",
            "--resume",
            "thread-1",
            "--attach",
            "--claude-remote"
        ]
    );
    assert_eq!(reconnect, argv);
    assert_eq!(session, Some(("laptop".into(), "r9".into(), false)));
    let mut blank = r.clone();
    blank.fields[4].value.clear();
    assert_eq!(
        blank.submit(&st, "r9").unwrap_err(),
        "No native conversation identity is available."
    );

    let mut e = Form::machine_edit(&st.machines[0]);
    e.input(1, " desk ");
    e.input(4, "plan");
    let Submission::Cli {
        argv, done, page, ..
    } = e.submit(&st, "r10").unwrap()
    else {
        panic!()
    };
    assert_eq!(
        argv,
        [
            "machine",
            "edit",
            "laptop",
            "--name",
            "LAPTOP",
            "--notes",
            "desk",
            "--tags",
            "",
            "--permissions",
            "codex=yolo,claude=plan,opencode=default,antigravity=default,grok=default"
        ]
    );
    assert_eq!(
        (done.as_str(), page),
        ("Machine saved", Some(Page::Machines))
    );
    e.input(0, "  ");
    assert_eq!(e.submit(&st, "r10").unwrap_err(), "A machine needs a name");
    assert!(e.retry_on_error());
    assert!(!m.retry_on_error());

    let existing = NamedProfile {
        name: "Research".into(),
        profile: st.presets["Research"].clone(),
    };
    let mut p = Form::profile(Some(existing), &st).unwrap();
    p.input(0, " Deep ");
    p.input(6, "gpt-6");
    p.input(9, "Go");
    let Submission::Profile {
        argv,
        name,
        previous,
        saved,
    } = p.submit(&st, "r11").unwrap()
    else {
        panic!()
    };
    assert_eq!(
        argv,
        [
            "profiles",
            "add",
            "Deep",
            "--machine",
            "studio",
            "--provider",
            "",
            "--account",
            "",
            "--cwd",
            "~/research",
            "--model",
            "gpt-6",
            "--effort",
            "",
            "--prompt",
            "Go",
            "--permissions",
            "",
            "--shortcut",
            "1",
            "--replace",
            "Research"
        ]
    );
    assert_eq!(
        (name.as_str(), previous.as_deref()),
        ("Deep", Some("Research"))
    );
    assert_eq!(
        (saved.model.as_str(), saved.shortcut, saved.prompt.as_str()),
        ("gpt-6", 1, "Go")
    );
    p.input(0, "Research");
    let Submission::Profile { previous, .. } = p.submit(&st, "r11").unwrap() else {
        panic!()
    };
    assert_eq!(previous, None);
    let fresh = Form::profile(None, &st).unwrap();
    let Submission::Profile { argv, .. } = fresh.submit(&st, "r12").unwrap() else {
        panic!()
    };
    assert_eq!(argv.len(), 21);
}

#[test]
fn the_view_matches_the_shapes() {
    let st = state();
    let mut p = Form::profile(None, &st).unwrap();
    p.error = "Nope".into();
    let v = p.view(&st);
    assert_eq!(keys(&v), shape_keys(&shapes::OVERLAY));
    assert_eq!(v["kind"], "form");
    assert_eq!(v["width"], 680);
    assert_eq!(v["title"], "New profile");
    assert_eq!(v["status"], "Nope");
    assert_eq!(v["statusError"], true);
    assert_eq!(v["footer"], p.hint());
    assert_eq!(v["focusId"], "field-0");
    let fields = v["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 10);
    for f in fields {
        assert_eq!(keys(f), shape_keys(&shapes::FIELD));
    }
    assert_eq!(fields[0]["id"], "0");
    assert_eq!(fields[0]["kind"], "text");
    assert_eq!(fields[0]["focused"], true);
    assert_eq!(fields[0]["placeholder"], "Enter a name…");
    assert_eq!(fields[1]["kind"], "choice");
    assert_eq!(fields[1]["value"], "⌘⌥2");
    assert_eq!(fields[1]["options"][0], "None");
    assert_eq!(fields[2]["value"], "LAPTOP");
    assert_eq!(fields[4]["value"], "a@x");
    assert_eq!(fields[5]["kind"], "path");
    assert_eq!(fields[6]["kind"], "model");
    assert_eq!(fields[6]["placeholder"], "Provider default");
    assert_eq!(fields[7]["placeholder"], "Provider default");
    assert_eq!(fields[9]["multiline"], true);
    assert_eq!(fields[9]["hint"], "");
    let buttons = v["buttons"].as_array().unwrap();
    for b in buttons {
        assert_eq!(keys(b), shape_keys(&shapes::BUTTON));
    }
    assert_eq!(buttons[0]["id"], "button:cancel");
    assert_eq!(buttons[1]["label"], "Save profile");
    assert_eq!(buttons[1]["primary"], true);
    // A focused choice takes no text focus; a focused multiline field says so.
    p.error.clear();
    p.focus = 1;
    let v = p.view(&st);
    assert_eq!(v["focusId"], "");
    assert_eq!(v["status"], "");
    assert_eq!(v["statusError"], false);
    p.focus = 9;
    let v = p.view(&st);
    assert_eq!(
        v["fields"][9]["hint"],
        "Shift+Enter new line · Tab next field"
    );
    assert_eq!(v["focusId"], "field-9");
    let a = Form::account().view(&st);
    assert_eq!(a["buttons"][1]["label"], "Sign in");
    assert!(a["footer"]
        .as_str()
        .unwrap()
        .starts_with("Space picks the provider"));
}

#[test]
fn drafts_keep_typed_text_for_fresh_forms_only() {
    let st = state();
    let mut m = Form::machine();
    assert!(m.keeps_draft());
    assert_eq!(m.draft(), None);
    m.input(0, "me@host");
    m.input(2, "password");
    let (kind, values) = m.draft().unwrap();
    assert_eq!(kind, FormKind::Machine);
    assert_eq!(values, ["me@host", "", "password", ""]);
    let back = Form::machine().with_draft(&values);
    assert_eq!(back.value("SSH destination"), "me@host");
    assert_eq!(back.value("First connection"), "password");
    // A draft from another shape is ignored; a choice alone is no draft.
    assert_eq!(
        Form::machine()
            .with_draft(&["x".into()])
            .value("SSH destination"),
        ""
    );
    // Authentication is a text field in the source, so the Account form
    // always has input and always keeps its draft.
    let a = Form::account();
    assert_eq!(
        a.draft(),
        Some((
            FormKind::Account,
            vec!["codex".into(), "Provider sign-in".into()]
        ))
    );
    assert!(!Form::folder(Some(("k", "Work"))).keeps_draft());
    assert!(!Form::label(item(&st, limited("codex"))).keeps_draft());
    let mut p = Form::profile(None, &st).unwrap();
    p.input(0, "Deep");
    assert!(p.draft().is_some());
    assert_eq!(DRAFT_KEPT, "Draft kept; reopen the form to continue");
}

#[test]
fn pair_imessage_asks_for_a_number_with_its_country_code() {
    let st = state();
    let mut p = Form::pair_imessage();
    assert_eq!(
        (p.title, p.submit_label()),
        ("Pair iMessage", "Get pairing code")
    );
    assert_eq!(labels(&p), [PHONE_FIELD]);
    assert_eq!(p.fields[0].kind, FieldKind::Text);
    assert!(p
        .hint()
        .starts_with("Use the number you send iMessages from"));
    assert!(p.retry_on_error());
    assert!(!p.keeps_draft());
    assert_eq!(p.pairing_phone(), Some(Err(PHONE_ERROR.to_string())));
    assert_eq!(p.submit(&st, "r1").unwrap_err(), PHONE_ERROR);
    p.input(0, "+1 (415) 555-2671");
    assert_eq!(p.pairing_phone(), Some(Ok("+14155552671".to_string())));
    assert_eq!(Form::account().pairing_phone(), None);

    assert_eq!(
        normalize_phone("+1 (415) 555-2671").as_deref(),
        Some("+14155552671")
    );
    for invalid in ["4155552671", "+0123456789", "+123", "+1415555abcd", ""] {
        assert_eq!(normalize_phone(invalid), None, "{invalid}");
    }
}
