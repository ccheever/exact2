//! @ref LLP 1121 stage 14 C2 — `iconFillRules` in the compatibility id: an
//! app whose plan keeps an even-odd icon (its `icons` row's path starts
//! `evenodd:`) needs a binary that reads the marker, so the input is
//! `["evenodd"]` and the id moves; an app with none has no such input, so
//! its id (and every app's today) is as it was. An absent key, not a null
//! one: a key in the inputs, even null, moves every id.

use std::path::Path;

#[cfg(test)]
thread_local! {
    /// Tests only: compute the compatibility id without this input's code
    /// path, as main did, to compare against.
    pub(crate) static SKIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The fill rules the app at `app_dir`'s plan needs a binary to read beyond
/// nonzero: `["evenodd"]` when it keeps an even-odd icon, else none. Read
/// from the plan that ships (C2 round 1, Astra 3 = Grok): the bake's
/// `out/app.plan` when it wrote one (`receipt::emit` requires it), else the
/// `app.contract` plan the compatibility id already compiled (`compiled`,
/// `colors::compiled`: no second compile), else a game's `app/app.contract`,
/// compiled here. A contract that is there and doesn't compile fails the id
/// (no fail-open: the rule could be missed). `entries`: the entry plans that
/// ship beside it (widgets, activities), each read the same way.
pub(crate) fn needed(
    app_dir: &Path,
    compiled: Option<&exact_plan::Plan>,
    out: Option<&Path>,
    entries: &[&exact_plan::Plan],
) -> Result<Option<Vec<&'static str>>, String> {
    #[cfg(test)]
    if SKIP.with(std::cell::Cell::get) {
        return Ok(None);
    }
    let other = shipped(app_dir, compiled, out)?;
    let main = other.as_ref().or(compiled);
    let mut any = false;
    for plan in main.into_iter().chain(entries.iter().copied()) {
        any |= even_odd(plan)?;
    }
    Ok(any.then(|| vec!["evenodd"]))
}

/// The plan that ships when it is not `compiled`, as [`needed`] reads it:
/// the bake's `out/app.plan` when it wrote one, else a game's
/// `app/app.contract` compiled here when there is no `compiled`; `None`
/// when `compiled` is the one, or there is none. A plan that is there and
/// doesn't decode or compile is an error.
pub(crate) fn shipped(
    app_dir: &Path,
    compiled: Option<&exact_plan::Plan>,
    out: Option<&Path>,
) -> Result<Option<exact_plan::Plan>, String> {
    let shipped = out.map(|o| o.join("app.plan")).filter(|p| p.is_file());
    match (shipped, compiled) {
        (Some(file), _) => {
            let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            Ok(Some(exact_plan::Plan::decode(&bytes).map_err(|e| {
                format!("the baked plan is invalid: {e:?}")
            })?))
        }
        (None, Some(_)) => Ok(None),
        (None, None) => {
            let game = app_dir.join("app/app.contract");
            if !game.is_file() {
                return Ok(None);
            }
            Ok(Some(
                contract::compile_path(&game).map_err(|e| format!("app/app.contract: {e}"))?,
            ))
        }
    }
}

/// The entry plans that ship beside the app's (widgets, activities:
/// `plans/<kind>.plan`): the caller's, else compiled once into `owned`
/// (moved out of compat.rs under its line cap).
pub(crate) fn entries<'a>(
    app_dir: &Path,
    given: Option<&'a std::collections::BTreeMap<String, contract::snapshot::Entry>>,
    owned: &'a mut Option<std::collections::BTreeMap<String, contract::snapshot::Entry>>,
) -> Result<&'a std::collections::BTreeMap<String, contract::snapshot::Entry>, String> {
    match given {
        Some(entries) => Ok(entries),
        None => Ok(owned.insert(contract::snapshot::entries(app_dir)?)),
    }
}

/// `iconFillRules` into `compat`'s inputs, and its id again, when a plan
/// that ships (the app's, read as [`needed`] reads it, or an entry's) keeps
/// an even-odd icon; nothing otherwise.
pub(crate) fn apply(
    compat: &mut crate::compat::Compat,
    app_dir: &Path,
    compiled: Option<&exact_plan::Plan>,
    entries: &std::collections::BTreeMap<String, contract::snapshot::Entry>,
) -> Result<(), String> {
    let out = std::env::var_os("OUT_DIR").map(std::path::PathBuf::from);
    let entry_plans: Vec<&exact_plan::Plan> = entries.values().map(|e| &e.plan).collect();
    if let Some(rules) = needed(app_dir, compiled, out.as_deref(), &entry_plans)? {
        compat.inputs["iconFillRules"] = serde_json::json!(rules);
        compat.id = crate::compat::compatibility_digest(&compat.inputs);
    }
    Ok(())
}

