//! Declared pairs (LLP 1121 D3, `pair=`): a token says what it is drawn
//! on, a colour token or a role, never itself (`tamber-pair`); a pair below
//! WCAG's 4.5:1 in some tuple and scheme is warned of, once, at its lowest
//! (`tamber-contrast`), a residual side read at its fallback; the warning
//! never refuses the design.

use crate::common::vocab;
use exact_tamber::check;

const AXES: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n";

#[test]
fn a_pair_names_colour_tokens_and_roles_in_the_order_written() {
    let folding = check(
        &format!("{AXES}  surface.page = light-dark(#ffffff, #000000)\n  surface.card = light-dark(#f1f3f5, #16181c)\n  text.primary = light-dark(#0f1419, #e7e9ea)  pair=surface.page,surface.card,Canvas\n"),
        &vocab(),
    )
    .unwrap();
    let text = folding
        .tokens
        .iter()
        .find(|t| t.name == "text.primary")
        .unwrap();
    assert_eq!(text.pairs, ["surface.page", "surface.card", "Canvas"]);
    assert_eq!(folding.warnings, Vec::new());
}

#[test]
fn a_pair_on_itself_or_on_nothing_is_refused() {
    let all = check(
        &format!("{AXES}  surface.page = #ffffff\n  text.primary = #000000  pair=text.primary\n  text.muted = #777777  pair=surface.nowhere\n"),
        &vocab(),
    )
    .unwrap_err();
    let ids: Vec<(&str, u32)> = all.iter().map(|d| (d.id, d.line)).collect();
    assert_eq!(ids, [("tamber-pair", 8), ("tamber-pair", 9)], "{all:?}");
    assert!(
        all[0].message.contains("drawn on itself"),
        "{}",
        all[0].message
    );
    assert!(
        all[1]
            .message
            .contains("`pair=surface.nowhere` names no colour token or role"),
        "{}",
        all[1].message
    );
}

#[test]
fn a_pair_below_the_text_ratio_is_warned_of_at_its_lowest_tuple_and_scheme() {
    let folding = check(
        &format!("{AXES}  surface.page = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #15202b)\n  text.muted = light-dark(#595959, #3a3a3a)  pair=surface.page\n"),
        &vocab(),
    )
    .unwrap();
    assert_eq!(folding.warnings.len(), 1, "{:?}", folding.warnings);
    let w = &folding.warnings[0];
    assert_eq!((w.id, w.line), ("tamber-contrast", 9));
    // Dark on the dim ground is the lowest: #3a3a3a on #15202b.
    assert!(
        w.message
            .starts_with("`text.muted` on `surface.page` is 1.45:1"),
        "{}",
        w.message
    );
    assert!(
        w.message.contains("in dark-style=dim, dark: below 4.5:1"),
        "{}",
        w.message
    );
}

#[test]
fn a_role_side_is_read_at_its_fallback_and_the_warning_says_so() {
    let folding = check(
        &format!("{AXES}  ink.faint = #eeeeee  pair=Canvas\n"),
        &vocab(),
    )
    .unwrap();
    assert_eq!(folding.warnings.len(), 1, "{:?}", folding.warnings);
    assert!(
        folding.warnings[0].message.contains("read at its fallback"),
        "{}",
        folding.warnings[0].message
    );
}
