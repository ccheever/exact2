//! A node's host-known state looks on the page (LLP 1121 D8, stage 8 B):
//! `:hover`, `:focus-visible` and `:active` (the press glue's
//! `data-pressed`, never a stylesheet `:active`), as one scoped rule per
//! state in the page's looks sheet, composed in that order over the node's
//! inline base so rows a look doesn't name show the base. A look's
//! `-exact-press-scale` is the node's press scale. `data-exact-look` names
//! the states, so the unsaid press (index.html's 65%) stands aside for an
//! `:active` look and the press glue tracks the node.

use super::*;
use exact_kernel::{NodeRef, StyleId, StyleProps, LOOK_ACTIVE, LOOK_FOCUS_VISIBLE, LOOK_HOVER};

/// The press curve and its duration (input-glue.js `showPress`).
const EASE: &str = "120ms cubic-bezier(.16,1,.3,1)";

/// [`props_for`], with `data-exact-look` naming the node's looks.
pub(in crate::host) fn props(node: &NodeRef<'_>) -> SortedMap<String, String> {
    let mut props = props_for(node);
    let names: Vec<&str> = node.looks().iter().map(|(state, _)| name(*state)).collect();
    if !names.is_empty() {
        props.insert("data-exact-look".into(), names.join(" "));
    }
    props
}

/// [`css_style`] with [`with_press`].
pub(in crate::host) fn css_style<'a>(
    kernel: &Kernel,
    node: &NodeRef<'a>,
) -> std::borrow::Cow<'a, StyleProps> {
    with_press(element::css_style(kernel, node), node)
}

/// `style` with an `:active` look's press scale as the node's own: a press
/// scale shows only while pressed, as the look does.
pub(in crate::host) fn with_press<'a>(
    mut style: std::borrow::Cow<'a, StyleProps>,
    node: &NodeRef<'_>,
) -> std::borrow::Cow<'a, StyleProps> {
    let active = node.looks().iter().find(|(state, _)| *state == LOOK_ACTIVE);
    if let Some((_, look)) = active.filter(|(_, l)| l.mask.has(StyleId::PressScale)) {
        let style = style.to_mut();
        style.press_scale = look.press_scale;
        style.mask.set(StyleId::PressScale);
    }
    style
}

fn name(state: u8) -> &'static str {
    match state {
        LOOK_ACTIVE => "active",
        LOOK_HOVER => "hover",
        _ => "focus-visible",
    }
}

/// Each look's declarations, its press scale left out (the node's own,
/// [`with_press`]); a look with none is left out. `css` writes them
/// ([`crate::css::css_text`], or the JS target's relative form).
pub fn declarations(
    looks: &[(u8, StyleProps)],
    css: impl Fn(&StyleProps) -> String,
) -> Vec<(u8, String)> {
    let mut scale = exact_kernel::StyleMask::EMPTY;
    scale.set(StyleId::PressScale);
    looks
        .iter()
        .map(|(state, look)| {
            let mut look = look.clone();
            look.clear(scale);
            (*state, css(&look))
        })
        .filter(|(_, css)| !css.is_empty())
        .collect()
}

/// The looks' [`declarations`] for the element `at` selects, as page rules,
/// empty for none: `:hover` (where the pointer hovers), `:focus-visible`,
/// then `[data-pressed]`, the last winning; a disabled node takes none.
/// While the press glue eases (`data-exact-easing`), the press look's rows
/// transition on the press curve, both ways.
pub fn rules(at: &str, looks: &[(u8, String)]) -> String {
    let mut out = String::new();
    let enabled = ":not(:disabled,[disabled=\"true\"])";
    let rank = |state: u8| {
        [LOOK_HOVER, LOOK_FOCUS_VISIBLE, LOOK_ACTIVE]
            .iter()
            .position(|s| *s == state)
    };
    let mut looks: Vec<_> = looks.iter().collect();
    looks.sort_by_key(|(state, _)| rank(*state));
    for (state, css) in looks {
        let rows: String = css
            .split_terminator(';')
            .map(|row| format!("{row}!important;"))
            .collect();
        match *state {
            LOOK_HOVER => write!(out, "@media (hover:hover){{{at}:hover{enabled}{{{rows}}}}}"),
            LOOK_FOCUS_VISIBLE => write!(out, "{at}:focus-visible{enabled}{{{rows}}}"),
            _ => {
                let eased: Vec<String> = css
                    .split_terminator(';')
                    .filter_map(|row| row.split_once(':'))
                    .map(|(property, _)| format!("{property} {EASE}"))
                    .collect();
                write!(
                    out,
                    "{at}[data-pressed]{enabled}{{{rows}}}{at}[data-exact-easing]{{transition:{}!important}}",
                    eased.join(",")
                )
            }
        }
        .expect("a String");
    }
    out
}

impl<D: DataSource> Host<D> {
    /// Send the node's look rules when they changed (`""` clears them).
    pub(in crate::host) fn emit_looks(&mut self, id: ViewId, batch: &mut Batch) {
        let Some(node) = self.runner.kernel().node(id) else {
            return;
        };
        let fonts = &self.font_names;
        let looks = declarations(node.looks(), |l| crate::css::css_text(l, fonts).0);
        let rules = rules(&format!("[data-view=\"{id}\"]"), &looks);
        let m = self.mirror.entry(id).or_default();
        if rules != m.looks {
            batch.looks(id, &rules);
            m.looks = rules;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_compose_hover_then_focus_then_press_over_the_base() {
        let mut active = StyleProps::default();
        active.opacity = 0.5;
        active.mask.set(StyleId::Opacity);
        active.press_scale = 0.9;
        active.mask.set(StyleId::PressScale);
        let hover = active.clone();
        let looks = [(LOOK_ACTIVE, active), (LOOK_HOVER, hover)];
        let css = rules(
            "[data-view=\"4\"]",
            &declarations(&looks, |l| crate::css::css_text(l, &[]).0),
        );
        let at = |s: &str| css.find(s).unwrap_or_else(|| panic!("{s} in {css}"));
        assert!(at(":hover") < at("[data-pressed]"), "{css}");
        assert!(css.contains("opacity:0.5!important"), "{css}");
        assert!(
            css.contains("[data-exact-easing]{transition:opacity 120ms"),
            "{css}"
        );
        assert!(
            !css.contains("--exact-press"),
            "the scale is the node's own: {css}"
        );
        assert!(rules("x", &[]).is_empty(), "no looks, no rule");
    }
}
