//! LLP 1133.025 stage 0 in the runner, through this checkout's
//! `@exact/login-item`, `@exact/global-shortcuts` and `@exact/dock`
//! installed beside the app and answered by their substitutes, as under the
//! agent on macOS: the login item's registration and a launch the login item
//! made; a global shortcut listed while read, pressed by `tap @ref`, moved
//! to new keys and released with its last reader; the absent cells; and the
//! status item and Dock menu the agent reads from the tree.

use exact_runner::{agent, CallOrigin, Runner};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};

const APP: &str = r#"use loginItemStatus, registerLoginItem, unregisterLoginItem, LoginItemStatus from "@exact/login-item"
use globalShortcut, checkGlobalShortcut, GlobalShortcut, GlobalShortcutCheck from "@exact/global-shortcuts"
use requestAttention from "@exact/dock"

component App
  state chord = "Meta+Shift+2"
  state pressed = 0
  state heard = ""
  resource login = loginItemStatus() as shape LoginItemStatus
  resource snap = globalShortcut(id="snap", keys=chord) as shape GlobalShortcut
  mutation registered as shape LoginItemStatus
  mutation checked as shape GlobalShortcutCheck
  action enable
    send registered = registerLoginItem(hidden=true)
  action disable
    send registered = unregisterLoginItem()
  action rebind
    chord = "Control+Alt+K"
  action check
    send checked = checkGlobalShortcut(keys="Meta+Shift+3")
  action attend
    requestAttention(critical=true)
  action press(id: string)
    pressed = pressed + 1
    heard = id
  action open
    focus()
  view
    column globalshortcut=press
      text testId="pressed" `${pressed}`
      text testId="snap" `${snap.registered}`
      button testId="tray" -exact-menu="status" aria-label="Snap" popovertarget="traymenu"
        text "S"
      column id="traymenu" popover="auto"
        button testId="open" press=open
          text "Open Snap"
        hr
        button testId="pause" disabled=true
          text "Pause"
      button testId="new" -exact-menu="file dock" press=open
        text "New Conversation"
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-desktop-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["login-item", "global-shortcuts", "dock"] {
            let to = packages.join(package).canonicalize().unwrap();
            symlink_dir(to, path.join("node_modules/@exact").join(package)).unwrap();
        }
        std::fs::write(path.join("app.contract"), APP).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Grants;
impl exact_runner::DataSource for Grants {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "device.login-item\ndevice.global-shortcuts\n"
    }
}

fn boot(app: &App, target: &str) -> Runner<Grants> {
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Grants,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_module_target(target);
    settle(&mut r);
    r
}

/// Time to now, and every topic the substitutes announced asked again.
fn settle(r: &mut Runner<Grants>) {
    for _ in 0..8 {
        let at = r.now_ms();
        r.advance(at).unwrap();
        if r.apply_announced().0.is_empty() {
            break;
        }
    }
}

