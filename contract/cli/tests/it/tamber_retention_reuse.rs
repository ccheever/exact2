//! The design server's retention after a name-moving edit (LLP 1121 stage
//! 12 B1's follow-up, theme2's 11:32 ruling): built only as far as the
//! design's retention, and on an edit the design alone moved, with the last
//! build's types and expansion reused where the program loads as it did.
//! For every edit kind B1's re-checks covered, a primed server answers as a
//! fresh one does, and the reused types and expansion are what checking
//! again gives (`EXACT_TAMBER_RETENTION_VERIFY`, condition (b)).
#![cfg(unix)]

use super::tamber_package_params::{package, reading};
use super::tamber_serve::{digest, Serve};
use serde_json::{json, Value};
use std::path::PathBuf;

/// An edit between the primed server's first answer and its next.
enum Edit<'a> {
    /// A file of the app written: its path from the app's directory.
    Write(&'a str, &'a str),
    /// Several, at once.
    Writes(&'a [(&'a str, &'a str)]),
    /// A `set-param` the server writes.
    SetParam(&'a str, &'a str),
}

fn propose(name: &str) -> Value {
    json!({"method": "propose", "ops": [{"op": "set-token", "name": name, "value": "#444444"}]})
}

/// Each answer with what a fresh server's can't share (the files' digests)
/// left out.
fn bare(mut v: Value) -> Value {
    if let Some(ok) = v.get_mut("ok").and_then(Value::as_object_mut) {
        ok.remove("files");
    }
    v.as_object_mut().unwrap().remove("id");
    v
}

/// Prime a server on the app `setup` makes, make `edit`, and ask each of
/// `probes`; then ask them of a fresh server. The two answer alike, and the
/// primed server's builds are `built` (each line of its log). Run twice:
/// as the server runs, reusing (what the answers come from), and verifying
/// that what it reused is what checking again gives (condition (b)).
fn primed_as_fresh(setup: &dyn Fn(&str) -> PathBuf, edit: Edit, probes: &[&str], built: &[&str]) {
    for verify in [false, true] {
        let dir = setup(if verify { "verify" } else { "reuse" });
        let log = dir.join("retention.log");
        let mut env = vec![("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())];
        if verify {
            env.push(("EXACT_TAMBER_RETENTION_VERIFY", "1"));
        }
        let mut primed = Serve::start(&dir, &env);
        // (Refused where the app starts broken: a repair's case.)
        let _ = primed.ask(propose(probes[0]));
        match &edit {
            Edit::Write(file, text) => std::fs::write(dir.join(file), text).unwrap(),
            Edit::Writes(all) => {
                for (file, text) in *all {
                    std::fs::create_dir_all(dir.join(file).parent().unwrap()).unwrap();
                    std::fs::write(dir.join(file), text).unwrap();
                }
            }
            Edit::SetParam(name, value) => {
                let design = dir.join("app.tamber");
                let (set, _) = primed.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&design)}, "ops": [{"op": "set-param", "name": name, "value": value}]}));
                assert!(set.get("ok").is_some(), "{set}");
            }
        }
        let answers: Vec<Value> = probes
            .iter()
            .map(|p| bare(primed.ask(propose(p)).0))
            .collect();
        drop(primed);
        let lines = std::fs::read_to_string(&log).unwrap();
        let lines: Vec<&str> = lines.lines().collect();
        assert_eq!(
            lines, built,
            "the primed server's builds (verify: {verify})"
        );
        let mut fresh = Serve::start(&dir, &[]);
        for (p, primed) in probes.iter().zip(answers) {
            let (expected, _) = fresh.ask(propose(p));
            assert_eq!(primed, bare(expected), "{p} (verify: {verify})");
        }
        drop(fresh);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

const PIN: &str = "tamber t\n\ntheme\n  ink = #111111\n  ink2 = #222222\n  reading.pin = reading.pin.base\n  reading.pin.base = #333333\n";

