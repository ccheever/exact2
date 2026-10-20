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
    let (written, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&dir.join("app.tamber"))}, "ops": [{"op": "set-token", "name": "gap", "value": "#444444"}]}));
    assert_eq!(written["ok"]["class"], "rebuild", "{written}");
    let failed = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(failed.id, "tamber-kind", "{failed:?}");
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
