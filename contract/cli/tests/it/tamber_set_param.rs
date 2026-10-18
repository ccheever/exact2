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

/// LLP 1122 T3 (D6's re-scope): a param edit moved to the token it aliases,
/// and back, is one transaction of the edit's inverse and the new write, and
/// one undo restores the bytes; a `when` branch moves to the same branch.
#[test]
fn a_re_scope_between_a_param_and_its_token_is_one_transaction_and_one_undo() {
    let dir = app("rescope");
    let mut s = Serve::start(&dir);
    let read = || std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    let compact = json!([{"axis": "density", "value": "compact"}]);
    // The param edit, then re-scoped to its token: the line goes, the token takes the value.
    let edit = s.write(&dir, json!([{"op": "set-param", "name": "PostButton.radius", "value": "8px"}]));
    assert!(edit["ok"]["inverse"].is_string(), "{edit}");
    let edited = read();
    let moved = s.write(&dir, json!([
        {"op": "unset-param", "name": "PostButton.radius"},
        {"op": "set-token", "name": "radius.md", "when": [], "value": "8px"},
    ]));
    assert!(moved["ok"]["inverse"].is_string(), "{moved}");
    assert_eq!(read(), DESIGN.replace("radius.md = 12px", "radius.md = 8px"));
    // One undo: the param edit as it was.
    s.ask(json!({"method": "undo", "client": "t", "inverse": moved["ok"]["inverse"]}));
    assert_eq!(read(), edited);
    s.ask(json!({"method": "undo", "client": "t", "inverse": edit["ok"]["inverse"]}));
    assert_eq!(read(), DESIGN);
    // A param's `when` line (its own line first) re-scoped to the token's
    // same branch, which the transaction adds; never its default.
    let line = s.write(&dir, json!([{"op": "set-param", "name": "PostButton.radius", "value": "radius.md"}]));
    assert!(line["ok"]["inverse"].is_string(), "{line}");
    let when = s.write(&dir, json!([{"op": "set-param", "name": "PostButton.radius", "when": compact, "value": "4px"}]));
    assert!(when["ok"]["inverse"].is_string(), "{when}");
    let with_when = read();
    let to_token = s.write(&dir, json!([
        {"op": "unset-param", "name": "PostButton.radius", "when": compact},
        {"op": "set-token", "name": "radius.md", "when": compact, "value": "4px"},
    ]));
    assert!(to_token["ok"]["inverse"].is_string(), "{to_token}");
    let text = read();
    assert!(text.contains("  radius.md = 12px
    when density=compact: 4px
"), "{text}");
    assert!(text.contains("  PostButton.radius = radius.md
") && !text.contains("compact: 4px

params"), "{text}");
    // And back: the token's branch goes, the param's `when` line returns.
    let to_param = s.write(&dir, json!([
        {"op": "unset-token-when", "name": "radius.md", "when": compact},
        {"op": "set-param", "name": "PostButton.radius", "when": compact, "value": "4px"},
    ]));
    assert!(to_param["ok"]["inverse"].is_string(), "{to_param}");
    assert_eq!(read(), with_when);
    s.ask(json!({"method": "undo", "client": "t", "inverse": to_param["ok"]["inverse"]}));
    assert_eq!(read(), text);
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

#[test]
fn round_two_a_dominated_token_an_ambiguous_line_branch_pins_and_an_icon_file() {
    // A `when` naming a token out of range, though no tuple picks it: at
    // its own line.
    let dir = app("dominated");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\naxes\n  density = regular | compact\n  size = small | large\n\ntheme\n  radius.md = 12px\n  radius.big = 80px\n  duration.fast = 150ms\n\nparams\n  PostButton.radius = 8px\n    when density=compact: radius.big\n    when density=compact size=small: 8px\n    when density=compact size=large: 8px\n",
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(
        (e.id.as_ref(), e.span.line),
        ("tamber-param-range", 14),
        "{e:?}"
    );
    // A conditional literal no longer follows the default's token there.
    std::fs::write(
        dir.join("app.tamber"),
        format!(
            "{DESIGN}\nparams\n  PostButton.radius = radius.md\n    when density=compact: 8px\n"
        ),
    )
    .unwrap();
    let mut s = Serve::start(&dir);
    let described = s.ask(json!({"method": "describe"}));
    let radius = &described["ok"]["ir"]["components"][0]["params"][0];
    assert_eq!(radius["pinned"], json!(["radius.md"]), "{radius}");
    drop(s);
    // A short line grown ambiguous: another write is refused until it is
    // removed, which its own spelling may always do.
    let amb = std::env::temp_dir().join(format!("exact2-set-param-amb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&amb);
    std::fs::create_dir_all(amb.join("b")).unwrap();
    std::fs::write(amb.join("a.contract"), "component Card\n  params\n    inset: length = \"4px\"\n  view\n    column padding=\"param.inset\"\n      text \"a\"\n").unwrap();
    std::fs::write(
        amb.join("b/c.contract"),
        "component Card\n  view\n    column\n      text \"b\"\n",
    )
    .unwrap();
    std::fs::write(
        amb.join("app.contract"),
        "use Card from \"./a.contract\"\nuse Card as Other from \"./b/c.contract\"\n\ncomponent App\n  view\n    column\n      Card()\n      Other()\n",
    )
    .unwrap();
    std::fs::write(
        amb.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n\nparams\n  Card.inset = 9px\n",
    )
    .unwrap();
    let mut s = Serve::start(&amb);
    let other = s.write(
        &amb,
        json!([{"op": "set-param", "name": "a.contract#Card.inset", "value": "4px"}]),
    );
    assert_eq!(
        other["error"]["diagnostics"][0]["id"], "tamber-param-ambiguous",
        "{other}"
    );
    let repair = s.write(&amb, json!([{"op": "unset-param", "name": "Card.inset"}]));
    assert!(repair["ok"].is_object(), "{repair}");
    drop(s);
    // An icon param set to an SVG the app lacks is refused.
    let icons = std::env::temp_dir().join(format!("exact2-set-param-svg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&icons);
    std::fs::create_dir_all(&icons).unwrap();
    std::fs::write(
        icons.join("app.contract"),
        "component App\n  params\n    glyph: icon = \"icon(sf \\\"star\\\")\"\n  view\n    column\n      image \"symbol:param.glyph\"\n",
    )
    .unwrap();
    std::fs::write(
        icons.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n",
    )
    .unwrap();
    let mut s = Serve::start(&icons);
    let svg = s.write(
        &icons,
        json!([{"op": "set-param", "name": "App.glyph", "value": "icon(svg \"missing.svg\")"}]),
    );
    assert_eq!(
        svg["error"]["diagnostics"][0]["id"], "tamber-icon-svg",
        "{svg}"
    );
}

/// The server keeps the app's params and its own `.contract` files from
/// one walk of the Contract sources, walked again when one of them is
/// edited: a param a component gains resolves, a range it narrows refuses,
/// and a file a new `use` reaches is the server's.
#[test]
fn a_contract_edit_is_read_again_by_the_next_request() {
    let dir = app("walk");
    let mut s = Serve::start(&dir);
    let gap = s.ask(json!({"method": "resolve", "token": "PostButton.gap"}));
    assert!(gap["error"].is_object(), "no param yet: {gap}");
    let post = VIEW.split("component App").next().unwrap();
    std::fs::write(
        dir.join("post.contract"),
        post.replace(
            "    fade: duration",
            "    gap: length = \"radius.md\"  range=\"0..40\"\n    fade: duration",
        )
        .replace("range=\"0..40\"  control", "range=\"0..4\"  control")
        .replace(
            "    column border-radius",
            "    column gap=\"param.gap\" border-radius",
        ),
    )
    .unwrap();
    let gap = s.ask(json!({"method": "resolve", "token": "PostButton.gap"}));
    assert_eq!(gap["ok"]["token"], "post.contract#PostButton.gap", "{gap}");
    let wide = s.ask(json!({"method": "propose", "ops": [{"op": "set-param", "name": "PostButton.radius", "value": "8px"}]}));
    assert!(wide["error"].is_object(), "the narrowed range: {wide}");
    let files = |s: &mut Serve| s.ask(json!({"method": "hello"}))["ok"]["files"].clone();
    assert!(files(&mut s).get("badge.contract").is_none());
    std::fs::write(
        dir.join("badge.contract"),
        "component Badge\n  view\n    text \"b\"\n",
    )
    .unwrap();
    let root = std::fs::read_to_string(dir.join("app.contract")).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        format!("use Badge from \"./badge.contract\"\n{root}").replace(
            "      PostButton()\n      PostButton()\n",
            "      PostButton()\n      PostButton()\n      Badge()\n",
        ),
    )
    .unwrap();
    assert!(
        files(&mut s).get("badge.contract").is_some(),
        "a file a new use reaches"
    );
}

/// Review round 1 of the walk: what decides the params and the server's own
/// files beyond the Contract sources' bytes is read again too: a native
/// component's params in the design, a `package.json` that makes a file a
/// package's, and a root that comes after the server started.
#[test]
fn the_walk_follows_the_design_a_manifest_and_a_late_root() {
    let dir = std::env::temp_dir().join(format!("exact2-walk-inputs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("vendor/p")).unwrap();
    let card = |inset: &str| {
        format!("tamber t\n\ntheme\n  ink = #111111\n\ncomponent MapCard\n  params\n    inset: length = {inset}  in 0..24\n")
    };
    std::fs::write(dir.join("app.tamber"), card("8px")).unwrap();
    let mut s = Serve::start(&dir);
    let files = |s: &mut Serve| s.ask(json!({"method": "hello"}))["ok"]["files"].clone();
    assert!(files(&mut s).get("app.contract").is_none(), "no root yet");
    std::fs::write(
        dir.join("vendor/p/index.contract"),
        "component Badge\n  params\n    gap: length = \"4px\"  range=\"0..40\"\n  view\n    text \"b\" padding=\"param.gap\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Badge from \"./vendor/p/index.contract\"\n\ncomponent App\n  view\n    column\n      text \"a\" color=\"ink\"\n      Badge()\n",
    )
    .unwrap();
    let now = files(&mut s);
    assert!(now.get("app.contract").is_some(), "the late root: {now}");
    assert!(now.get("vendor/p/index.contract").is_some(), "{now}");
    let inset = |s: &mut Serve| {
        let r = s.ask(json!({"method": "resolve", "token": "MapCard.inset"}));
        r["ok"]["tuples"][0]["value"].clone()
    };
    assert_eq!(inset(&mut s), "8px");
    std::fs::write(dir.join("app.tamber"), card("12px")).unwrap();
    assert_eq!(inset(&mut s), "12px", "the design's default");
    let gap = |s: &mut Serve| {
        s.ask(json!({"method": "resolve", "token": "Badge.gap"}))["ok"]["token"].clone()
    };
    assert_eq!(gap(&mut s), "vendor/p/index.contract#Badge.gap");
    std::fs::write(dir.join("vendor/p/package.json"), "{\"name\": \"p\"}").unwrap();
    let now = files(&mut s);
    assert!(
        now.get("vendor/p/index.contract").is_none(),
        "a package's now: {now}"
    );
    assert_eq!(
        gap(&mut s),
        "p/index.contract#Badge.gap",
        "named by its package now"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