/// The app `reading` makes, its design `design`, and a colour row of an
/// expression over its token `over`: a synthetic token (D3) each answer
/// lists.
fn with(tag: &str, design: &str, over: &str) -> PathBuf {
    let dir = reading(tag);
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        format!("use Card from \"@acme/reading\"\ncomponent App\n  view\n    column background-color=\"color-mix(in srgb, {over} 50%, white)\"\n      Card()\n"),
    )
    .unwrap();
    dir
}

#[test]
fn an_alias_moved_reuses_the_program_and_answers_as_fresh() {
    let moved = PIN.replace("reading.pin = reading.pin.base", "reading.pin = ink2");
    primed_as_fresh(
        &|m| with(&format!("ret-alias-{m}"), PIN, "ink"),
        Edit::Write("app.tamber", &moved),
        &["ink", "ink2", "reading.pin.base"],
        &["built", "built reused"],
    );
}

#[test]
fn a_param_set_to_a_token_reuses_the_program_and_answers_as_fresh() {
    primed_as_fresh(
        &|m| with(&format!("ret-set-param-{m}"), PIN, "ink"),
        Edit::SetParam("Map.pin", "ink"),
        &["ink", "ink2"],
        &["built", "built reused"],
    );
}

#[test]
fn a_role_alias_moved_by_case_reuses_the_program_and_answers_as_fresh() {
    let design = |to: &str| {
        format!("tamber t\n\naxes\n  mode = a | b\n\ntheme\n  old = #111111\n  fresh = #222222\n  Canvas = old\n  CanvasText = fresh\n  pick = {to}\n\nparams\n  Map.pin = pick\n")
    };
    primed_as_fresh(
        &|m| with(&format!("ret-case-{m}"), &design("CANVAS"), "fresh"),
        Edit::Write("app.tamber", &design("CANVASTEXT")),
        &["old", "fresh"],
        &["built", "built reused"],
    );
}

const ICONS: &str = "tamber t\n\ntheme\n  ink = #111111\n  paper = #ffffff\n  ground = #eeeeee\n  text = #000000  pair=paper\n\nicons\n  icon.brand = icon(sf \"star\")\n";

#[test]
fn an_icon_moved_to_a_token_answers_as_fresh() {
    let moved = "tamber t\n\ntheme\n  ink = #111111\n  paper = #ffffff\n  ground = #eeeeee\n  text = #000000  pair=paper\n  icon.brand = icon(sf \"star\")\n";
    primed_as_fresh(
        &|m| with(&format!("ret-icon-{m}"), ICONS, "ink"),
        Edit::Write("app.tamber", moved),
        &["ink", "paper"],
        &["built", "built reused"],
    );
}

#[test]
fn a_pair_moved_answers_as_fresh() {
    primed_as_fresh(
        &|m| with(&format!("ret-pair-{m}"), ICONS, "ink"),
        Edit::Write("app.tamber", &ICONS.replace("pair=paper", "pair=ground")),
        &["ink", "ground"],
        &["built", "built reused"],
    );
}

#[test]
fn a_header_moved_answers_as_fresh() {
    primed_as_fresh(
        &|m| with(&format!("ret-header-{m}"), PIN, "ink"),
        Edit::Write("app.tamber", &PIN.replace("tamber t", "tamber u")),
        &["ink", "ink2"],
        &["built", "built reused"],
    );
}

const LIB: &str = "tamber reading\n\ntheme\n  reading.gap: length = space.md\n  reading.pin = #112233\n  reading.alt = #445566\n\ncomponent Map\n  params\n    pin: color = reading.pin\n";

fn with_lib(tag: &str, lib: &str) -> PathBuf {
    let dir = with(tag, PIN, "ink");
    package(&dir, "reading", super::tamber_package_params::CARD, lib);
    dir
}

#[test]
fn a_package_alias_moved_reuses_the_program_and_answers_as_fresh() {
    primed_as_fresh(
        &|m| with_lib(&format!("ret-package-{m}"), LIB),
        Edit::Write(
            "node_modules/@acme/reading/reading.tamber",
            &LIB.replace("reading.pin = #112233", "reading.pin = reading.alt"),
        ),
        &["ink", "ink2"],
        &["built", "built reused"],
    );
}