fn logs(r: &Runner<Grants>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn state(r: &Runner<Grants>) -> Json {
    serde_json::from_str(&agent::handle(r, r#"{"op":"state"}"#)).unwrap()
}

fn show(r: &Runner<Grants>, name: &str) -> String {
    format!("{:?}", r.resource(name).or(r.slot(name)))
}

/// The text of the node with `test_id`.
fn text(r: &Runner<Grants>, test_id: &str) -> String {
    let tree: Json = serde_json::from_str(&agent::handle(r, r#"{"op":"tree"}"#)).unwrap();
    let nodes = tree["nodes"].as_array().unwrap();
    let node = (nodes.iter())
        .find(|n| n["props"]["testId"] == test_id)
        .unwrap_or_else(|| panic!("no {test_id}: {}", logs(r)));
    let child = node["children"][0].clone();
    let text = nodes.iter().find(|n| n["id"] == child).unwrap_or(node);
    text["props"]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn op(r: &mut Runner<Grants>, request: Json) -> Result<String, String> {
    let reply = agent::answer(r, &request.to_string())
        .expect("an agent op")
        .0;
    settle(r);
    let parsed: Json = serde_json::from_str(&reply).unwrap_or_default();
    match parsed["error"].as_str() {
        Some(error) => Err(error.to_owned()),
        None => Ok(reply),
    }
}

fn act(r: &mut Runner<Grants>, action: &str) {
    r.act(action, Vec::new()).unwrap();
    settle(r);
}

/// The registration's entries in `state.globalShortcuts`.
fn shortcuts(r: &Runner<Grants>) -> Vec<Json> {
    let all = state(r)["globalShortcuts"].clone();
    all.as_array().cloned().unwrap_or_default()
}

/// D2: a registration is the app's, kept across its sessions, `hidden` as
/// asked; a status the platform reports as needing approval stays so; a
/// launch the login item made says so, with its `hidden`.
#[test]
fn a_login_item_registers_and_a_login_launch_says_so() {
    let app = App::new("login");
    let mut r = boot(&app, "macos");
    let login = show(&r, "login");
    assert!(login.contains("\"disabled\""), "{login}");
    act(&mut r, "enable");
    let login = show(&r, "login");
    assert!(login.contains("\"enabled\""), "{login}\n{}", logs(&r));
    assert!(
        logs(&r).contains("registerLoginItem: enabled"),
        "{}",
        logs(&r)
    );
    act(&mut r, "disable");
    assert!(
        show(&r, "login").contains("\"disabled\""),
        "{}",
        show(&r, "login")
    );

    let approval = json!({ "op": "prefer", "extension": { "login-item.loginItemStatus": { "state": "requires-approval" } } });
    r.extension_agent(&approval.to_string()).unwrap().1.unwrap();
    settle(&mut r);
    act(&mut r, "enable");
    let registered = show(&r, "registered");
    assert!(registered.contains("\"requires-approval\""), "{registered}");

    let mut again = boot(&app, "macos");
    op(
        &mut again,
        json!({ "op": "launchEntry", "launch": "login", "entry": "hidden" }),
    )
    .unwrap();
    // The status's fields in order: apple, available, hidden,
    // launchedAtLogin, state, windows.
    let login = show(&again, "login");
    let launched = "Bool(true), Bool(false), Bool(true), Str(\"disabled\")";
    assert!(login.contains(launched), "{login}");
    assert!(
        logs(&again).contains("launch --login hidden"),
        "{}",
        logs(&again)
    );
}

/// D2, D3: the changes a registration makes need a person's act.
#[test]
fn a_tool_cannot_register_a_login_item_or_check_a_chord() {
    let app = App::new("origin");
    let mut r = boot(&app, "macos");
    r.act_from(CallOrigin::Tool, "enable", Vec::new()).unwrap();
    settle(&mut r);
    assert!(
        logs(&r).contains("registerLoginItem: refused, person-required"),
        "{}",
        logs(&r)
    );
    assert!(show(&r, "login").contains("\"disabled\""));
}

/// D3: a read registers its chord and lists it; `tap @ref` presses it,
/// raising `globalshortcut` with its id; new keys list the new entry and
/// unregister the old.
#[test]
fn a_shortcut_is_listed_while_read_and_pressed_by_its_ref() {
    let app = App::new("shortcut");
    let mut r = boot(&app, "macos");
    let listed = shortcuts(&r);
    assert_eq!(listed.len(), 1, "{listed:?}\n{}", logs(&r));
    assert_eq!(
        (&listed[0]["id"], &listed[0]["keys"]),
        (&json!("snap"), &json!("Meta+Shift+2"))
    );
    assert_eq!(text(&r, "snap"), "true");
    let reference = listed[0]["ref"].as_u64().expect("a ref");
    op(&mut r, json!({ "op": "tap", "ticket": reference })).unwrap();
    assert_eq!(text(&r, "pressed"), "1", "{}", logs(&r));
    assert_eq!(shortcuts(&r).len(), 1, "a press keeps the registration");

    act(&mut r, "rebind");
    let listed = shortcuts(&r);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["keys"], "Control+Alt+K");
    assert!(op(&mut r, json!({ "op": "tap", "ticket": reference })).is_err());
    assert!(logs(&r).contains("unregistered"), "{}", logs(&r));
}

/// D3: a chord another app holds is preferred as the platform's answer:
/// the registration is listed unregistered, no press reaches it, and a
/// check answers the same reason.
#[test]
fn a_chord_in_use_is_not_registered_and_a_check_says_why() {
    let app = App::new("in-use");
    let mut r = boot(&app, "macos");
    let held = json!({ "op": "prefer", "extension": { "global-shortcuts.globalShortcut": { "registered": false, "reason": "in-use" } } });
    r.extension_agent(&held.to_string()).unwrap().1.unwrap();
    settle(&mut r);
    act(&mut r, "rebind");
    let listed = shortcuts(&r);
    assert_eq!(
        (&listed[0]["registered"], &listed[0]["reason"]),
        (&json!(false), &json!("in-use")),
        "{listed:?}"
    );
    let reference = listed[0]["ref"].as_u64().unwrap();
    let refused = op(&mut r, json!({ "op": "tap", "ticket": reference })).unwrap_err();
    assert!(refused.contains("in-use"), "{refused}");
    act(&mut r, "check");
    let checked = show(&r, "checked");
    assert!(checked.contains("\"in-use\""), "{checked}");
}

/// D14: outside macOS and Windows each module answers that it is absent,
/// and the status and Dock buttons stay ordinary buttons the agent lists.
#[test]
fn the_desktop_modules_are_absent_on_the_other_targets() {
    let app = App::new("absent");
    for target in ["web", "linux", "ios", "android"] {
        let mut r = boot(&app, target);
        assert!(shortcuts(&r).is_empty(), "{target}");
        act(&mut r, "enable");
        assert!(
            logs(&r).contains("NotSupportedError")
                || show(&r, "registered").contains("NotSupported"),
            "{target}: {}\n{}",
            show(&r, "registered"),
            logs(&r)
        );
    }
}

/// D4, D5, §11: `state.statusItems` and `state.dockMenu` from the tree, the
/// status button marked as presented elsewhere and reached by `tap`.
#[test]
fn the_agent_lists_the_status_item_and_the_dock_menu() {
    let app = App::new("presented");
    let r = boot(&app, "macos");
    let s = state(&r);
    let items = s["statusItems"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{s}");
    assert_eq!(
        (&items[0]["label"], &items[0]["testId"]),
        (&json!("Snap"), &json!("tray"))
    );
    let menu = items[0]["menu"].as_array().unwrap();
    let tree = agent::handle(&r, r#"{"op":"tree"}"#);
    assert_eq!(menu.len(), 3, "{menu:?}\n{tree}");
    assert_eq!(
        (&menu[0]["label"], &menu[1], &menu[2]["disabled"]),
        (&json!("Open Snap"), &json!("—"), &json!(true))
    );
    let dock = s["dockMenu"].as_array().unwrap();
    assert_eq!(dock.len(), 1);
    assert_eq!(dock[0]["label"], "New Conversation");
    assert!(tree.contains("\"presented\":\"status\""), "{tree}");
    // Where no status area is, the button is ordinary; the lists stay.
    let r = boot(&app, "web");
    let tree = agent::handle(&r, r#"{"op":"tree"}"#);
    assert!(!tree.contains("\"presented\""), "{tree}");
    assert_eq!(state(&r)["statusItems"].as_array().map(Vec::len), Some(1));
}

/// D5: a request for attention is the agent's to read, critical or not.
#[test]
fn a_request_for_attention_is_kept_for_the_agent() {
    let app = App::new("attention");
    let mut r = boot(&app, "macos");
    act(&mut r, "attend");
    let s = state(&r);
    let attention = s.to_string();
    assert!(
        attention.contains("\"attention\":true"),
        "{attention}\n{}",
        logs(&r)
    );
}
