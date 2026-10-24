//! An icon token's path for Apple (LLP 1121 D10; stage 9 A): an icon with no
//! `sf` source and a template SVG draws its path, as Linux, the reader and the
//! web do, from the plan's path data. Swift gets it as absolute `M`/`L`/`C`/`Z`
//! commands (`exact_kernel::svg::parse_d`, the parse Linux draws, quadratics
//! and arcs already cubics) beside its viewBox, so it builds a `CGPath` with
//! no SVG parser of its own. An `sf` wins: nothing is sent when one is given.

use std::fmt::Write;

/// The commands and viewBox of the icon `name` after `symbol:` draws, when
/// Apple has no SF Symbol for it and it has a template path; else `None`.
pub(crate) fn commands(name: &str) -> Option<(String, &str)> {
    if !exact_kernel::apple_symbol_name(name).is_empty() {
        return None;
    }
    let path = exact_kernel::symbol_path(name).filter(|p| p.filled && !p.even_odd)?;
    let mut out = String::new();
    // Each of the SVG's paths, `;` between them: each is filled nonzero on its
    // own, as Linux and the web fill them (one winding over all would cut an
    // opposite path's overlap out).
    for (i, d) in path.d.split(';').enumerate() {
        if i > 0 && !out.is_empty() {
            out.push_str("; ");
        }
        for seg in exact_kernel::svg::parse_d(d).0 {
            use exact_kernel::svg::Seg::*;
            let _ = match seg {
                Move(x, y) => write!(out, "M {} {} ", n(x), n(y)),
                Line(x, y) => write!(out, "L {} {} ", n(x), n(y)),
                Cubic(a, b, c, d, x, y) => write!(
                    out,
                    "C {} {} {} {} {} {} ",
                    n(a),
                    n(b),
                    n(c),
                    n(d),
                    n(x),
                    n(y)
                ),
                Close => write!(out, "Z "),
            };
        }
    }
    let out = out.trim_end().trim_end_matches(';').trim_end().to_string();
    (!out.is_empty()).then_some((out, path.view_box))
}

/// `,"symbolPath":…,"symbolViewBox":…` after a control face's symbol in
/// its JSON (a button face, a grouped row) when `source` (after `symbol:`)
/// is an icon Apple draws by its path (stage 9 A2): the face makes a
/// template image of it its control tints as a symbol. Nothing otherwise.
pub(crate) fn push_json(source: Option<&str>, json: &mut String) {
    let Some((d, view_box)) = source.and_then(commands) else {
        return;
    };
    let quote = exact_runner::agent::quote;
    json.push_str(",\"symbolPath\":");
    quote(&d, json);
    json.push_str(",\"symbolViewBox\":");
    quote(view_box, json);
}

/// The icon token's name as its author wrote it (`icon.reply`) for the
/// source form `source` (after `symbol:`), from the plan's icons table,
/// which keeps every icon a literal or a computed source names. The form
/// carries no name (the compiler and the runner's bridge expand the name
/// away), so two tokens with the same source (`icon.reply` and
/// `icon.respond` on one SVG) are told apart by nothing the host gets: the
/// first declared is the best known. Keeping the chosen name would move the
/// source form or the plan (theme2's to rule). `None` for no icon or no row.
pub(crate) fn icon_name<'a>(plan: &'a exact_plan::Plan, source: &str) -> Option<&'a str> {
    let icon = exact_kernel::Icon::parse(source)?;
    let s = |id| plan.strings.get(id as usize).unwrap_or("");
    plan.icons
        .iter()
        .find(|i| exact_runner::icon_of(plan, i) == Some(icon))
        .map(|i| s(i.name.0))
}

/// A coordinate, the shortest text that reads back as the same `f32` (no
/// fixed precision: a viewBox may be any size).
fn n(v: f32) -> String {
    if v == 0.0 {
        "0".into()
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::{commands, icon_name, push_json};

    #[test]
    fn an_icons_name_is_the_plans_row_for_its_source_form() {
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        b.icon(
            "icon.reply",
            "",
            "0 0 24 24",
            "M2 2 L22 2 Z",
            "icons/reply.svg",
        );
        b.icon(
            "icon.heart",
            "heart",
            "0 0 24 24",
            "M2 2 L22 2 Z",
            "icons/reply.svg",
        );
        let plan = b.finish().unwrap();
        assert_eq!(
            icon_name(&plan, "icon||0 0 24 24|M2 2 L22 2 Z|icons/reply.svg"),
            Some("icon.reply")
        );
        assert_eq!(
            icon_name(&plan, "icon|heart|0 0 24 24|M2 2 L22 2 Z|icons/reply.svg"),
            Some("icon.heart")
        );
        assert_eq!(icon_name(&plan, "icon||0 0 24 24|M0 0 Z|icons/x.svg"), None);
        // Two tokens on one SVG have one source form: the first declared is the best known.
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        b.icon(
            "icon.reply",
            "",
            "0 0 24 24",
            "M2 2 L22 2 Z",
            "icons/reply.svg",
        );
        b.icon(
            "icon.respond",
            "",
            "0 0 24 24",
            "M2 2 L22 2 Z",
            "icons/reply.svg",
        );
        let both = b.finish().unwrap();
        assert_eq!(
            icon_name(&both, "icon||0 0 24 24|M2 2 L22 2 Z|icons/reply.svg"),
            Some("icon.reply")
        );
        assert_eq!(icon_name(&plan, "star"), None);
    }

    #[test]
    fn a_faces_json_carries_an_svg_only_icons_path_and_no_other() {
        let mut json = String::new();
        push_json(
            Some("icon||0 0 24 24|M2 2 L22 2 L12 20 Z|icons/tri.svg"),
            &mut json,
        );
        assert_eq!(
            json,
            r#","symbolPath":"M 2 2 L 22 2 L 12 20 Z","symbolViewBox":"0 0 24 24""#
        );
        for source in [
            Some("icon|heart|0 0 24 24|M2 2 L22 2 L12 20 Z|icons/tri.svg"),
            Some("sf/lock"),
            Some("star"),
            None,
        ] {
            let mut json = String::new();
            push_json(source, &mut json);
            assert_eq!(json, "", "{source:?}");
        }
    }

    #[test]
    fn an_svg_only_icon_sends_its_path_and_an_sf_icon_none() {
        // A quadratic and an arc arrive as cubics; two paths as one.
        let (d, vb) =
            commands("icon||0 0 24 24|M2 2 Q12 0 22 2 Z;M4 12 A8 8 0 0 0 20 12 Z|icons/x.svg")
                .unwrap();
        assert_eq!(vb, "0 0 24 24");
        assert!(d.starts_with("M 2 2 C "), "{d}");
        assert_eq!(d.matches('M').count(), 2, "{d}");
        assert_eq!(d.matches(';').count(), 1, "each path its own: {d}");
        // A tiny viewBox keeps its coordinates.
        let (d, _) = commands(
            "icon||0 0 0.0004 0.0004|M0.0001 0.0001 L0.0003 0.0001 L0.0003 0.0003 Z|icons/x.svg",
        )
        .unwrap();
        assert_eq!(d, "M 0.0001 0.0001 L 0.0003 0.0001 L 0.0003 0.0003 Z");
        assert!(!d.contains('Q') && !d.contains('A'), "{d}");
        // An `sf` source wins; a file that is no template has no path; a role is Apple's symbol.
        assert_eq!(
            commands("icon|heart|0 0 24 24|M2 2 L4 4 Z|icons/x.svg"),
            None
        );
        assert_eq!(commands("icon||0 0 24 24||icons/two.svg"), None);
        assert_eq!(commands("star"), None);
    }
}
