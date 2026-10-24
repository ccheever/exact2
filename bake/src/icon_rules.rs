//! @ref LLP 1121 stage 14 C2 — `iconFillRules` in the compatibility id: an
//! app whose plan keeps an even-odd icon (its `icons` row's path starts
//! `evenodd:`) needs a binary that reads the marker, so the input is
//! `["evenodd"]` and the id moves; an app with none has no such input, so
//! its id (and every app's today) is as it was. An absent key, not a null
//! one: a key in the inputs, even null, moves every id.

use std::path::Path;

/// The fill rules the app at `app_dir`'s plan needs a binary to read beyond
/// nonzero: `["evenodd"]` when it keeps an even-odd icon, else none. The
/// plan is compiled from the app's contract (`app.contract`, or a game's
/// `app/app.contract`); one that doesn't compile here is the build's own
/// refusal to make, and needs none here.
pub(crate) fn needed(app_dir: &Path) -> Option<Vec<&'static str>> {
    let contract = ["app.contract", "app/app.contract"]
        .iter()
        .map(|f| app_dir.join(f))
        .find(|p| p.is_file())?;
    let plan = contract::compile_path(&contract).ok()?;
    even_odd(&plan).then(|| vec!["evenodd"])
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

    /// An app that keeps an even-odd icon has `iconFillRules` in its
    /// compatibility inputs, so its id moves; the same app with the icon
    /// nonzero has no such input, as every app before.
    #[test]
    fn only_an_app_with_an_even_odd_icon_moves_its_compatibility_id() {
        let app = |name: &str, rule: &str| {
            let dir = std::env::temp_dir()
                .join(format!("exact-icon-rules-{name}-{}", std::process::id()));
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
        };
        let compat = |dir: &std::path::Path| {
            let manifest = contract::Manifest::read(dir).unwrap();
            crate::compat::compatibility_id(
                dir,
                "macos",
                "aarch64-apple-darwin",
                &manifest,
                Some(""),
            )
            .unwrap()
        };
        let (odd, plain) = (app("odd", "evenodd"), app("plain", "nonzero"));
        assert_eq!(super::needed(&odd), Some(vec!["evenodd"]));
        assert_eq!(super::needed(&plain), None);
        let (a, b) = (compat(&odd), compat(&plain));
        assert_eq!(a.inputs["iconFillRules"], serde_json::json!(["evenodd"]));
        assert!(b.inputs.get("iconFillRules").is_none(), "{}", b.inputs);
        assert_ne!(a.id, b.id);
        let mut without = a.inputs.clone();
        without.as_object_mut().unwrap().remove("iconFillRules");
        assert_eq!(
            crate::compat::compatibility_digest(&without),
            b.id,
            "nothing else differs"
        );
        for dir in [odd, plain] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
