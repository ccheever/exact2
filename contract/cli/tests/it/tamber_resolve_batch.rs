//! The third pass's #5 B (*tamber/1.17*): `resolve` with `tokens` answers
//! several from one snapshot and one fold, each as `resolve` with that
//! `token` would, a refused name refused in its place. And a write (or a
//! scrub's frame) to the app's design names in `changed` a package's token
//! derived from one the app sets: the app's design is planned folded with
//! its packages' (the build's fold), so its `changed` is that whole view's,
//! each under the app's file.
#![cfg(unix)]

use super::tamber_package_params::package;
use super::tamber_serve::{app, digest, Serve};
use serde_json::{json, Value};

/// The app at `tag`: `brand`, `accent` from it, and `@acme/reading`'s
/// `reading.card` set to it, whose `reading.edge` (the package's own line)
/// derives from `reading.card`.
fn reading(tag: &str) -> std::path::PathBuf {
    let dir = app(
        tag,
        "tamber t\n\ntheme\n  brand = #336699\n  accent = brand\n  reading.card = brand\n",
    );
    package(
        &dir,
        "reading",
        "component Card\n  params\n    fill: color = \"reading.card\"\n  view\n    column background-color=\"param.fill\" border-color=\"reading.edge\" border-width=1\n      text \"c\"\n",
        "tamber reading\n\ntheme\n  reading.card = #112233\n  reading.edge = reading.card\n",
    );
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column background-color=\"accent\"\n      Card()\n",
    )
    .unwrap();
    dir
}

#[test]
fn a_batch_answers_each_name_as_its_own_resolve_would_a_refused_one_in_place() {
    let dir = reading("resolve-batch");
    let mut s = Serve::start(&dir, &[]);
    // A package component's param by its short and full names too, each answer named in full as one alone is.
    let names = [
        "brand",
        "reading.edge",
        "no.such",
        "accent",
        "brand",
        "Card.fill",
        "@acme/reading/index.contract#Card.fill",
    ];
    let (batch, _) = s.ask(json!({"method": "resolve", "tokens": names}));
    let answers = batch["ok"]["answers"].as_array().expect("answers");
    assert_eq!(answers.len(), names.len(), "{batch}");
    for (name, answer) in names.iter().zip(answers) {
        let (one, _) = s.ask(json!({"method": "resolve", "token": name}));
        match one.get("ok") {
            Some(ok) => {
                assert_eq!(batch["ok"]["files"], ok["files"], "{name}");
                let mut ok = ok.clone();
                ok.as_object_mut().unwrap().remove("files");
                assert_eq!(answer, &ok, "{name}");
            }
            None => assert_eq!(answer, &json!({"error": one["error"]}), "{name}"),
        }
    }
    // The package's derived token takes the app's brand through reading.card.
    assert_eq!(
        answers[1]["tuples"][0]["light"],
        json!("#336699ff"),
        "{batch}"
    );
    assert!(answers[2]["error"]["message"].is_string(), "{batch}");
    for param in &answers[5..] {
        assert_eq!(
            param["token"],
            json!("@acme/reading/index.contract#Card.fill"),
            "{batch}"
        );
        assert_eq!(param["tuples"][0]["light"], json!("#336699ff"), "{batch}");
    }
}

#[test]
fn a_batch_past_its_bound_or_beside_token_is_refused_whole() {
    let dir = reading("resolve-batch-bound");
    let mut s = Serve::start(&dir, &[]);
    let many: Vec<String> = (0..4097).map(|i| format!("t{i}")).collect();
    let (r, _) = s.ask(json!({"method": "resolve", "tokens": many}));
    assert_eq!(r["error"]["id"], json!("tamber-resolve-batch"), "{r}");
    assert!(
        r["error"]["message"].as_str().unwrap().contains("4096"),
        "{r}"
    );
    let (r, _) = s.ask(json!({"method": "resolve", "tokens": ["brand"], "token": "brand"}));
    assert!(
        r["error"]["message"].is_string() && r.get("ok").is_none(),
        "{r}"
    );
    let (r, _) = s.ask(json!({"method": "resolve", "tokens": "brand"}));
    assert!(
        r["error"]["message"].is_string() && r.get("ok").is_none(),
        "{r}"
    );
    // The bound itself is answered.
    let at: Vec<&str> = std::iter::repeat_n("brand", 4096).collect();
    let (r, _) = s.ask(json!({"method": "resolve", "tokens": at}));
    assert_eq!(r["ok"]["answers"].as_array().map(Vec::len), Some(4096));
}

