//! @ref LLP 1121 D15, D13 (stage 7 B) — the app's value of a param: a line
//! of the design's `params` block, which `set-param` writes and
//! `unset-param` removes (undoing the first restores the component's
//! default), checked against the app's params and their ranges, classed
//! `live` (or `rebuild` for a duration), folded into the build, and read by
//! `resolve`, `explain`, `uses` and `describe` by the param's full name.
#![cfg(unix)]

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  radius.md = 12px\n  duration.fast = 150ms\n";

const VIEW: &str = r##"component PostButton
  params
    radius: length = "radius.md"  range="0..40"  control="slider"
    fade: duration = "duration.fast"
  view
    column border-radius="param.radius" transition="opacity param.fade"
      text "Post"
component App
  view
    column
      PostButton()
      PostButton()
"##;

fn app(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-set-param-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let root = "use PostButton from \"./post.contract\"\n\n".to_string()
        + VIEW
            .split("component App")
            .nth(1)
            .map(|r| format!("component App{r}"))
            .unwrap()
            .as_str();
    std::fs::write(dir.join("app.contract"), root).unwrap();
    std::fs::write(
        dir.join("post.contract"),
        VIEW.split("component App").next().unwrap(),
    )
    .unwrap();
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    dir
}

struct Serve {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Serve {
    fn start(dir: &std::path::Path) -> Serve {
        let mut child = Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(["tamber", "serve", "--stdio", dir.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Serve {
            child,
            input,
            output,
            next: 0,
        }
    }

    fn ask(&mut self, mut request: Value) -> Value {
        self.next += 1;
        request["id"] = json!(self.next);
        writeln!(self.input, "{request}").unwrap();
        self.input.flush().unwrap();
        loop {
            let mut line = String::new();
            if self.output.read_line(&mut line).unwrap() == 0 {
                return Value::Null;
            }
            let v: Value = serde_json::from_str(&line).unwrap();
            if v.get("event").is_some() {
                continue;
            }
            return v;
        }
    }

    fn write(&mut self, dir: &std::path::Path, ops: Value) -> Value {
        use sha2::Digest;
        let seen: String = sha2::Sha256::digest(std::fs::read(dir.join("app.tamber")).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.ask(json!({"method": "transaction", "client": "t", "files": {"app.tamber": seen}, "ops": ops}))
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn set_param_writes_the_apps_value_and_undo_restores_the_default() {
    let dir = app("set");
    let mut s = Serve::start(&dir);
    // A length param's value is live; a duration's rebuilds.
    let live = s.ask(json!({"method": "propose", "ops": [{"op": "set-param", "name": "PostButton.radius", "value": "8px"}]}));
    assert_eq!(live["ok"]["class"], "live", "{live}");
    let rebuild = s.ask(json!({"method": "propose", "ops": [{"op": "set-param", "name": "PostButton.fade", "value": "300ms"}]}));
    assert_eq!(rebuild["ok"]["class"], "rebuild", "{rebuild}");
    let done = s.write(
        &dir,
        json!([{"op": "set-param", "name": "PostButton.radius", "value": "8px"}]),
    );
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let text = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    assert!(
        text.ends_with("\nparams\n  PostButton.radius = 8px\n"),
        "{text}"
    );
    // A `when` line on it, by its full name: on the line the design has.
    let when = s.write(&dir, json!([{"op": "set-param", "name": "post.contract#PostButton.radius", "when": [{"axis": "density", "value": "compact"}], "value": "4px"}]));
    assert!(when["ok"]["inverse"].is_string(), "{when}");
    let text = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    assert!(
        text.ends_with("  PostButton.radius = 8px\n    when density=compact: 4px\n"),
        "{text}"
    );
    // `resolve`, `explain` and `describe` speak the full name.
    let resolved = s.ask(json!({"method": "resolve", "token": "PostButton.radius"}));
    assert_eq!(
        resolved["ok"]["token"], "post.contract#PostButton.radius",
        "{resolved}"
    );
    let values: Vec<String> = resolved["ok"]["tuples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["value"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(values, ["8px", "4px"]);
    // Every instance's row reads the one value.
    let uses = s.ask(json!({"method": "uses", "token": "PostButton.radius"}));
    assert_eq!(
        uses["ok"]["token"], "post.contract#PostButton.radius",
        "{uses}"
    );
    let mut sites: Vec<u64> = uses["ok"]["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["site"].as_u64().unwrap())
        .collect();
    sites.dedup();
    assert_eq!(sites.len(), 2, "two instances: {uses}");
    let explained = s.ask(json!({"method": "explain", "token": "PostButton.radius"}));
    assert_eq!(explained["ok"]["value"], "8px", "{explained}");
    let described = s.ask(json!({"method": "describe"}));
    // The hash only where the components block maps a param to its token.
    let mut shown = described.clone();
    for p in shown["ok"]["ir"]["components"][0]["params"]
        .as_array_mut()
        .unwrap()
    {
        p["token"] = Value::Null;
    }
    assert!(!shown.to_string().contains("param.f"), "{described}");
    let radius = &described["ok"]["ir"]["components"][0]["params"][0];
    assert_eq!(radius["set"]["value"], "8px");
    assert_eq!(
        radius["pinned"],
        json!(["radius.md"]),
        "a literal no longer follows the default's token"
    );
    // Undo the `when`, then the first set: the component's default again.
    s.ask(json!({"method": "undo", "client": "t", "inverse": when["ok"]["inverse"]}));
    let undone = s.ask(json!({"method": "undo", "client": "t", "inverse": done["ok"]["inverse"]}));
    assert!(undone["ok"].is_object(), "{undone}");
    assert_eq!(
        std::fs::read_to_string(dir.join("app.tamber")).unwrap(),
        DESIGN
    );
}

#[test]
fn a_param_set_out_of_range_or_by_an_unknown_name_is_refused() {
    let dir = app("refused");
    let mut s = Serve::start(&dir);
    let out = s.write(
        &dir,
        json!([{"op": "set-param", "name": "PostButton.radius", "value": "80px"}]),
    );
    assert_eq!(
        out["error"]["diagnostics"][0]["id"], "tamber-param-range",
        "{out}"
    );
    let rem = s.write(
        &dir,
        json!([{"op": "set-param", "name": "PostButton.radius", "value": "1rem"}]),
    );
    assert_eq!(
        rem["error"]["diagnostics"][0]["id"], "tamber-range-unit",
        "{rem}"
    );
    let unknown = s.write(
        &dir,
        json!([{"op": "set-param", "name": "PostButton.nope", "value": "1px"}]),
    );
    assert_eq!(unknown["error"]["id"], "tamber-param-unknown", "{unknown}");
    assert_eq!(
        std::fs::read_to_string(dir.join("app.tamber")).unwrap(),
        DESIGN
    );
}

#[test]
fn the_build_takes_the_apps_value_and_warns_of_one_no_param_answers() {
    let dir = app("build");
    std::fs::write(
        dir.join("app.tamber"),
        format!("{DESIGN}\nparams\n  PostButton.radius = 6px\n  Gone.radius = 1px\n"),
    )
    .unwrap();
    let (built, warned) = contract::compile_path_all_warned(&dir.join("app.contract"), false);
    let (plan, _) = built.unwrap_or_else(|e| panic!("{e:?}"));
    let token = contract_lower::params::token_name("post.contract#PostButton", "radius");
    let value = plan
        .theme
        .iter()
        .find(|t| plan.str(t.name) == token)
        .map(|t| plan.str(t.value).to_string());
    assert_eq!(value.as_deref(), Some("6px"));
    assert!(
        warned
            .iter()
            .any(|w| w.id == "tamber-param-unknown" && w.message.contains("Gone.radius")),
        "{warned:?}"
    );
}

#[test]
fn a_duration_set_reaches_the_row_and_one_transaction_writes_one_line() {
    let dir = app("motion");
    std::fs::write(
        dir.join("app.tamber"),
        format!("{DESIGN}\nparams\n  PostButton.fade = 300ms\n"),
    )
    .unwrap();
    let plan =
        contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"));
    let strings = format!("{:?}", plan.strings);
    assert!(
        strings.contains("300ms") && !strings.contains("150ms"),
        "{strings}"
    );
    // Two names for one param in one transaction: one line, the last value.
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    let mut s = Serve::start(&dir);
    let out = s.write(
        &dir,
        json!([
            {"op": "set-param", "name": "PostButton.radius", "value": "8px"},
            {"op": "set-param", "name": "post.contract#PostButton.radius", "value": "16px"}
        ]),
    );
    assert!(out["ok"].is_object(), "{out}");
    let text = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    assert!(
        text.ends_with("\nparams\n  PostButton.radius = 16px\n"),
        "{text}"
    );
    // A second line for one param, by hand, is refused by the build.
    std::fs::write(
        dir.join("app.tamber"),
        format!("{DESIGN}\nparams\n  PostButton.radius = 8px\n  post.contract#PostButton.radius = 4px\n"),
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(e.id, "tamber-param-twice", "{e:?}");
}

#[test]
fn every_write_keeps_a_param_in_range_at_the_line_that_breaks_it() {
    let dir = app("range");
    // An unset that unmasks a default out of range.
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  radius.md = 80px\n  duration.fast = 150ms\n\nparams\n  PostButton.radius = 8px\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir);
    let out = s.write(
        &dir,
        json!([{"op": "unset-param", "name": "PostButton.radius"}]),
    );
    assert_eq!(
        out["error"]["diagnostics"][0]["id"], "tamber-param-range",
        "{out}"
    );
    // A `when` value out of range, at its line.
    drop(s);
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    let mut s = Serve::start(&dir);
    let out = s.write(&dir, json!([
        {"op": "set-param", "name": "PostButton.radius", "value": "8px"},
        {"op": "set-param", "name": "PostButton.radius", "when": [{"axis": "density", "value": "compact"}], "value": "80px"}
    ]));
    let d = &out["error"]["diagnostics"][0];
    assert_eq!(d["id"], "tamber-param-range", "{out}");
    assert_eq!(d["line"], 12, "the `when` line: {out}");
    // The CLI's check builds the app, so an override out of range fails it.
    std::fs::write(
        dir.join("app.tamber"),
        format!("{DESIGN}\nparams\n  PostButton.radius = 80px\n"),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "check"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("tamber-param-range") && err.contains("app.tamber:11:"),
        "{err}"
    );
}

#[test]
fn reads_by_an_ambiguous_name_are_refused_and_an_icon_alias_follows_its_set() {
    let dir = std::env::temp_dir().join(format!("exact2-set-param-icons-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("b")).unwrap();
    let card = |extra: &str| {
        format!("component Card\n  params\n    inset: length = \"4px\"\n{extra}  view\n    column padding=\"param.inset\"\n      text \"c\"\n")
    };
    std::fs::write(dir.join("a.contract"), card("")).unwrap();
    std::fs::write(dir.join("b/c.contract"), card("")).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        r##"use Card from "./a.contract"
use Card as Other from "./b/c.contract"

component App
  params
    base: icon = "icon(sf \"star\")"
    shown: icon = "param.base"
    ink: color = "fg.ink"
  view
    column background-color="param.ink"
      image "symbol:param.shown"
      Card()
      Other()
"##,
    )
    .unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  fg.ink = #111111\n  fg.inky = #ff00aa\n\nparams\n  App.base = icon(sf \"heart\")\n  App.ink = fg.inky\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Serve::start(&dir);
    let ambiguous = s.ask(json!({"method": "resolve", "token": "Card.inset"}));
    assert_eq!(
        ambiguous["error"]["id"], "tamber-param-ambiguous",
        "{ambiguous}"
    );
    let shown = s.ask(json!({"method": "resolve", "token": "App.shown"}));
    assert!(shown.to_string().contains("heart"), "{shown}");
    let explained = s.ask(json!({"method": "explain", "token": "App.shown"}));
    let message = explained["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("is an icon") && !message.contains("param.f"),
        "{explained}"
    );
    let described = s.ask(json!({"method": "describe"}));
    let ink = described["ok"]["ir"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["params"].as_array().unwrap().clone())
        .find(|p| p["param"] == "ink")
        .unwrap();
    assert_eq!(
        ink["pinned"],
        json!(["fg.ink"]),
        "`fg.inky` is no `fg.ink`: {ink}"
    );
}

#[test]
fn a_param_may_read_a_dial_only_params_read() {
    let dir = app("dial");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\naxes\n  density = regular | compact\n\ndials\n  pad: length = 8px  in 0px..32px\n\ntheme\n  radius.md = 12px\n  duration.fast = 150ms\n\nparams\n  PostButton.radius = pad\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir);
    let out = s.ask(json!({"method": "propose", "ops": [{"op": "set-param", "name": "PostButton.radius", "value": "calc(pad + 1px)"}]}));
    assert!(out["ok"].is_object(), "{out}");
}
