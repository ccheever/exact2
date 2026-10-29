//! An icon token's path for Apple (LLP 1121 D10; stage 9 A): an icon with no
//! `sf` source and a template SVG draws its path, as Linux, the reader and the
//! web do, from the plan's path data. Swift gets it as absolute `M`/`L`/`C`/`Z`
//! commands (`exact_kernel::svg::parse_d`, the parse Linux draws, quadratics
//! and arcs already cubics) beside its viewBox, so it builds a `CGPath` with
//! no SVG parser of its own. An `sf` wins: nothing is sent when one is given.
//! An even-odd icon (stage 14 C2: its path's `evenodd:` marker, which only
//! `exact_kernel::Icon` reads) says so beside its path: `symbolFillRule`.

use std::fmt::Write;

/// The commands and viewBox of the icon `name` after `symbol:` draws, when
/// Apple has no SF Symbol for it and it has a template path, and whether it
/// fills even-odd; else `None`. A role always has an SF Symbol, so a role's
/// even-odd silhouette never comes here.
pub(crate) fn commands(name: &str) -> Option<(String, &str, bool)> {
    if !exact_kernel::apple_symbol_name(name).is_empty() {
        return None;
    }
    let path = exact_kernel::symbol_path(name).filter(|p| p.filled)?;
    let mut out = String::new();
    // Each of the SVG's paths, `;` between them: each is filled on its own by
    // the icon's rule, as Linux and the web fill them (one winding over all
    // would cut an opposite path's overlap out).
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
    (!out.is_empty()).then_some((out, path.view_box, path.even_odd))
}

/// `,"symbolPath":…,"symbolViewBox":…` after a control face's symbol in
/// its JSON (a button face, a grouped row) when `source` (after `symbol:`)
/// is an icon Apple draws by its path (stage 9 A2): the face makes a
/// template image of it its control tints as a symbol. Nothing otherwise.
pub(crate) fn push_json(source: Option<&str>, json: &mut String) {
    let Some((d, view_box, even_odd)) = source.and_then(commands) else {
        return;
    };
    let quote = exact_runner::agent::quote;
    json.push_str(",\"symbolPath\":");
    quote(&d, json);
    json.push_str(",\"symbolViewBox\":");
    quote(view_box, json);
    if even_odd {
        json.push_str(",\"symbolFillRule\":\"evenodd\"");
    }
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
        let (d, vb, _) =
            commands("icon||0 0 24 24|M2 2 Q12 0 22 2 Z;M4 12 A8 8 0 0 0 20 12 Z|icons/x.svg")
                .unwrap();
        assert_eq!(vb, "0 0 24 24");
        assert!(d.starts_with("M 2 2 C "), "{d}");
        assert_eq!(d.matches('M').count(), 2, "{d}");
        assert_eq!(d.matches(';').count(), 1, "each path its own: {d}");
        // A tiny viewBox keeps its coordinates.
        let (d, _, _) = commands(
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

    /// Stage 14 C2: an even-odd icon (its path's `evenodd:` marker, read by
    /// `exact_kernel::Icon`) sends its path, the marker gone, and says
    /// `symbolFillRule`; a nonzero icon sends what it always did.
    #[test]
    fn an_even_odd_icon_says_its_rule_and_a_nonzero_one_sends_what_it_did() {
        let even = "icon||0 0 24 24|evenodd:M2 2 L22 2 L22 22 L2 22 Z M8 8 L16 8 L16 16 L8 16 Z|icons/frame.svg";
        let (d, vb, even_odd) = commands(even).unwrap();
        assert!(even_odd);
        assert_eq!(vb, "0 0 24 24");
        assert!(
            d.starts_with("M 2 2 L 22 2"),
            "the marker is not a command: {d}"
        );
        let mut json = String::new();
        push_json(Some(even), &mut json);
        assert_eq!(
            json,
            r#","symbolPath":"M 2 2 L 22 2 L 22 22 L 2 22 Z M 8 8 L 16 8 L 16 16 L 8 16 Z","symbolViewBox":"0 0 24 24","symbolFillRule":"evenodd""#
        );
        let mut plain = String::new();
        push_json(
            Some("icon||0 0 24 24|M2 2 L22 2 L12 20 Z|icons/tri.svg"),
            &mut plain,
        );
        assert_eq!(
            plain, r#","symbolPath":"M 2 2 L 22 2 L 12 20 Z","symbolViewBox":"0 0 24 24""#,
            "byte-identical to before C2"
        );
        assert_eq!(
            commands("icon|sf.square|0 0 24 24|evenodd:M2 2 L4 4 Z|icons/x.svg"),
            None,
            "an `sf` still wins"
        );
    }

    /// The plan row of an even-odd icon is read through the one reader too,
    /// so its name is found (it was missed while its raw path kept the marker).
    #[test]
    fn an_even_odd_icons_name_is_its_row() {
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        b.icon(
            "icon.frame",
            "",
            "0 0 24 24",
            "evenodd:M2 2 L22 2 Z",
            "icons/frame.svg",
        );
        b.icon(
            "icon.block",
            "",
            "0 0 24 24",
            "M2 2 L22 2 Z",
            "icons/frame.svg",
        );
        let plan = b.finish().unwrap();
        assert_eq!(
            icon_name(
                &plan,
                "icon||0 0 24 24|evenodd:M2 2 L22 2 Z|icons/frame.svg"
            ),
            Some("icon.frame")
        );
        assert_eq!(
            icon_name(&plan, "icon||0 0 24 24|M2 2 L22 2 Z|icons/frame.svg"),
            Some("icon.block"),
            "the same d nonzero is the other row"
        );
    }
}