/// Whether `plan` draws an icon even-odd: a row of its `icons` table (what a
/// computed source names), a literal source among its strings (`symbol:`
/// and a form the build or an author wrote, which makes no row; C2 round 1,
/// Astra 2), or a source inside a baked value, a resource's initial or
/// placeholder value, whose strings are in the data pool, not interned
/// (round 2, Astra 2); each read through `exact_kernel::Icon`, the one reader.
/// A baked value that is there (not zero-length, which is none) and doesn't
/// decode fails, naming its resource and field (round 3): an unread value
/// could hold the rule.
pub(crate) fn even_odd(plan: &exact_plan::Plan) -> Result<bool, String> {
    let source = |text: &str| {
        text.strip_prefix("symbol:")
            .and_then(exact_kernel::Icon::parse)
            .is_some_and(|icon| icon.even_odd)
    };
    fn within(v: &exact_plan::Value, source: &dyn Fn(&str) -> bool) -> bool {
        match v {
            exact_plan::Value::List(items) | exact_plan::Value::Record(items) => {
                items.iter().any(|v| within(v, source))
            }
            exact_plan::Value::Option(Some(v)) => within(v, source),
            v => v.as_str().is_some_and(source),
        }
    }
    let mut baked = false;
    for r in plan.resources.iter() {
        for (field, bytes) in [
            ("initial", r.initial),
            ("placeholder_value", r.placeholder_value),
        ] {
            if bytes.len == 0 {
                continue;
            }
            let value = exact_plan::Value::from_bytes(plan.bytes(bytes)).map_err(|e| {
                format!(
                    "the plan's resource `{}` has a malformed {field} value ({e:?})",
                    plan.str(r.name)
                )
            })?;
            baked |= within(&value, &source);
        }
    }
    let row = plan
        .icons
        .iter()
        .any(|i| exact_runner::icon_of(plan, i).is_some_and(|icon| icon.even_odd));
    Ok(row || baked || plan.strings.iter().any(source))
}

#[cfg(test)]
mod tests {
    use super::even_odd;

    /// A plan with an even-odd icon needs the rule; one with only nonzero
    /// icons, or none, needs nothing (its compatibility id is unchanged).
    #[test]
    fn only_an_even_odd_icon_needs_the_rule() {
        let plan = |path: &str| {
            let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
            b.icon("icon.a", "", "0 0 24 24", path, "icons/a.svg");
            b.finish().unwrap()
        };
        assert!(even_odd(&plan("evenodd:M0 0h4v4H0Z")).unwrap());
        assert!(!even_odd(&plan("M0 0h4v4H0Z")).unwrap());
        assert!(!even_odd(&plan("")).unwrap());
        assert!(!even_odd(
            &exact_plan::builder::PlanBuilder::new(0, 0)
                .finish()
                .unwrap()
        )
        .unwrap());
    }