/// A package design's header repaired (B1's re-check: the header keys): a
/// design that names another namespace, then its own.
#[test]
fn a_package_header_repaired_answers_as_fresh() {
    primed_as_fresh(
        &|m| {
            with_lib(
                &format!("ret-package-header-{m}"),
                &LIB.replacen("tamber reading", "tamber reading2", 1),
            )
        },
        Edit::Write("node_modules/@acme/reading/reading.tamber", LIB),
        &["ink", "ink2"],
        &["built", "built reused"],
    );
}

/// A design that doesn't parse, then repaired (condition (c)): the broken
/// build checks the program again, never reusing past it.
#[test]
fn a_design_repaired_after_a_parse_failure_answers_as_fresh() {
    for verify in [false, true] {
        let dir = with(&format!("ret-broken-{verify}"), PIN, "ink");
        let log = dir.join("retention.log");
        let mut env = vec![("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())];
        if verify {
            env.push(("EXACT_TAMBER_RETENTION_VERIFY", "1"));
        }
        let mut primed = Serve::start(&dir, &env);
        assert!(primed.ask(propose("ink")).0.get("ok").is_some());
        std::fs::write(dir.join("app.tamber"), "tamber t\n\ntheme\n  ink #111111\n").unwrap();
        assert!(primed.ask(propose("ink")).0.get("ok").is_none());
        let moved = PIN.replace("reading.pin = reading.pin.base", "reading.pin = ink2");
        std::fs::write(dir.join("app.tamber"), &moved).unwrap();
        let answers: Vec<Value> = ["ink", "ink2"]
            .iter()
            .map(|p| bare(primed.ask(propose(p)).0))
            .collect();
        drop(primed);
        let lines = std::fs::read_to_string(&log).unwrap();
        // The broken build checks the program again (no reuse past a design
        // that doesn't parse), and keeps what it checked: the repaired one,
        // loading as that did, reuses it.
        assert_eq!(
            lines.lines().collect::<Vec<_>>(),
            ["built", "built", "built reused"],
            "{lines}"
        );
        let mut fresh = Serve::start(&dir, &[]);
        for (p, primed) in ["ink", "ink2"].iter().zip(answers) {
            assert_eq!(primed, bare(fresh.ask(propose(p)).0), "{p}");
        }
        drop(fresh);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A motion token's alias moved: its value is written into the views at
/// load, so the program no longer loads as it did, and the build checks it
/// again (condition (a)'s compared fields).
#[test]
fn a_motion_alias_moved_checks_the_program_again() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  dur.fast = 200ms\n  dur.slow = 400ms\n";
    let setup = |m: &str| {
        let dir = with(&format!("ret-motion-{m}"), design, "ink");
        std::fs::write(
            dir.join("app.contract"),
            "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column transition-duration=\"dur.fast\"\n      Card()\n",
        )
        .unwrap();
        dir
    };
    primed_as_fresh(
        &setup,
        Edit::Write(
            "app.tamber",
            &design.replace("dur.fast = 200ms", "dur.fast = dur.slow"),
        ),
        &["ink"],
        &["built", "built"],
    );
}

/// A Contract source edited: its key moves, and the build checks it again.
#[test]
fn a_contract_edit_checks_the_program_again() {
    primed_as_fresh(
        &|m| with(&format!("ret-contract-{m}"), PIN, "ink"),
        Edit::Write(
            "app.contract",
            "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column background-color=\"color-mix(in srgb, ink2 50%, white)\"\n      Card()\n",
        ),
        &["ink", "ink2"],
        &["built", "built"],
    );
}

/// A package's Contract source edited: the sources' key moves, so nothing
/// is offered and the build checks the program again (the strings and
/// motion cases reach the gate itself).
#[test]
fn a_package_contract_edit_checks_the_program_again() {
    primed_as_fresh(
        &|m| with(&format!("ret-package-contract-{m}"), PIN, "ink"),
        Edit::Write(
            "node_modules/@acme/reading/index.contract",
            &super::tamber_package_params::CARD.replace("text \"c\"", "text \"d\""),
        ),
        &["ink", "ink2"],
        &["built", "built"],
    );
}

/// The strings tables edited with an alias move: the program reloads with
/// other strings, which types read, so the build checks it again (the
/// gate compares them; the sources' key doesn't hold them).
#[test]
fn strings_edited_with_an_alias_move_check_the_program_again() {
    let setup = |m: &str| {
        let dir = with(&format!("ret-strings-{m}"), PIN, "ink");
        std::fs::create_dir_all(dir.join("strings")).unwrap();
        std::fs::write(dir.join("strings/en.json"), r#"{"hello": "Hello"}"#).unwrap();
        dir
    };
    let moved = PIN.replace("reading.pin = reading.pin.base", "reading.pin = ink2");
    primed_as_fresh(
        &setup,
        Edit::Writes(&[
            ("strings/en.json", r#"{"hello": "Hi", "bye": "Bye"}"#),
            ("app.tamber", &moved),
        ]),
        &["ink", "ink2"],
        &["built", "built"],
    );
}

/// A motion token passed only as a component's argument: the loader reads
/// it there without writing it into the views, so the program loads as it
/// did and is reused; retention, read from the design, answers as fresh.
#[test]
fn a_motion_argument_alias_moved_reuses_the_program_and_answers_as_fresh() {
    let design = "tamber t\n\ntheme\n  ink = #111111\n  dur.fast = 200ms\n  dur.slow = 400ms\n";
    let setup = |m: &str| {
        let dir = with(&format!("ret-motion-arg-{m}"), design, "ink");
        std::fs::write(
            dir.join("app.contract"),
            "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column background-color=\"color-mix(in srgb, ink 50%, white)\"\n      Fade(d=\"dur.fast\")\n        Card()\ncomponent Fade\n  slot\n  props\n    d: string\n  view\n    column transition-duration=d\n      children\n",
        )
        .unwrap();
        dir
    };
    primed_as_fresh(
        &setup,
        Edit::Write(
            "app.tamber",
            &design.replace("dur.fast = 200ms", "dur.fast = dur.slow"),
        ),
        &["ink"],
        &["built", "built reused"],
    );
}

/// A build that stops before the design (a Contract type error): it says
/// no retention, nothing is cached or kept, every propose builds again and
/// is answered `rebuild`, as on main.
#[test]
fn a_build_stopping_before_the_design_caches_nothing() {
    let dir = with("ret-before-design", PIN, "ink");
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    text missing\n",
    )
    .unwrap();
    let log = dir.join("retention.log");
    let mut s = Serve::start(
        &dir,
        &[("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())],
    );
    for _ in 0..2 {
        let (r, _) = s.ask(propose("ink"));
        assert_eq!(r["ok"]["class"], "rebuild", "{r}");
    }
    drop(s);
    let lines = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        lines.lines().collect::<Vec<_>>(),
        ["built", "built"],
        "{lines}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The oracle the answers above stand for (Astra's round 2): a reused
/// retention build's retention and synthetic candidates are exactly what an
/// ordinary whole build of the same files fixes, for six edit kinds the
/// design alone moves (the package and motion-argument cases are compared
/// by their answers above).
#[test]
fn the_retention_build_says_what_the_whole_build_does() {
    let case = |m: &str| {
        format!("tamber t\n\naxes\n  mode = a | b\n\ntheme\n  old = #111111\n  fresh = #222222\n  Canvas = old\n  CanvasText = fresh\n  pick = {m}\n\nparams\n  Map.pin = pick\n")
    };
    let moved = ICONS.replace("\n\nicons\n", "\n");
    let cases: Vec<(&str, String, String, &str)> = vec![
        (
            "alias",
            PIN.into(),
            PIN.replace("reading.pin = reading.pin.base", "reading.pin = ink2"),
            "ink",
        ),
        (
            "set-param",
            PIN.into(),
            format!("{PIN}\nparams\n  Map.pin = ink\n"),
            "ink",
        ),
        ("case", case("CANVAS"), case("CANVASTEXT"), "fresh"),
        ("icon", ICONS.into(), moved, "ink"),
        (
            "pair",
            ICONS.into(),
            ICONS.replace("pair=paper", "pair=ground"),
            "ink",
        ),
        (
            "header",
            PIN.into(),
            PIN.replace("tamber t", "tamber u"),
            "ink",
        ),
    ];
    for (tag, before, after, over) in cases {
        let dir = with(&format!("ret-oracle-{tag}"), &before, over);
        let root = dir.join("app.contract");
        let first = contract::retention_build(&root, None);
        assert!(first.retention.is_some() && !first.reused, "{tag}");
        std::fs::write(dir.join("app.tamber"), &after).unwrap();
        let second = contract::retention_build(&root, first.kept);
        assert!(second.reused, "{tag}: the design alone moved");
        let _ = contract::compile_path_all(&root, false);
        let whole = (
            contract_lower::retention(),
            contract_lower::synthetic_candidates(),
        );
        assert!(!whole.1.is_empty(), "{tag}: a synthetic expression");
        assert_eq!((second.retention, second.candidates), whole, "{tag}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// theme2's Q2 as corrected at 11:54: a build that fails after the design
/// still says its retention, which is cached; a kind change is not hidden. The server
/// never compiled a write's Contract (main's too): a token made a colour
/// where a row reads a length is classed `rebuild`, so the dev loop's whole
/// build says the refusal, as it does here; and the retention that build
/// stopped after is kept, a second propose building nothing.
#[test]
fn an_edit_failing_after_the_design_is_classed_rebuild_while_retention_is_kept() {
    let dir = with(
        "ret-fails-after",
        "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n",
        "ink",
    );
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column padding=\"gap\"\n      Card()\n",
    )
    .unwrap();
    let log = dir.join("retention.log");
    let builds = || std::fs::read_to_string(&log).map_or(0, |t| t.lines().count());
    let mut s = Serve::start(
        &dir,
        &[("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())],
    );
    let (ok, _) = s.ask(propose("ink"));
    assert!(ok.get("ok").is_some(), "{ok}");
    // The rows check (theme2's 14:19 ruling) refuses that write now, in the
    // compiler's words; made on disk past the server, the build refuses it.
    let (written, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [{"op": "set-token", "name": "gap", "value": "#444444"}]}));
    assert_eq!(
        written["error"]["diagnostics"][0]["id"], "tamber-kind",
        "{written}"
    );
    edit(&dir, "gap = 8px", "gap = #444444");
    let failed = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(failed.id, "tamber-kind", "{failed:?}");
    assert_eq!(
        written["error"]["diagnostics"][0]["message"], failed.message,
        "the compiler's words"
    );
    // The write moved a kind, which keys: the next propose builds, as far
    // as the retention of a build that then fails, and the one after hits
    // the cache (one line for the two).
    let n = builds();
    for _ in 0..2 {
        let (again, _) = s.ask(propose("ink"));
        assert!(again.get("ok").is_some(), "{again}");
    }
    assert_eq!(builds(), n + 1, "the failing build's retention is kept");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The press-scale app: `scale` an alias of `one`, a button's press scale,
/// with or without a synthetic expression (`over`).
fn press(tag: &str, over: bool) -> PathBuf {
    let dir = super::tamber_serve::app(
        tag,
        "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n  neg = -1\n  zero = 0\n  scale = one\n",
    );
    let colour = if over {
        " background-color=\"color-mix(in srgb, ink 50%, white)\""
    } else {
        ""
    };
    std::fs::write(
        dir.join("app.contract"),
        format!("component App\n  view\n    column{colour}\n      button \"Go\" -exact-press-scale=\"scale\"\n"),
    )
    .unwrap();
    dir
}

/// theme2's 12:15 ruling (option (4)): a design-valid move whose whole build
/// fails after the design (a negative press scale) is answered from the
/// stopped build at once, and once the background build reports the
/// failure, every later edit is `rebuild`, as on main; moving it back
/// answers `live` again. With or without synthetic expressions.
#[test]
fn a_build_failing_after_the_design_is_rebuild_once_the_check_reports() {
    for over in [false, true] {
        let dir = press(&format!("ret-press-{over}"), over);
        // Outside the app: a file in it is an input the check fingerprints.
        let log = PathBuf::from(format!("{}.check.log", dir.display()));
        let _ = std::fs::remove_file(&log);
        let mut s = Serve::start(&dir, &[("EXACT_TAMBER_CHECK_LOG", log.to_str().unwrap())]);
        let class = |s: &mut Serve| s.ask(propose("ink")).0["ok"]["class"].clone();
        assert_eq!(class(&mut s), "live");
        let set = |s: &mut Serve, to: &str| {
            let (w, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [{"op": "set-token", "name": "scale", "value": to}]}));
            assert!(w.get("ok").is_some(), "{w}");
        };
        // Past the server (an editor): the write check would refuse it.
        edit(&dir, "scale = one", "scale = neg");
        // The answer right after the move is the stopped build's; once the
        // check fails, `rebuild`, and it stays.
        assert_eq!(class(&mut s), "live", "over: {over}");
        until(&mut s, "rebuild");
        seen(&log, "failed");
        assert_eq!(class(&mut s), "rebuild", "over: {over}");
        set(&mut s, "one");
        assert_eq!(class(&mut s), "live", "over: {over}");
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The server's close never waits on a background build (condition (c)).
#[test]
fn closing_the_server_mid_check_exits_at_once() {
    let dir = press("ret-close", false);
    // A gate never opened: the check waits for good.
    let gate = PathBuf::from(format!("{}.gate", dir.display()));
    let _ = std::fs::remove_file(&gate);
    let mut s = Serve::start(&dir, &[("EXACT_TAMBER_CHECK_GATE", gate.to_str().unwrap())]);
    assert!(s.ask(propose("ink")).0.get("ok").is_some());
    assert!(
        s.close(std::time::Duration::from_secs(5)),
        "the server waited on its check"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Wait until `log` has a line `line`.
fn seen(log: &std::path::Path, line: &str) -> String {
    for _ in 0..600 {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        if text.lines().any(|l| l == line) {
            return text;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("no check said `{line}`");
}

/// Ask until the answer's class is `want`: the background check's word is
/// read on an answer after its result, however its builds were coalesced.
fn until(s: &mut Serve, want: &str) {
    for _ in 0..200 {
        if s.ask(propose("ink")).0["ok"]["class"] == want {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("never `{want}`");
}

/// theme2's 12:19 amendment to (b): while the build is broken, an unrelated
/// value edit (same retention key) still answers `rebuild`, as on main; a
/// value edit that fixes it returns the state to known once a whole build of
/// exactly the files as they are passes.
#[test]
fn a_value_edit_while_broken_is_rebuild_and_a_fixing_one_returns_to_live() {
    let dir = press("ret-press-fix", false);
    // Outside the app: a file in it is an input the check fingerprints.
    let log = PathBuf::from(format!("{}.check.log", dir.display()));
    let _ = std::fs::remove_file(&log);
    let mut s = Serve::start(&dir, &[("EXACT_TAMBER_CHECK_LOG", log.to_str().unwrap())]);
    let class = |s: &mut Serve| s.ask(propose("ink")).0["ok"]["class"].clone();
    let write = |s: &mut Serve, name: &str, to: &str| {
        let (w, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [{"op": "set-token", "name": name, "value": to}]}));
        assert!(w.get("ok").is_some(), "{w}");
    };
    assert_eq!(class(&mut s), "live");
    edit(&dir, "scale = one", "scale = zero");
    until(&mut s, "rebuild");
    seen(&log, "failed");
    // An unrelated value edit: the key stays, the build is still broken.
    write(&mut s, "ink", "#555555");
    assert_eq!(class(&mut s), "rebuild");
    // The value that breaks it fixed (`0` to `2`, a number either way, so
    // the key stays): rebuild until the check of exactly these files
    // passes, then known.
    write(&mut s, "zero", "2");
    assert_eq!(class(&mut s), "rebuild");
    until(&mut s, "live");
    seen(&log, "passed");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A result raced by an edit mid-build is dropped: the check started for a
/// broken design, which an edit fixes while it builds, says `lost`, never
/// `passed`, and the state stays as the stopped build made it.
#[test]
fn a_check_raced_by_an_edit_mid_build_is_dropped() {
    let dir = press("ret-press-race", false);
    let gate = PathBuf::from(format!("{}.gate", dir.display()));
    let _ = std::fs::remove_file(&gate);
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n  neg = -1\n  scale = neg\n",
    )
    .unwrap();
    // Outside the app: a file in it is an input the check fingerprints.
    let log = PathBuf::from(format!("{}.check.log", dir.display()));
    let _ = std::fs::remove_file(&log);
    let mut s = Serve::start(
        &dir,
        &[
            ("EXACT_TAMBER_CHECK_LOG", log.to_str().unwrap()),
            ("EXACT_TAMBER_CHECK_GATE", gate.to_str().unwrap()),
        ],
    );
    assert!(s.ask(propose("ink")).0.get("ok").is_some());
    // Mid-build: its files read the first time, held at the gate.
    seen(&log, "began");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n  neg = -1\n  scale = one\n",
    )
    .unwrap();
    std::fs::write(&gate, "").unwrap();
    let lines = seen(&log, "lost");
    assert_eq!(
        lines.lines().collect::<Vec<_>>(),
        ["began", "lost"],
        "{lines}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A check lost to an edit is asked for again (Astra's round 1): checking a
/// broken design, an unrelated value edit (same retention key) lands while
/// it builds; the lost check's files are built again as they are now, and
/// the answers become `rebuild`.
#[test]
fn a_lost_check_is_asked_for_again() {
    let dir = press("ret-press-lost", false);
    let gate = PathBuf::from(format!("{}.gate", dir.display()));
    let _ = std::fs::remove_file(&gate);
    let open = || std::fs::write(&gate, "").unwrap();
    // Outside the app: a file in it is an input the check fingerprints.
    let log = PathBuf::from(format!("{}.check.log", dir.display()));
    let _ = std::fs::remove_file(&log);
    let mut s = Serve::start(
        &dir,
        &[
            ("EXACT_TAMBER_CHECK_LOG", log.to_str().unwrap()),
            ("EXACT_TAMBER_CHECK_GATE", gate.to_str().unwrap()),
        ],
    );
    let class = |s: &mut Serve| s.ask(propose("ink")).0["ok"]["class"].clone();
    let write = |s: &mut Serve, name: &str, to: &str| {
        let (w, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [{"op": "set-token", "name": name, "value": to}]}));
        assert!(w.get("ok").is_some(), "{w}");
    };
    assert_eq!(class(&mut s), "live");
    open();
    seen(&log, "passed");
    edit(&dir, "scale = one", "scale = zero");
    class(&mut s);
    // While that check is held at the gate (its second `began`), an
    // unrelated value edit; then the gate opens for every build.
    for _ in 0..600 {
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        if text.lines().filter(|l| *l == "began").count() >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    write(&mut s, "ink", "#555555");
    open();
    seen(&log, "lost");
    for _ in 0..200 {
        open();
        if class(&mut s) == "rebuild" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(class(&mut s), "rebuild");
    seen(&log, "failed");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// An open scrub's frames follow the check (Astra's round 1): a scrub
/// opened while the check of a broken design runs answers `rebuild` once
/// it reports, though the scrub cached its retention when it opened.
#[test]
fn an_open_scrub_follows_the_check() {
    let dir = press("ret-press-scrub", false);
    let gate = PathBuf::from(format!("{}.gate", dir.display()));
    let _ = std::fs::remove_file(&gate);
    // Outside the app: a file in it is an input the check fingerprints.
    let log = PathBuf::from(format!("{}.check.log", dir.display()));
    let _ = std::fs::remove_file(&log);
    let mut s = Serve::start(
        &dir,
        &[
            ("EXACT_TAMBER_CHECK_LOG", log.to_str().unwrap()),
            ("EXACT_TAMBER_CHECK_GATE", gate.to_str().unwrap()),
        ],
    );
    edit(&dir, "scale = one", "scale = zero");
    let files = json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let frame = |s: &mut Serve, open: bool, v: &str| {
        let (r, _) = s.ask(json!({"method": "scrub-fold", "open": open, "files": files, "ops": [{"op": "set-token", "name": "ink", "value": v}]}));
        r["ok"]["class"].clone()
    };
    assert_eq!(
        frame(&mut s, true, "#121212"),
        "live",
        "answered from the stopped build"
    );
    // The gate opened for every build (the first, of the files before the
    // write, is lost).
    for _ in 0..600 {
        std::fs::write(&gate, "").unwrap();
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        if text.lines().any(|l| l == "failed") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    seen(&log, "failed");
    until(&mut s, "rebuild");
    assert_eq!(frame(&mut s, false, "#131313"), "rebuild");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Grok's round 1: two writes and no propose between. The breaking write
/// sees the files before it; the next (an unrelated value) is the first to
/// see the new retention and starts the check of the files before it; that
/// failure, of files since moved, still marks the key, and the answers
/// become `rebuild`.
#[test]
fn a_failure_of_files_since_moved_still_marks_the_key() {
    let dir = press("ret-press-moved", false);
    let log = PathBuf::from(format!("{}.check.log", dir.display()));
    let _ = std::fs::remove_file(&log);
    let mut s = Serve::start(&dir, &[("EXACT_TAMBER_CHECK_LOG", log.to_str().unwrap())]);
    let write = |s: &mut Serve, name: &str, to: &str| {
        let (w, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [{"op": "set-token", "name": name, "value": to}]}));
        assert!(w.get("ok").is_some(), "{w}");
    };
    edit(&dir, "scale = one", "scale = zero");
    write(&mut s, "ink", "#555555");
    for _ in 0..100 {
        if s.ask(propose("ink")).0["ok"]["class"] == "rebuild" {
            drop(s);
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("the failure was dropped");
}

/// A scrub opened under a retention the files have since left never builds
/// them under its key (Astra's round 2): the files changed on disk to a
/// passing design under another retention, the frames stay `rebuild`; the
/// files back, the failure stands.
#[test]
fn a_scrubs_frames_never_clear_a_key_the_files_left() {
    let dir = press("ret-press-scrub-left", false);
    let log = PathBuf::from(format!("{}.check.log", dir.display()));
    let _ = std::fs::remove_file(&log);
    let mut s = Serve::start(&dir, &[("EXACT_TAMBER_CHECK_LOG", log.to_str().unwrap())]);
    edit(&dir, "scale = one", "scale = zero");
    let broken = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    let files = json!({"app.tamber": digest(&dir.join("app.tamber"))});
    let frame = |s: &mut Serve, open: bool, v: &str| {
        let (r, _) = s.ask(json!({"method": "scrub-fold", "open": open, "files": files, "ops": [{"op": "set-token", "name": "ink", "value": v}]}));
        r["ok"]["class"].clone()
    };
    frame(&mut s, true, "#121212");
    // The check fails, and before any answer reads it, the alias moves
    // elsewhere to a passing value: another retention.
    seen(&log, "failed");
    std::fs::write(
        dir.join("app.tamber"),
        broken.replace("scale = zero", "scale = one"),
    )
    .unwrap();
    for v in ["#131313", "#141414", "#151515"] {
        assert_eq!(frame(&mut s, false, v), "rebuild");
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    // Back: the failure under its key stands.
    std::fs::write(dir.join("app.tamber"), &broken).unwrap();
    assert_eq!(s.ask(propose("ink")).0["ok"]["class"], "rebuild");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Edit the app's design on disk, past the server, as an editor does:
/// `from` to `to`.
fn edit(dir: &std::path::Path, from: &str, to: &str) {
    let path = dir.join("app.tamber");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{text}");
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
}
