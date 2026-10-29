//! @ref LLP 1045.000 D2 — `atomFace` in the compatibility id: the runner's
//! bridge reads an `AtomFace` record by its fields
//! (`exact_runner::decorations::ATOM_FACE_FIELDS`) and clears a Markdown
//! node's faces when the plan's record has others, so an app whose plan
//! binds `atomFaces` names those fields, and its id moves when they do; an
//! app that binds none has no such input and keeps its id, as
//! `icon_rules`' even-odd icons do. The other records an app builds
//! (`TextDecorations`, `MediaMetadata`) are read by position too, and take
//! an input of their own this way when their fields change.

#[cfg(test)]
thread_local! {
    /// Tests only: compute the compatibility id without this input, as main
    /// did, to compare against.
    pub(crate) static SKIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether `plan` binds a Markdown node's `atomFaces`.
pub(crate) fn binds(plan: &exact_plan::Plan) -> bool {
    plan.bindings.iter().any(|b| {
        b.kind == exact_plan::BindingKind::Prop && b.id == exact_kernel::PropId::AtomFaces as u16
    })
}

/// `atomFace` into `compat`'s inputs, and its id again, when a plan that
/// ships (`main`, the app's, or an entry's) binds `atomFaces`; nothing
/// otherwise.
pub(crate) fn apply(
    compat: &mut crate::compat::Compat,
    main: Option<&exact_plan::Plan>,
    entries: &std::collections::BTreeMap<String, contract::snapshot::Entry>,
) {
    #[cfg(test)]
    if SKIP.with(std::cell::Cell::get) {
        return;
    }
    if main
        .into_iter()
        .chain(entries.values().map(|e| &e.plan))
        .any(binds)
    {
        compat.inputs["atomFace"] = serde_json::json!(exact_runner::decorations::ATOM_FACE_FIELDS);
        compat.id = crate::compat::compatibility_digest(&compat.inputs);
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn app(name: &str, view: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("exact-atom-faces-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("app.json"),
            r#"{"app":{"id":"dev.test.faces","name":"Faces"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("app.contract"),
            format!("atom mention prefix=\"@\"\ncomponent App\n  derive faces = [AtomFace(source=\"@a\", label=\"\", icon=\"\", title=\"\", color=\"\", backgroundColor=\"\")]\n  view\n{view}"),
        )
        .unwrap();
        dir
    }

    /// The app's compatibility id, through this input's code path or
    /// (`main`) without it, as main computes it.
    fn compat(dir: &Path, main: bool) -> crate::compat::Compat {
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

    /// A plan that binds `atomFaces` names `AtomFace`'s fields and gets
    /// another id: main's inputs with that key alone added. One that binds
    /// none keeps main's id and inputs exactly.
    #[test]
    fn the_id_moves_exactly_when_a_plan_binds_atom_faces() {
        let bound = app(
            "bound",
            "    textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces\n",
        );
        let (now, main) = (compat(&bound, false), compat(&bound, true));
        assert_eq!(
            now.inputs["atomFace"],
            serde_json::json!(exact_runner::decorations::ATOM_FACE_FIELDS)
        );
        assert_ne!(now.id, main.id);
        let mut without = now.inputs.clone();
        without.as_object_mut().unwrap().remove("atomFace");
        assert_eq!(without, main.inputs, "nothing else differs");
        assert_eq!(crate::compat::compatibility_digest(&without), main.id);
        // The same app with atoms and no faces bound: main's id.
        let unbound = app(
            "unbound",
            "    textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\"\n",
        );
        let (now, main) = (compat(&unbound, false), compat(&unbound, true));
        assert_eq!(now.id, main.id);
        assert_eq!(now.inputs, main.inputs);
        assert!(now.inputs.get("atomFace").is_none(), "{}", now.inputs);
        for dir in [bound, unbound] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
