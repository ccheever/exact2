//! A node's `:hover` and `:focus-visible` looks on the page (LLP 1121 D8,
//! stage 8 B), as one rule a state in the page's looks sheet, composed in
//! that order over the node's inline base, so rows a look doesn't name show
//! the base. They show at once (no transition: a look's colours snap, under
//! Reduce Motion too), and under the shell's pressed paint, which `:active`
//! lowers to (LLP 1061 D7, index.html's important rules).

use super::*;
use exact_kernel::{StyleProps, LOOK_HOVER};

/// Each look's declarations; a look with none is left out. `css` writes
/// them ([`crate::css::css_text`], or the JS target's relative form).
pub fn declarations(
    looks: &[(u8, StyleProps)],
    css: impl Fn(&StyleProps) -> String,
) -> Vec<(u8, String)> {
    (looks.iter())
        .map(|(state, look)| (*state, css(look)))
        .filter(|(_, css)| !css.is_empty())
        .collect()
}

/// The looks' [`declarations`] for the element `at` selects, as page rules,
/// empty for none: `:hover` (where the pointer hovers), then
/// `:focus-visible`, the last winning, each also where it is held; a
/// disabled node takes none.
pub fn rules(at: &str, looks: &[(u8, String)]) -> String {
    let mut out = String::new();
    // A disabled box says `disabled` (a bool's text; none, or "false", is enabled).
    let enabled = ":not(:disabled,[disabled]:not([disabled=\"false\"]))";
    for (state, css) in looks {
        let rows: String = (css.split_terminator(';'))
            .map(|row| format!("{row}!important;"))
            .collect();
        match *state {
            // A held state (LLP 1122 D8, `data-exact-held`) is the state's own
            // rule, whatever the pointer (the input glue's `hold`).
            LOOK_HOVER => write!(
                out,
                "@media (hover:hover){{{at}:hover{enabled}{{{rows}}}}}{at}[data-exact-held~=hover]{enabled}{{{rows}}}"
            ),
            _ => write!(
                out,
                "{at}:focus-visible{enabled},{at}[data-exact-held~=focus-visible]{enabled}{{{rows}}}"
            ),
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
    fn looks_compose_hover_then_focus_over_the_base() {
        let mut hover = StyleProps {
            opacity: 0.5,
            ..Default::default()
        };
        hover.mask.set(exact_kernel::StyleId::Opacity);
        let looks = [
            (LOOK_HOVER, hover.clone()),
            (exact_kernel::LOOK_FOCUS_VISIBLE, hover),
        ];
        let css = rules(
            "[data-view=\"4\"]",
            &declarations(&looks, |l| crate::css::css_text(l, &[]).0),
        );
        let at = |s: &str| css.find(s).unwrap_or_else(|| panic!("{s} in {css}"));
        assert!(at(":hover") < at("[data-exact-held~=hover]"), "{css}");
        assert!(
            at("[data-exact-held~=hover]") < at(":focus-visible"),
            "{css}"
        );
        assert!(
            css.contains("[data-exact-held~=focus-visible]"),
            "a held state's rule: {css}"
        );
        assert!(css.contains("opacity:0.5!important"), "{css}");
        assert!(
            !css.contains("transition"),
            "the rules add no transition: {css}"
        );
        assert!(rules("x", &[]).is_empty(), "no looks, no rule");
    }
}
