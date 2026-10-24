//! The symbol tables symbols.js draws a bound source from (LLP 1011; LLP
//! 1121 D10): each entry `name: [path, filled]`, and an icon's
//! `[path, filled, viewBox, file]` (a path drawn, or a file the page shows),
//! with a fifth `1` for an icon filled even-odd (stage 14 C2).

use exact_plan::Plan;

/// One entry, as JavaScript; an even-odd icon's carries a fifth `1`.
fn entry(
    name: &str,
    path: &str,
    filled: bool,
    view_box: &str,
    file: &str,
    even_odd: bool,
) -> String {
    let j = |s: &str| serde_json::to_string(s).unwrap();
    match (view_box, file) {
        ("", "") => format!("{}:[{},{}]", j(name), j(path), filled as u8),
        _ => format!(
            "{}:[{},{},{},{}{}]",
            j(name),
            j(path),
            filled as u8,
            j(view_box),
            j(file),
            if even_odd { ",1" } else { "" }
        ),
    }
}

/// A role's entries: by its name, and (`sf`) by its Apple name as an SF
/// Symbol, which draws the role where no SF Symbol is.
fn role(name: &str, sf: bool) -> Vec<String> {
    let Some((apple, path, filled)) = exact_kernel::generated::symbol(name) else {
        return Vec::new();
    };
    let mut out = vec![entry(name, path, filled, "", "", false)];
    if sf && !apple.is_empty() {
        out.push(entry(&format!("sf/{apple}"), path, filled, "", "", false));
    }
    out
}

/// The plan's table: its icon tokens, by name and by SF Symbol (those come
/// first, as an icon's `sf` does before a role's Apple name), and, when a
/// source is bound, each role the plan's strings name.
pub(crate) fn table(plan: &Plan, bound: bool) -> String {
    let s = |id: u32| plan.strings.get(id as usize).unwrap_or("");
    let mut out: Vec<String> = Vec::new();
    for i in &plan.icons {
        // The row through its one reader, which strips an even-odd icon's
        // marker into its rule (stage 14 C2); a row it can't read is no entry.
        let Some(icon) = exact_runner::icon_of(plan, i) else {
            continue;
        };
        let (path, file) = match icon.path {
            "" => ("", icon.svg),
            path => (path, ""),
        };
        out.push(entry(
            s(i.name.0),
            path,
            true,
            icon.view_box,
            file,
            icon.even_odd,
        ));
        if !icon.sf.is_empty() {
            out.push(entry(
                &format!("sf/{}", icon.sf),
                path,
                true,
                icon.view_box,
                file,
                icon.even_odd,
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

#[cfg(test)]
mod tests {
    use super::table;

    /// Stage 14 C2: an even-odd icon's row is read through
    /// `exact_kernel::Icon`: its entry's path without the marker and a fifth
    /// `1`; a nonzero icon's entry is as before; a row with a rule no reader
    /// knows is no entry.
    #[test]
    fn an_even_odd_icons_entry_carries_its_rule() {
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        b.icon(
            "icon.ring",
            "ring",
            "0 0 24 24",
            "evenodd:M2 2H22V22H2Z M8 8H16V16H8Z",
            "icons/ring.svg",
        );
        b.icon("icon.sq", "", "0 0 24 24", "M2 2H22V22H2Z", "icons/sq.svg");
        b.icon(
            "icon.odd",
            "",
            "0 0 24 24",
            "winding:M0 0h1Z",
            "icons/odd.svg",
        );
        let t = table(&b.finish().unwrap(), false);
        assert!(
            t.contains(r#""icon.ring":["M2 2H22V22H2Z M8 8H16V16H8Z",1,"0 0 24 24","",1]"#),
            "{t}"
        );
        assert!(
            t.contains(r#""sf/ring":["M2 2H22V22H2Z M8 8H16V16H8Z",1,"0 0 24 24","",1]"#),
            "{t}"
        );
        assert!(
            t.contains(r#""icon.sq":["M2 2H22V22H2Z",1,"0 0 24 24",""]"#),
            "{t}"
        );
        assert!(!t.contains("icon.odd") && !t.contains("evenodd:"), "{t}");
    }
}
