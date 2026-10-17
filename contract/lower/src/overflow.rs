//! What a box's literal overflow rows compute to, for the checks that ask
//! whether it scrolls (a glass group on a scroller, a route's scroller, a
//! scroll ancestor). CSS Overflow 3 §3.1 as the kernel's `style.rs` lays it
//! out: the last of `overflow`, `overflow-x` and `overflow-y` wins per axis,
//! and beside an axis that is not `visible`, `visible` computes to `auto`
//! (no row admits `clip`).
use contract_syntax::{Attr, Expr};

/// One way the rows can come out: each axis's computed value, `None` where
/// a computed expression decides it.
type Case<'a> = (Option<&'a str>, Option<&'a str>);

/// Every way a box's overflow rows can compute: one case per combination of
/// the literal arms of a choice (`?:`, `match`, `let`), an arm's two axes
/// kept together.
pub(crate) struct Overflow<'a>(Vec<Case<'a>>);

/// A literal `overflow`'s x and y values: `"hidden auto"` is x hidden and y
/// auto, one value is both axes'.
fn axes(text: &str) -> (&str, &str) {
    let v = exact_kernel::style::sides::values(text);
    let x = v.first().copied().unwrap_or(text);
    (x, v.get(1).copied().unwrap_or(x))
}

/// The literal values `e` can take, `None` for one that declares nothing
/// (an absent arm, `""`, `"unset"`: the row's default); `None` overall when
/// some value is computed.
fn arms(e: &Expr) -> Option<Vec<Option<&str>>> {
    Some(match e {
        Expr::Str(s, _) => vec![(!matches!(s.trim(), "" | "unset")).then_some(s.as_str())],
        Expr::None(_) => vec![None],
        Expr::Ternary(_, yes, no, _) => [arms(yes)?, arms(no)?].concat(),
        Expr::Match { some, none, .. } => [arms(some)?, arms(none)?].concat(),
        Expr::Let { body, .. } => arms(body)?,
        _ => return None,
    })
}

