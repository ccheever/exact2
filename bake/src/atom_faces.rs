//! @ref LLP 1045.000 D2 — `atomFace` in the compatibility id: the runner's
//! bridge reads an `AtomFace` record by its fields
//! (`exact_runner::decorations::ATOM_FACE_FIELDS`) and clears a Markdown
//! node's faces when the plan's record has others, so an app whose plan
//! binds `atomFaces` names those fields, and its id moves when they do; an
//! app that binds none has no such input and keeps its id, as
//! `icon_rules`' even-odd icons do. The other records an app builds
//! (`TextDecorations`, `MediaMetadata`) are read by position too, and take
//! an input of their own this way when their fields change.

use std::path::Path;

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
/// ships (the app's, read as `icon_rules` reads it, or an entry's) binds
/// `atomFaces`; nothing otherwise.
pub(crate) fn apply(
    compat: &mut crate::compat::Compat,
    app_dir: &Path,
    compiled: Option<&exact_plan::Plan>,
    entries: &std::collections::BTreeMap<String, contract::snapshot::Entry>,
) -> Result<(), String> {
    #[cfg(test)]
    if SKIP.with(std::cell::Cell::get) {
        return Ok(());
    }
    let out = std::env::var_os("OUT_DIR").map(std::path::PathBuf::from);
    let other = crate::icon_rules::shipped(app_dir, compiled, out.as_deref())?;
    let main = other.as_ref().or(compiled);
    if main
        .into_iter()
        .chain(entries.values().map(|e| &e.plan))
        .any(binds)
    {
        compat.inputs["atomFace"] =
            serde_json::json!(exact_runner::decorations::ATOM_FACE_FIELDS);
        compat.id = crate::compat::compatibility_digest(&compat.inputs);
    }
    Ok(())
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

    /// Every `.contract` source under `dir`.
    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if name != "node_modules" && name != "target" && name != ".exact" {
                    sources(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "contract") {
                out.push(path);
            }
        }
    }

    /// Markdown Stress is the one app in this repository whose id moves: a
    /// plan binds `atomFaces` only where a source it compiles writes it, the
    /// app's own or a package's, and of every app's and package's sources
    /// only Markdown Stress's do; its plan binds them, and its id moves.
    #[test]
    fn markdown_stress_is_the_one_repo_app_whose_id_moves() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut paths = Vec::new();
        for dir in ["apps", "packages"] {
            sources(&root.join(dir), &mut paths);
        }
        assert!(paths.len() > 100, "no sources found under {}", root.display());
        // The app or package a source is in: `apps/<name>`, `packages/<name>`.
        let owner = |p: &Path| {
            let parts: Vec<_> = p.strip_prefix(&root).unwrap().iter().take(2).collect();
            parts.join(std::ffi::OsStr::new("/")).to_string_lossy().into_owned()
        };
        let mut writers: Vec<String> = paths
            .iter()
            .filter(|p| std::fs::read_to_string(p).is_ok_and(|s| s.contains("atomFaces")))
            .map(|p| owner(p))
            .collect();
        writers.sort();
        writers.dedup();
        assert_eq!(writers, ["apps/markdown-stress"]);
        let stress = root.join("apps/markdown-stress");
        let plan = crate::colors::compiled(&stress).unwrap().expect("its plan");
        assert!(super::binds(&plan));
        let (now, main) = (compat(&stress, false), compat(&stress, true));
        assert!(now.inputs.get("atomFace").is_some());
        assert_ne!(now.id, main.id);
    }
}
