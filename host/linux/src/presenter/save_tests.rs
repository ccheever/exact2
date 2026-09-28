//! `saveFile` on Linux (LLP 1069.010 D3): no panel, so only the agent's
//! substitute answers. A held `export` names its `from`; `type @t <path>`
//! copies the `app:/` file there and fires `change` with the name; `tap @t
//! cancel` fires `cancel`; an ungranted `from` is refused with `cancel`.
use super::*;
use crate::agent::handle;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct Granted;
impl DataSource for Granted {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "fs.read app:/data/out\nfs.write app:/data/out"
    }
}

#[test]
fn an_export_is_held_answered_with_a_path_and_refused_outside_its_grant() {
    let plan = contract::compile(
        "component App\n  state saved = \"none\"\n  state cancels = 0\n  action save\n    saveFile(\"out\", \"app:/data/out/a.json\", \"a.json\")\n  action stray\n    saveFile(\"out\", \"app:/data/secret.json\", \"s.json\")\n  action done(name) writes saved\n    saved = name\n  action cancelled writes cancels\n    cancels = cancels + 1\n  view\n    column width=300 height=300\n      input id=\"out\" testId=\"out\" display=\"none\" change=done cancel=cancelled\n      button press=save testId=\"save\" width=100 height=40\n        text \"Save\"\n      button press=stray testId=\"stray\" width=100 height=40\n        text \"Stray\"\n",
    )
    .unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        Granted,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let id = |p: &Presenter<Granted>, name: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let source = crate::picker::resolve("app:/data/out/a.json").unwrap();
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"{\"version\":1}").unwrap();
    let dir = std::env::temp_dir().join(format!("exact-save-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let to = dir.join("chosen.json");

    let save = id(&p, "save");
    handle(&mut p, &format!(r#"{{"op":"tap","id":{save}}}"#));
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    let held = state["pending"][0].clone();
    assert_eq!(held["device"]["capability"], "export", "{state}");
    assert_eq!(held["device"]["args"]["from"], "app:/data/out/a.json");
    let t = held["ticket"].as_u64().unwrap();
    let relative = handle(
        &mut p,
        &format!(r#"{{"op":"type","ticket":{t},"text":"chosen.json"}}"#),
    );
    assert!(relative.contains("absolute file path"), "{relative}");
    let text = serde_json::Value::from(to.to_string_lossy().into_owned());
    let answered = json(handle(
        &mut p,
        &format!(r#"{{"op":"type","ticket":{t},"text":{text}}}"#),
    ));
    assert_eq!(answered["delivery"], "substituted", "{answered}");
    assert_eq!(std::fs::read(&to).unwrap(), b"{\"version\":1}");
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    assert_eq!(state["slots"]["saved"], "chosen.json", "{state}");

    handle(&mut p, &format!(r#"{{"op":"tap","id":{save}}}"#));
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    let u = state["pending"][0]["ticket"].as_u64().unwrap();
    handle(
        &mut p,
        &format!(r#"{{"op":"tap","ticket":{u},"choice":"cancel"}}"#),
    );
    let stray = id(&p, "stray");
    handle(&mut p, &format!(r#"{{"op":"tap","id":{stray}}}"#));
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    assert_eq!(state["slots"]["cancels"], 2, "{state}");
    assert_eq!(state["pending"], serde_json::json!([]), "{state}");
    let logs = handle(&mut p, r#"{"op":"logs"}"#);
    assert!(logs.contains("saveFile: saved"), "{logs}");
    assert!(
        logs.contains(
            "saveFile: refused: app:/data/secret.json is outside the app's fs.read grants"
        ),
        "{logs}"
    );
    assert!(
        !logs.contains("chosen.json\""),
        "the path is never journalled"
    );
    let _ = std::fs::remove_dir_all(dir);
}