impl<'a> Overflow<'a> {
    /// The rows `attrs` declare, over `y`: a scroller's `scroll`, else
    /// `visible` (x is `visible`).
    pub(crate) fn of(attrs: impl IntoIterator<Item = &'a Attr>, y: &'a str) -> Self {
        let mut cases: Vec<Case<'a>> = vec![(Some("visible"), Some(y))];
        for a in attrs {
            let (to_x, to_y) = match a.name.as_str() {
                "overflow" => (true, true),
                "overflow-x" | "overflowX" => (true, false),
                "overflow-y" | "overflowY" => (false, true),
                _ => continue,
            };
            let Some(arms) = arms(&a.value) else {
                for case in &mut cases {
                    if to_x {
                        case.0 = None;
                    }
                    if to_y {
                        case.1 = None;
                    }
                }
                continue;
            };
            let mut next = Vec::new();
            for case in &cases {
                for arm in &arms {
                    let (vx, vy) = arm.map_or(("visible", y), axes);
                    let new = (
                        if to_x { Some(vx) } else { case.0 },
                        if to_y { Some(vy) } else { case.1 },
                    );
                    if !next.contains(&new) {
                        next.push(new);
                    }
                }
            }
            cases = next;
        }
        let computed = |v: Option<&'a str>, other: Option<&str>| match (v, other) {
            (Some("visible"), Some(o)) if o != "visible" => Some("auto"),
            _ => v,
        };
        Overflow(
            cases
                .into_iter()
                .map(|(x, y)| (computed(x, y), computed(y, x)))
                .collect(),
        )
    }

    /// Whether some case may scroll on either axis: a known axis that is
    /// neither `visible` nor `hidden`, or a known non-`visible` axis beside a
    /// computed one (a `visible` there would compute to `auto`).
    pub(crate) fn may_scroll(&self) -> bool {
        let scrolls = |v: &str| !matches!(v, "visible" | "hidden");
        self.0.iter().any(|case| match *case {
            (Some(x), Some(y)) => scrolls(x) || scrolls(y),
            (Some(k), None) | (None, Some(k)) => k != "visible",
            (None, None) => false,
        })
    }

    /// Whether y scrolls in every case (`Some(true)`), in none
    /// (`Some(false)`), or the compiler cannot tell (`None`).
    pub(crate) fn scrolls_y(&self) -> Option<bool> {
        let each = self.0.iter().map(|case| match *case {
            (_, None) | (None, Some("visible")) => None,
            (_, Some(y)) => Some(!matches!(y, "visible" | "hidden")),
        });
        let mut all = each.collect::<Option<Vec<bool>>>()?.into_iter();
        let first = all.next()?;
        all.all(|v| v == first).then_some(first)
    }

    /// Whether y may scroll: a known y that scrolls, or a computed y. A
    /// computed x beside a `visible` y is not counted: before the
    /// two-value split no x row made a scroll ancestor.
    pub(crate) fn may_scroll_y(&self) -> bool {
        self.0
            .iter()
            .any(|case| case.1.is_none_or(|y| !matches!(y, "visible" | "hidden")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract_syntax::Span;

    fn row(name: &str, value: Expr) -> Attr {
        Attr {
            name: name.into(),
            value,
            span: Span::default(),
            written: true,
        }
    }
    fn lit(s: &str) -> Expr {
        Expr::Str(s.into(), Span::default())
    }
    fn computed() -> Expr {
        Expr::Ident("o".into(), Span::default())
    }
    fn choice(yes: &str, no: &str) -> Expr {
        Expr::Ternary(
            Box::new(computed()),
            Box::new(lit(yes)),
            Box::new(lit(no)),
            Span::default(),
        )
    }
    fn cases(rows: &[Attr], y: &str) -> Vec<(Option<String>, Option<String>)> {
        Overflow::of(rows, y)
            .0
            .iter()
            .map(|(x, y)| (x.map(String::from), y.map(String::from)))
            .collect()
    }
    fn known(x: &str, y: &str) -> (Option<String>, Option<String>) {
        (Some(x.into()), Some(y.into()))
    }

    /// CSS Overflow 3 §3.1: a `visible` axis beside a scrolling or hidden one
    /// computes to `auto`; the last row of an axis wins, a later literal over
    /// an earlier computed one; `""` and `"unset"` declare nothing.
    #[test]
    fn each_axis_computes_as_css_does() {
        assert_eq!(
            cases(&[row("overflow", lit("hidden visible"))], "visible"),
            [known("hidden", "auto")]
        );
        assert_eq!(
            cases(&[row("overflow-x", lit("hidden"))], "visible"),
            [known("hidden", "auto")]
        );
        assert_eq!(
            cases(&[row("overflow-y", lit("hidden"))], "scroll"),
            [known("auto", "hidden")]
        );
        assert_eq!(cases(&[], "scroll"), [known("auto", "scroll")]);
        let late = [row("overflow-x", computed()), row("overflow", lit("auto"))];
        assert_eq!(cases(&late, "visible"), [known("auto", "auto")]);
        assert!(Overflow::of(&late, "visible").may_scroll());
        for reset in ["", "unset"] {
            let rows = [row("overflow", lit(reset))];
            assert_eq!(
                cases(&rows, "visible"),
                [known("visible", "visible")],
                "{reset:?}"
            );
            assert!(!Overflow::of(&rows, "visible").may_scroll());
        }
    }

    /// A choice's arms keep their two axes together: `hidden` or `visible`
    /// never mixes into `hidden visible`.
    #[test]
    fn a_choice_is_one_case_an_arm() {
        let still = [row("overflow", choice("hidden", "visible"))];
        assert_eq!(
            cases(&still, "visible"),
            [known("hidden", "hidden"), known("visible", "visible")]
        );
        assert!(!Overflow::of(&still, "visible").may_scroll());
        assert_eq!(Overflow::of(&still, "visible").scrolls_y(), Some(false));
        let mixed = [row("overflow", choice("hidden visible", "hidden"))];
        assert!(Overflow::of(&mixed, "visible").may_scroll());
        assert_eq!(Overflow::of(&mixed, "visible").scrolls_y(), None);
    }

    /// A computed axis: beside a known non-`visible` one it may scroll; a
    /// computed x alone is no scroll ancestor, a computed y may be one.
    #[test]
    fn a_computed_axis_is_doubt() {
        let x = [row("overflow-x", computed())];
        assert!(!Overflow::of(&x, "visible").may_scroll_y());
        assert!(!Overflow::of(&x, "visible").may_scroll());
        assert_eq!(Overflow::of(&x, "visible").scrolls_y(), None);
        let y = [row("overflow-y", computed())];
        assert!(Overflow::of(&y, "visible").may_scroll_y());
        let beside = [
            row("overflow-x", lit("hidden")),
            row("overflow-y", computed()),
        ];
        assert!(Overflow::of(&beside, "visible").may_scroll());
    }
}
