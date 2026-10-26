//! What `app.json` adds to a compiled plan: the app's identity, its hatch
//! words, and the system's ways into the app (LLP 1133.023): the manifest's
//! `shortcuts` and the packages that publish shortcuts and content open a
//! URL at the navigation root, so an app with any of them and no
//! `navigate=` root is refused (`quick-actions-root`, `search-index-root`).

use crate::{native, CompileError, Manifest};
use contract_syntax::{File, Node, Span};
use exact_plan::Plan;
use std::path::Path;

/// Read `app.json` beside the app, where there is one, into `plan`.
pub(crate) fn from_manifest(
    path: &Path,
    app_root: &Path,
    file: &File,
    plan: &mut Plan,
) -> Result<(), Vec<CompileError>> {
    if !app_root.join("app.json").is_file() && !app_root.join(".shells/app.json").is_file() {
        return Ok(());
    }
    let refused = |id: &str, message: String| {
        vec![CompileError {
            pass: "app",
            id: id.into(),
            message,
            span: Span::default(),
            file: Some(path.into()),
            related: Box::new([]),
        }]
    };
    let manifest = Manifest::read(app_root).map_err(|m| refused("app-manifest", m))?;
    if !plan.app_id.is_empty() && plan.app_id != manifest.id {
        return Err(refused(
            "app-identity",
            format!(
                "the plan names app {}, but app.json names {}",
                plan.app_id, manifest.id
            ),
        ));
    }
    // @ref LLP 1075.003.000.001 §4.3 — each hatch word with the platforms
    // that handle it, under the plan's signature: a host calls a word only
    // where the plan says its platform handles it.
    let rows = native::hatch_rows(&manifest).map_err(|m| refused("app-manifest", m))?;
    for (word, platforms) in rows {
        plan.add_hatch(&word, platforms);
    }
    let shortcuts = (manifest.json.get("shortcuts").and_then(|s| s.as_array()))
        .filter(|s| !s.is_empty())
        .cloned();
    let uses = |extension: &str| (plan.modules.iter()).any(|m| plan.str(m.extension) == extension);
    let root = navigation_root(file);
    if !root && (shortcuts.is_some() || uses("quick-actions")) {
        return Err(refused(
            "quick-actions-root",
            "a quick action opens its url at the navigation root, and the root component's outermost node has no `navigate=` (LLP 1133.023 D1; docs/recipes/navigation-stack.md)".into(),
        ));
    }
    if !root && uses("search-index") {
        return Err(refused(
            "search-index-root",
            "a search result opens its entry's url at the navigation root, and the root component's outermost node has no `navigate=` (LLP 1133.023 D1; docs/recipes/navigation-stack.md)".into(),
        ));
    }
    // The agent's substitute shows the manifest's items before the app's
    // (LLP 1133.023 §10), as the platform does.
    if let Some(shortcuts) = shortcuts {
        for i in 0..plan.modules.len() {
            let row = &plan.modules[i];
            if plan.str(row.extension) != "quick-actions" {
                continue;
            }
            let mut sub: serde_json::Value =
                serde_json::from_str(plan.str(row.substitute)).unwrap_or_default();
            if sub["shortcuts"].is_null() {
                continue;
            }
            sub["manifest"] = serde_json::Value::Array(shortcuts.clone());
            plan.set_module_substitute(i, &sub.to_string());
        }
    }
    plan.app_id = manifest.id;
    Ok(())
}

/// Whether the root component's outermost node hears `navigate`.
fn navigation_root(file: &File) -> bool {
    let Some(root) = file.components.first() else {
        return false;
    };
    (root.view.iter()).any(|n| match n {
        Node::Element { attrs, .. } => attrs.iter().any(|a| a.name == "navigate"),
        _ => false,
    })
}
