//! The symbol tables symbols.js draws a bound source from (LLP 1011; LLP
//! 1121 D10): each entry `name: [path, filled]`, and an icon's
//! `[path, filled, viewBox, file]` (a path drawn, or a file the page shows).

use exact_plan::Plan;

/// One entry, as JavaScript.
fn entry(name: &str, path: &str, filled: bool, view_box: &str, file: &str) -> String {
    let j = |s: &str| serde_json::to_string(s).unwrap();
    match (view_box, file) {
        ("", "") => format!("{}:[{},{}]", j(name), j(path), filled as u8),
        _ => format!(
            "{}:[{},{},{},{}]",
            j(name),
            j(path),
            filled as u8,
            j(view_box),
            j(file)
        ),
    }
}

/// A role's entries: by its name, and (`sf`) by its Apple name as an SF
/// Symbol, which draws the role where no SF Symbol is.
fn role(name: &str, sf: bool) -> Vec<String> {
    let Some((apple, path, filled)) = exact_kernel::generated::symbol(name) else {
        return Vec::new();
    };
    let mut out = vec![entry(name, path, filled, "", "")];
    if sf && !apple.is_empty() {
        out.push(entry(&format!("sf/{apple}"), path, filled, "", ""));
    }
    out
}

/// The plan's table: its icon tokens, by name and by SF Symbol (those come
/// first, as an icon's `sf` does before a role's Apple name), and, when a
/// source is bound, each role the plan's strings name.
pub(crate) fn table(plan: &Plan, bound: bool) -> String {
    let s = |id: u32| plan.strings.get(id as usize).map_or("", String::as_str);
    let mut out: Vec<String> = Vec::new();
    for i in &plan.icons {
        let (path, file) = match s(i.path.0) {
            "" => ("", s(i.svg.0)),
            path => (path, ""),
        };
        out.push(entry(s(i.name.0), path, true, s(i.view_box.0), file));
        if !s(i.sf.0).is_empty() {
            out.push(entry(
                &format!("sf/{}", s(i.sf.0)),
                path,
                true,
                s(i.view_box.0),
                file,
            ));
        }
    }
    if bound {
        for text in &plan.strings {
            out.extend(role(text.strip_prefix("symbol:").unwrap_or(text), false));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|e| seen.insert(e.split(':').next().unwrap_or("").to_string()));
    format!("{{{}}}", out.join(","))
}

/// Every role, which symbols.js loads when a bound source names one the
/// plan's strings don't (`symbol-roles.js`).
pub(crate) fn every() -> String {
    let roles: Vec<String> = exact_kernel::generated::SYMBOL_ROLES
        .iter()
        .flat_map(|r| role(r, true))
        .collect();
    format!("export default {{{}}};", roles.join(","))
}
