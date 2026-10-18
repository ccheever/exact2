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
    // Each of the SVG's paths, filled nonzero; together, one path's union.
    for d in path.d.split(';') {
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
    let out = out.trim_end().to_string();
    (!out.is_empty()).then_some((out, path.view_box))
}

/// A coordinate, to thousandths, without a trailing zero.
fn n(v: f32) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

#[cfg(test)]
mod tests {
    use super::commands;

    #[test]
    fn an_svg_only_icon_sends_its_path_and_an_sf_icon_none() {
        // A quadratic and an arc arrive as cubics; two paths as one.
        let (d, vb) =
            commands("icon||0 0 24 24|M2 2 Q12 0 22 2 Z;M4 12 A8 8 0 0 0 20 12 Z|icons/x.svg")
                .unwrap();
        assert_eq!(vb, "0 0 24 24");
        assert!(d.starts_with("M 2 2 C "), "{d}");
        assert_eq!(d.matches('M').count(), 2, "{d}");
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
