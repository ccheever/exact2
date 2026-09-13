//! The shipped bytecode calls the real native device through the host data seam.
use exact_js::Module;
use exact_js_value::{to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, FailureKind, Outcome, Store};
use serde_json::Value as Json;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Device {
    root: PathBuf,
    module: Option<Module>,
    plan: Plan,
    store: Store,
}
impl Device {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "messages-device-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut device = Self {
            root,
            module: None,
            plan: Plan::decode(super::PLAN).unwrap(),
            store: Store::new(super::GRANTS, Vec::<(String, String)>::new()),
        };
        device.reopen();
        device
    }
    fn reopen(&mut self) {
        self.module = None;
        let mut m = super::native::module(super::BYTECODE, super::APP, super::GRANTS);
        m.configure_storage(
            self.root.join("data"),
            self.root.join("cache"),
            self.root.join("tmp"),
        )
        .unwrap();
        m.bind(&self.plan);
        assert!(!m.ready());
        m.activate().unwrap();
        self.module = Some(m);
    }
    fn call(&mut self, name: &str, args: Vec<Value>) -> Json {
        let m = self.module.as_mut().unwrap();
        let mut answer = m.answer(&mut self.store, name, &args).unwrap();
        for _ in 0..200 {
            answer = match answer {
                Answer::Now(value) => {
                    let source = self
                        .plan
                        .sources
                        .iter()
                        .find(|row| self.plan.str(row.name) == name)
                        .unwrap();
                    return to_json(&value, &Shape::from_plan(&self.plan, source.ty).unwrap())
                        .unwrap();
                }
                Answer::Later(request) => {
                    let outcome = if let Some(token) = request.continuation {
                        std::thread::spawn(m.continuation(token).expect("host continuation"))
                            .join()
                            .unwrap()
                    } else {
                        Outcome::Failed {
                            kind: FailureKind::Network,
                            message: "offline fixture".into(),
                        }
                    };
                    m.parse(&mut self.store, name, &args, outcome).unwrap()
                }
            };
        }
        panic!("{name} did not settle")
    }
    fn thread(&mut self) -> Json {
        self.call(
            "conversation",
            vec![
                Value::str("maya"),
                Value::Number(0.),
                Value::str(""),
                Value::str(""),
            ],
        )
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        self.module = None;
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn offline_messages_drafts_and_reactions_survive_the_real_native_reopen() {
    let mut d = Device::new();
    let before = d.thread()["messages"].as_array().unwrap().len();
    d.call(
        "sendMessage",
        vec![
            Value::str("maya"),
            Value::str("Kept offline 🌲"),
            Value::str("m9"),
            Value::Number(0.),
            Value::Number(45_000.),
        ],
    );
    let sent = d.thread()["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    let id = sent["id"].as_str().unwrap();
    d.call(
        "react",
        vec![Value::str("maya"), Value::str(id), Value::str("❤️")],
    );
    d.call(
        "saveDraft",
        vec![
            Value::str("dad"),
            Value::str("A durable draft"),
            Value::str(""),
        ],
    );
    d.call("syncMessages", vec![Value::Number(3_000.)]);
    d.reopen();
    let thread = d.thread();
    let messages = thread["messages"].as_array().unwrap();
    assert_eq!(messages.len(), before + 1);
    assert_eq!(messages.last().unwrap()["body"], "Kept offline 🌲");
    assert_eq!(messages.last().unwrap()["reaction"], "❤️");
    assert_eq!(messages.last().unwrap()["replyRoot"], "m9");
    let inbox = d.call("inbox", vec![Value::str(""), Value::Number(0.)]);
    assert_eq!(
        inbox["people"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "dad")
            .unwrap()["draft"],
        "A durable draft"
    );
    d.call(
        "sendMessage",
        vec![
            Value::str("maya"),
            Value::str("After restart"),
            Value::str(""),
            Value::Number(0.),
            Value::Number(46_000.),
        ],
    );
    let next = d.thread();
    assert_ne!(
        next["messages"].as_array().unwrap().last().unwrap()["id"],
        sent["id"]
    );
    assert!(
        d.plan
            .resources
            .iter()
            .filter(|r| ["inbox", "chat", "syncStatus"].contains(&d.plan.str(r.name)))
            .all(|r| r.reader),
        "bake records the storage dependency for post-pixel refresh"
    );
}
