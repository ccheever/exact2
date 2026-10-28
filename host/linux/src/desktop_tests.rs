//! LLP 1133.025's Windows implementations (`packages/*/windows/lib.rs`)
//! on this loop, over a fake of the host's services: a chord registered,
//! refused and released by its reader, a check that keeps nothing, the
//! login item set and read with the launch, attention asked, and a press
//! the host heard raised on the next turn. Compiled here, on every host,
//! because no app in the tree composes all three for Windows.
use crate::desktop::{Login, Refusal, Services};
use crate::package::{json, Entry, Json};
use crate::presenter::PainterChoice;
use crate::Presenter;
use exact_runner::DataSource;
use std::cell::RefCell;
use std::rc::Rc;

#[path = "../../../packages/dock/windows/lib.rs"]
mod dock;
#[path = "../../../packages/global-shortcuts/windows/lib.rs"]
mod global_shortcuts;
#[path = "../../../packages/login-item/windows/lib.rs"]
mod login_item;

#[derive(Default)]
struct Fake {
    said: RefCell<Vec<String>>,
    login: RefCell<Option<bool>>,
}

impl Services for Fake {
    fn register_hotkey(
        &self,
        session: &str,
        id: &str,
        keys: &str,
        extension: &'static str,
        event: &'static str,
    ) -> Result<(), Refusal> {
        self.said.borrow_mut().push(format!(
            "register {session} {id} {keys} {extension}.{event}"
        ));
        if keys == "Control+Alt+Delete" {
            Err("reserved")
        } else {
            Ok(())
        }
    }
    fn unregister_hotkey(&self, session: &str, id: &str, keys: &str) {
        self.said
            .borrow_mut()
            .push(format!("unregister {session} {id} {keys}"));
    }
    fn check_hotkey(&self, keys: &str) -> Result<(), Refusal> {
        self.said.borrow_mut().push(format!("check {keys}"));
        Err("in-use")
    }
    fn attention(&self, critical: bool) -> Result<(), String> {
        self.said.borrow_mut().push(format!("attention {critical}"));
        Err("the window is in front".into())
    }
    fn login(&self) -> Login {
        match *self.login.borrow() {
            Some(hidden) => Login {
                state: "enabled",
                hidden,
            },
            None => Login {
                state: "disabled",
                hidden: false,
            },
        }
    }
    fn set_login(&self, register: Option<bool>) -> Result<Login, String> {
        *self.login.borrow_mut() = register;
        Ok(self.login())
    }
    fn open_login_settings(&self) -> Result<(), String> {
        self.said.borrow_mut().push("settings".into());
        Ok(())
    }
    fn login_launch(&self) -> Option<bool> {
        Some(true)
    }
}

#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(name.into()))
    }
}

fn call(p: &mut Presenter<Nothing>, body: Json) -> Json {
    let (tx, rx) = std::sync::mpsc::channel();
    let reply = exact_runner::Reply::new(move |outcome| {
        if let exact_runner::Outcome::Response(r) = outcome {
            let _ = tx.send(serde_json::from_slice(&r.body).unwrap_or_default());
        }
    });
    let handler = p
        .host()
        .runner()
        .native_slot()
        .handler()
        .expect("a handler");
    handler(body.to_string().into_bytes(), reply);
    p.run_commands(Nothing::default);
    rx.try_recv().unwrap_or_default()
}

#[test]
fn the_windows_implementations_use_the_hosts_services() {
    let fake = Rc::new(Fake::default());
    crate::desktop::install(fake.clone());
    let plan = crate::test_plan("component App\n  view\n    text \"Hi\"\n");
    let (mut p, _) = Presenter::boot_with(
        &plan,
        Nothing,
        (400., 800.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    p.serve_packages(vec![
        Entry::of::<global_shortcuts::GlobalShortcuts>("global-shortcuts"),
        Entry::of::<login_item::LoginItem>("login-item"),
        Entry::of::<dock::Dock>("dock"),
    ]);

    let read = |keys: &str| json!({"extension": "global-shortcuts", "source": "globalShortcut", "args": {"id": "snap", "keys": keys}});
    let held = call(&mut p, read("Meta+Shift+2"));
    assert_eq!(
        (&held["registered"], &held["id"]),
        (&json!(true), &json!("snap")),
        "{held}"
    );
    let refused = call(&mut p, read("Control+Alt+Delete"));
    assert_eq!(
        (&refused["registered"], &refused["reason"]),
        (&json!(false), &json!("reserved"))
    );
    let checked = call(
        &mut p,
        json!({"extension": "global-shortcuts", "send": "checkGlobalShortcut", "args": {"keys": "Meta+K"}}),
    );
    assert_eq!(checked["reason"], "in-use");
    call(
        &mut p,
        json!({"extension": "global-shortcuts", "release": "globalShortcut", "args": {"id": "snap", "keys": "Meta+Shift+2"}}),
    );

    let status = call(
        &mut p,
        json!({"extension": "login-item", "source": "loginItemStatus", "args": {}}),
    );
    assert_eq!(
        (&status["state"], &status["launchedAtLogin"]),
        (&json!("disabled"), &json!(true)),
        "{status}"
    );
    let on = call(
        &mut p,
        json!({"extension": "login-item", "send": "registerLoginItem", "args": {"hidden": true}}),
    );
    assert_eq!(
        (&on["state"], &on["hidden"]),
        (&json!("enabled"), &json!(true)),
        "{on}"
    );
    assert!(
        on["windows"].is_object(),
        "a Windows status says it is Windows's"
    );
    let off = call(
        &mut p,
        json!({"extension": "login-item", "send": "unregisterLoginItem", "args": {}}),
    );
    assert_eq!(off["state"], "disabled");
    call(
        &mut p,
        json!({"extension": "login-item", "send": "openLoginItemSettings", "args": {}}),
    );
    let attended = call(
        &mut p,
        json!({"extension": "dock", "send": "requestAttention", "args": {"critical": true}}),
    );
    assert_eq!(attended, json!({}));

    assert_eq!(
        *fake.said.borrow(),
        [
            "register 1 snap Meta+Shift+2 global-shortcuts.globalshortcut",
            "register 1 snap Control+Alt+Delete global-shortcuts.globalshortcut",
            "check Meta+K",
            "unregister 1 snap Meta+Shift+2",
            "settings",
            "attention true",
        ]
    );

    // A press the host heard is raised on the next turn, as a person's act.
    crate::desktop::activate(
        "global-shortcuts",
        "globalshortcut",
        json!(["snap", {"id": "snap", "keys": "Meta+Shift+2"}]).to_string(),
    );
    p.run_commands(Nothing::default);
    let journal = p.host().runner().journal().collect::<Vec<_>>().join("\n");
    assert!(
        journal.contains("global-shortcuts.globalshortcut: no handler hears it"),
        "{journal}"
    );
    assert!(
        journal.contains("requestAttention: the window is in front"),
        "{journal}"
    );
}
