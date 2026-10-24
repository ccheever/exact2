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
/// nonzero: `["evenodd"]` when it keeps an even-odd icon, else none.
/// `compiled`: the app's `app.contract` plan the compatibility id already
/// compiled (`colors::compiled`), read again, not compiled again (a second
/// compile cost 84-133% of the app's own, round 1). Only an app with no
/// `app.contract` (a game's `app/app.contract`) is compiled here; one that
/// doesn't compile is the build's own refusal to make, and needs none.
pub(crate) fn needed(
    app_dir: &Path,
    compiled: Option<&exact_plan::Plan>,
) -> Option<Vec<&'static str>> {
    #[cfg(test)]
    if SKIP.with(std::cell::Cell::get) {
        return None;
    }
    let other;
    let plan = match compiled {
        Some(plan) => plan,
        None => {
            let contract = app_dir.join("app/app.contract");
            other = contract::compile_path(&contract).ok()?;
            &other
        }
    };
    even_odd(plan).then(|| vec!["evenodd"])
}

/// Whether `plan` keeps an icon filled even-odd, each row read through
/// `exact_kernel::Icon`, the one reader of the table.
pub(crate) fn even_odd(plan: &exact_plan::Plan) -> bool {
    plan.icons.iter().any(|i| {
        let s = |id| plan.str(id);
        exact_kernel::Icon::of_row(s(i.sf), s(i.view_box), s(i.path), s(i.svg))
            .is_some_and(|icon| icon.even_odd)
    })
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
        assert!(even_odd(&plan("evenodd:M0 0h4v4H0Z")));
        assert!(!even_odd(&plan("M0 0h4v4H0Z")));
        assert!(!even_odd(&plan("")));
        assert!(!even_odd(
            &exact_plan::builder::PlanBuilder::new(0, 0)
                .finish()
                .unwrap()
        ));
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
            super::needed(&plain, crate::colors::compiled(&plain).unwrap().as_ref()),
            None
        );
        let (now, main) = (compat(&plain, false), compat(&plain, true));
        assert_eq!(now.id, main.id);
        assert_eq!(now.inputs, main.inputs);
        assert!(now.inputs.get("iconFillRules").is_none(), "{}", now.inputs);
        let _ = std::fs::remove_dir_all(plain);
    }

    /// And a plan that keeps an even-odd icon gets `iconFillRules:
    /// ["evenodd"]` and another id: main's inputs with that key alone added.
    #[test]
    fn a_plan_with_an_even_odd_icon_gets_the_rule_and_another_id() {
        let odd = app("odd", "evenodd");
        assert_eq!(
            super::needed(&odd, crate::colors::compiled(&odd).unwrap().as_ref()),
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
            let needed = median(&mut || drop(super::needed(dir, plan.as_ref())));
            println!(
                "ICON-RULES {}: compiles {:?}; own compile {own:.1} ms; compat with {with:.1} ms, without {without}; reading the rules {:.3} ms",
                dir.display(),
                compiled.as_ref().map_or("refused", |_| "ok"),
                needed
            );
        }
    }
}