#[test]
fn a_batch_whose_answers_pass_their_bytes_is_refused_whole() {
    // A family near the value bound (4,096 bytes), its answer twice that: 4,096 of it pass 16 MiB.
    let stack: Vec<String> = (0..580).map(|i| format!("f{i:04}")).collect();
    let dir = app(
        "resolve-batch-bytes",
        &format!(
            "tamber t\n\ntheme\n  ink = #111111\n  face: family = {}\n",
            stack.join(", ")
        ),
    );
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    column background-color=\"ink\"\n      text \"x\" font-family=\"face\"\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (one, _) = s.ask(json!({"method": "resolve", "token": "face"}));
    assert!(one["ok"].to_string().len() > 4096, "{one}");
    let many: Vec<&str> = std::iter::repeat_n("face", 4096).collect();
    let (r, _) = s.ask(json!({"method": "resolve", "tokens": many}));
    assert_eq!(
        r["error"]["id"],
        json!("tamber-resolve-batch"),
        "{}",
        &r.to_string()[..300.min(r.to_string().len())]
    );
    assert!(
        r["error"]["message"].as_str().unwrap().contains("16 MiB"),
        "{r}"
    );
    let some: Vec<&str> = std::iter::repeat_n("face", 100).collect();
    let (r, _) = s.ask(json!({"method": "resolve", "tokens": some}));
    assert_eq!(r["ok"]["answers"].as_array().map(Vec::len), Some(100));
}

/// What a server before the batch answers one (it reads `token` alone): the
/// refusal the design tool asks again by name on (apps/design-tool/reread.ts
/// `resolveAll`), pinned here so a change to its words breaks a test, not
/// the fallback.
#[test]
fn a_resolve_naming_no_token_is_refused_in_the_words_the_tool_falls_back_on() {
    let dir = reading("resolve-batch-words");
    let mut s = Serve::start(&dir, &[]);
    let (r, _) = s.ask(json!({"method": "resolve"}));
    assert_eq!(
        r["error"]["message"],
        json!("`token` is a string, and needed"),
        "{r}"
    );
}

/// Whether `changed` names `token` in `file`.
fn names(changed: &Value, file: &str, token: &str) -> bool {
    changed
        .as_array()
        .is_some_and(|c| c.contains(&json!({"file": file, "token": token})))
}

#[test]
fn a_write_to_an_app_token_names_the_package_token_derived_from_it() {
    let dir = reading("resolve-batch-derived");
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-token", "name": "brand", "value": "#cc3300"});
    // A scrub's frame names it as the write does.
    let files = json!({"app.tamber": digest(&file)});
    let (frame, _) =
        s.ask(json!({"method": "scrub-fold", "open": true, "files": files, "ops": [op]}));
    let changed = &frame["ok"]["changed"];
    assert!(names(changed, "app.tamber", "reading.edge"), "{frame}");
    let (done, events) =
        s.ask(json!({"method": "transaction", "client": "one", "files": files, "ops": [op]}));
    let changed = &done["ok"]["changed"];
    for t in ["brand", "accent", "reading.card", "reading.edge"] {
        assert!(names(changed, "app.tamber", t), "{t}: {done}");
    }
    // Once, and the event says the same.
    let edge = changed
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["token"] == json!("reading.edge"))
        .count();
    assert_eq!(edge, 1, "{done}");
    let event = events
        .iter()
        .find(|e| e["event"] == json!("changed"))
        .expect("its event");
    assert_eq!(&event["changed"], changed, "{event}");
    // A write the package's tokens don't follow names none of them.
    let files = json!({"app.tamber": digest(&file)});
    let op = json!({"op": "add-token", "name": "lone", "value": "#010101"});
    let (done, _) =
        s.ask(json!({"method": "transaction", "client": "one", "files": files, "ops": [op]}));
    assert_eq!(
        done["ok"]["changed"],
        json!([{"file": "app.tamber", "token": "lone"}]),
        "{done}"
    );
}