    fn app(name: &str, rule: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("exact-icon-rules-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
        std::fs::write(
            dir.join("app.json"),
            r#"{"app":{"id":"dev.test.rules","name":"Rules"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("app.tamber"), "tamber t\n\ntheme\n  ink = #000000\n\nicons\n  icon.ring = icon(svg \"icons/ring.svg\")\n").unwrap();
        std::fs::write(dir.join("assets/icons/ring.svg"), format!(r#"<svg viewBox="0 0 24 24"><path fill-rule="{rule}" d="M2 2H22V22H2Z M8 8H16V16H8Z"/></svg>"#)).unwrap();
        std::fs::write(
            dir.join("app.contract"),
            "component App\n  view\n    image \"symbol:icon.ring\"\n",
        )
        .unwrap();
        dir
    }

    /// The app's compatibility id, through this input's code path or (`main`)
    /// without it, as main computes it.
    fn compat(dir: &std::path::Path, main: bool) -> crate::compat::Compat {
        let manifest = contract::Manifest::read(dir).unwrap();
        super::SKIP.with(|s| s.set(main));
        let compat = crate::compat::compatibility_id(
            dir,
            "macos",
            "aarch64-apple-darwin",
            &manifest,
            Some(""),
        );
        super::SKIP.with(|s| s.set(false));
        compat.unwrap()
    }

    /// theme2 (C2 round 1): a plan with no even-odd icon gets EXACTLY main's
    /// compatibility id and inputs (the id computed without this code path),
    /// with no `iconFillRules` key.
    #[test]
    fn a_plan_with_no_even_odd_icon_keeps_mains_compatibility_id() {
        let plain = app("plain", "nonzero");
        assert_eq!(
            super::needed(
                &plain,
                crate::colors::compiled(&plain).unwrap().as_ref(),
                None,
                &[]
            )
            .unwrap(),
            None
        );
        let (now, main) = (compat(&plain, false), compat(&plain, true));
        assert_eq!(now.id, main.id);
        assert_eq!(now.inputs, main.inputs);
        assert!(now.inputs.get("iconFillRules").is_none(), "{}", now.inputs);
        let _ = std::fs::remove_dir_all(plain);
    }

    /// theme2's stage 14 C ruling: the packages' moved icons are written
    /// nonzero (each fills the same either way), so an app using `@exact/cards`
    /// and `@exact/actions`, library-gate, keeps EXACTLY main's compatibility
    /// id and inputs, with no `iconFillRules`.
    #[test]
    fn library_gate_keeps_mains_compatibility_id() {
        let gate = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/library-gate");
        let compiled = crate::colors::compiled(&gate).unwrap();
        let plan = compiled.as_ref().expect("library-gate's plan");
        assert!(
            plan.icons
                .iter()
                .any(|i| plan.str(i.name) == "cards.verified-check"),
            "the moved icons are in its plan"
        );
        assert!(!even_odd(plan).unwrap());
        assert_eq!(super::needed(&gate, Some(plan), None, &[]).unwrap(), None);
        let (now, main) = (compat(&gate, false), compat(&gate, true));
        assert_eq!(now.id, main.id);
        assert_eq!(now.inputs, main.inputs);
        assert!(now.inputs.get("iconFillRules").is_none(), "{}", now.inputs);
    }

    /// And a plan that keeps an even-odd icon gets `iconFillRules:
    /// ["evenodd"]` and another id: main's inputs with that key alone added.
    #[test]
    fn a_plan_with_an_even_odd_icon_gets_the_rule_and_another_id() {
        let odd = app("odd", "evenodd");
        assert_eq!(
            super::needed(
                &odd,
                crate::colors::compiled(&odd).unwrap().as_ref(),
                None,
                &[]
            )
            .unwrap(),
            Some(vec!["evenodd"])
        );
        let (now, main) = (compat(&odd, false), compat(&odd, true));
        assert_eq!(now.inputs["iconFillRules"], serde_json::json!(["evenodd"]));
        assert!(main.inputs.get("iconFillRules").is_none());
        assert_ne!(now.id, main.id);
        let mut without = now.inputs.clone();
        without.as_object_mut().unwrap().remove("iconFillRules");
        assert_eq!(without, main.inputs, "nothing else differs");
        assert_eq!(crate::compat::compatibility_digest(&without), main.id);
        let _ = std::fs::remove_dir_all(odd);
    }

    /// C2 round 1 (Astra 2): a literal even-odd source with no tamber
    /// declaration makes no `icons` row, and still needs the rule: the
    /// compatibility id reads the plan's literal sources too.
    #[test]
    fn a_literal_even_odd_source_needs_the_rule_without_a_declaration() {
        let dir =
            std::env::temp_dir().join(format!("exact-icon-rules-literal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("app.json"),
            r#"{"app":{"id":"dev.test.rules","name":"Rules"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("app.contract"), "component App\n  view\n    image \"symbol:icon||0 0 24 24|evenodd:M2 2H22V22H2Z M8 8H16V16H8Z|\"\n").unwrap();
        let plan = crate::colors::compiled(&dir).unwrap().unwrap();
        assert!(plan.icons.is_empty(), "no row: {:?}", plan.icons);
        assert!(even_odd(&plan).unwrap());
        let (now, main) = (compat(&dir, false), compat(&dir, true));
        assert_eq!(now.inputs["iconFillRules"], serde_json::json!(["evenodd"]));
        assert_ne!(now.id, main.id);
        // The same literal nonzero: main's id.
        std::fs::write(
            dir.join("app.contract"),
            "component App\n  view\n    image \"symbol:icon||0 0 24 24|M2 2H22V22H2Z|\"\n",
        )
        .unwrap();
        assert_eq!(compat(&dir, false).id, compat(&dir, true).id);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// C2 round 1 (Astra 3 = Grok): the rule comes from the plan that ships:
    /// the bake's `out/app.plan` over a fresh compile of the contract; a
    /// game's `app/app.contract` that doesn't compile fails the id; an app
    /// with neither contract nor plan needs none.
    #[test]
    fn the_rule_comes_from_the_plan_that_ships_and_a_broken_contract_fails() {
        let plain = app("shipped", "nonzero");
        let compiled = crate::colors::compiled(&plain).unwrap();
        let out = plain.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        b.icon(
            "icon.ring",
            "",
            "0 0 24 24",
            "evenodd:M2 2H22V22H2Z M8 8H16V16H8Z",
            "icons/ring.svg",
        );
        std::fs::write(out.join("app.plan"), b.finish().unwrap().encode()).unwrap();
        assert_eq!(
            super::needed(&plain, compiled.as_ref(), Some(&out), &[]).unwrap(),
            Some(vec!["evenodd"]),
            "the baked plan's row"
        );
        assert_eq!(
            super::needed(&plain, compiled.as_ref(), None, &[]).unwrap(),
            None,
            "the contract's own"
        );
        std::fs::write(out.join("app.plan"), b"not a plan").unwrap();
        assert!(
            super::needed(&plain, compiled.as_ref(), Some(&out), &[]).is_err(),
            "an invalid baked plan fails"
        );
        let game =
            std::env::temp_dir().join(format!("exact-icon-rules-game-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        std::fs::create_dir_all(game.join("app")).unwrap();
        assert_eq!(
            super::needed(&game, None, None, &[]).unwrap(),
            None,
            "no contract, no plan"
        );
        std::fs::write(
            game.join("app/app.contract"),
            "component App\n  view\n    nonsense here\n",
        )
        .unwrap();
        let err = super::needed(&game, None, None, &[]).unwrap_err();
        assert!(err.starts_with("app/app.contract:"), "{err}");
        for dir in [plain, game] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// C2 round 2 (Astra 2): a source inside a baked value (a resource's
    /// initial value, the data source's answer at bake) is in the data pool,
    /// not among the plan's strings, and still needs the rule: the plan a
    /// bake writes is read whole, its resources' values walked.
    #[test]
    fn a_baked_answer_holding_an_even_odd_source_needs_the_rule() {
        use exact_runner::{DataError, DataSource, Value};
        struct Glyph(&'static str);
        impl DataSource for Glyph {
            fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
                assert_eq!(name, "glyph");
                Ok(Value::record(vec![Value::list(vec![Value::str(self.0)])]))
            }
        }
        let dir =
            std::env::temp_dir().join(format!("exact-icon-rules-baked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("out")).unwrap();
        std::fs::write(
            dir.join("app.json"),
            r#"{"app":{"id":"dev.test.rules","name":"Rules"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("app.contract"), "shape Glyph\n  srcs: string[]\ncomponent App\n  resource glyph = glyph() as shape Glyph\n  view\n    column\n      for s in glyph.srcs key = s\n        image s\n").unwrap();
        let compiled = crate::colors::compiled(&dir).unwrap().unwrap();
        assert!(
            !even_odd(&compiled).unwrap(),
            "nothing even-odd before the bake"
        );
        let odd = "symbol:icon||0 0 24 24|evenodd:M2 2H22V22H2Z M8 8H16V16H8Z|";
        for (answer, want) in [
            (odd, Some(vec!["evenodd"])),
            ("symbol:icon||0 0 24 24|M2 2H22V22H2Z|", None),
        ] {
            let baked = contract::bake(compiled.clone(), Glyph(answer)).unwrap();
            assert!(
                !baked.strings.iter().any(|s| s == answer),
                "the answer is a baked value, not an interned string"
            );
            std::fs::write(dir.join("out/app.plan"), baked.encode()).unwrap();
            assert_eq!(
                super::needed(&dir, Some(&compiled), Some(&dir.join("out")), &[]).unwrap(),
                want,
                "{answer}"
            );
        }
        // A value that is there and doesn't decode fails the id, named
        // (round 3); a zero-length one is none.
        let mut broken = contract::bake(compiled.clone(), Glyph(odd)).unwrap();
        let at = broken.data.len() as u32;
        broken.data.to_mut().extend_from_slice(&[0xff, 0xff, 0xff]);
        broken.resources[0].initial = exact_plan::Bytes { offset: at, len: 3 };
        let err = even_odd(&broken).unwrap_err();
        assert!(
            err.contains("resource `glyph`") && err.contains("malformed initial value"),
            "{err}"
        );
        std::fs::write(dir.join("out/app.plan"), broken.encode()).unwrap();
        assert!(
            super::needed(&dir, Some(&compiled), Some(&dir.join("out")), &[]).is_err(),
            "the compat id fails"
        );
        broken.resources[0].initial = exact_plan::Bytes { offset: 0, len: 0 };
        assert!(!even_odd(&broken).unwrap(), "a zero-length value is none");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// C2 round 2 (Astra 3): an even-odd icon a widget alone draws ships in
    /// its entry plan (`plans/<kind>.plan`): the compatibility id reads every
    /// entry plan beside the app's.
    #[test]
    fn an_even_odd_icon_in_a_widget_alone_moves_the_id() {
        let dir =
            std::env::temp_dir().join(format!("exact-icon-rules-widget-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.contract"), "shape Props\n  title: string\ncomponent App\n  view\n    text \"app\"\ncomponent Card\n  props\n    title: string\n  view\n    image \"symbol:icon||0 0 24 24|evenodd:M2 2H22V22H2Z M8 8H16V16H8Z|\"\n").unwrap();
        std::fs::write(dir.join("app.json"), r#"{"app":{"id":"test.widget","name":"Test"},"widgets":[{"kind":"card","component":"Card","props":"Props","families":["systemSmall"],"refresh":"refresh","preview":{"title":"Hello"}}]}"#).unwrap();
        let compiled = crate::colors::compiled(&dir).unwrap();
        let entries = contract::snapshot::entries(&dir).unwrap();
        let plans: Vec<&exact_plan::Plan> = entries.values().map(|e| &e.plan).collect();
        assert_eq!(plans.len(), 1);
        assert_eq!(
            super::needed(&dir, compiled.as_ref(), None, &[]).unwrap(),
            None,
            "the app's own plan draws none"
        );
        assert_eq!(
            super::needed(&dir, compiled.as_ref(), None, &plans).unwrap(),
            Some(vec!["evenodd"]),
            "its widget's does"
        );
        let (now, main) = (compat(&dir, false), compat(&dir, true));
        assert_eq!(now.inputs["iconFillRules"], serde_json::json!(["evenodd"]));
        assert_ne!(now.id, main.id);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// What the extra compile costs (theme2, C2 round 1), on the apps named in
    /// `EXACT_ICON_RULES_APPS` (`:`-separated app dirs): per app, warm, the
    /// median of 5 of the app's own compile (what its build script already
    /// does), the compatibility id with and without this input's path, and
    /// `needed` reading the plan compat already compiled. Run by hand with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn time_the_extra_compile() {
        let apps = std::env::var("EXACT_ICON_RULES_APPS").unwrap_or_default();
        for dir in apps
            .split(':')
            .filter(|d| !d.is_empty())
            .map(std::path::Path::new)
        {
            let median = |f: &mut dyn FnMut()| {
                f();
                let mut t: Vec<f64> = (0..5)
                    .map(|_| {
                        let at = std::time::Instant::now();
                        f();
                        at.elapsed().as_secs_f64() * 1000.0
                    })
                    .collect();
                t.sort_by(f64::total_cmp);
                t[2]
            };
            let contract = ["app.contract", "app/app.contract"]
                .iter()
                .map(|f| dir.join(f))
                .find(|p| p.is_file())
                .unwrap();
            let compiled = contract::compile_path(&contract)
                .map(|_| ())
                .map_err(|e| e.to_string());
            let own = median(&mut || drop(contract::compile_path(&contract)));
            let manifest = contract::Manifest::read(dir);
            let (with, without) = match &manifest {
                Ok(m) => {
                    let run = |main: bool| {
                        super::SKIP.with(|s| s.set(main));
                        let r = crate::compat::compatibility_id(
                            dir,
                            "macos",
                            "aarch64-apple-darwin",
                            m,
                            Some(""),
                        );
                        super::SKIP.with(|s| s.set(false));
                        r.map(|_| ())
                            .map_err(|e| e.chars().take(120).collect::<String>())
                    };
                    let outcome = run(false);
                    (
                        median(&mut || drop(run(false))),
                        format!("{:.1} ms ({:?})", median(&mut || drop(run(true))), outcome),
                    )
                }
                Err(e) => (f64::NAN, format!("no manifest: {e}")),
            };
            // Reading the fill rules from the plan compat already compiled.
            let plan = crate::colors::compiled(dir).ok().flatten();
            let needed = median(&mut || drop(super::needed(dir, plan.as_ref(), None, &[])));
            println!(
                "ICON-RULES {}: compiles {:?}; own compile {own:.1} ms; compat with {with:.1} ms, without {without}; reading the rules {:.3} ms",
                dir.display(),
                compiled.as_ref().map_or("refused", |_| "ok"),
                needed
            );
        }
    }
}
