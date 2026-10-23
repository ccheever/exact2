//! A length over base names the design leaves unset (LLP 1121 D11, stage
//! 12 B2): what the build cannot fold, since each host's column holds the
//! base's length. Every token reference is inlined and every constant part
//! folded, so its only free names are base names, and one expression has
//! one text however it was written (`calc(space.md + 2px + 2px)` is
//! `calc(space.md + 4px)`). Each host evaluates the text in its own column:
//! the kernel substitutes each base's length and folds the `calc()`, the JS
//! target writes each as its custom property.

use crate::measure::num;

/// The most atoms a length over base names may hold (each base, and each
/// `min()`, `max()` and `clamp()`, counted through what it holds): the
/// design checker's own bound. A token naming another copies its residual,
/// so a chain naming the next three times a level would grow 3^n; it is
/// refused at the level that passes this, in the build and the design
/// server's check alike.
pub const MAX_RESIDUAL_ATOMS: usize = 256;

/// A sum of terms, each an [`Atom`] times a coefficient, plus px.
#[derive(Debug, Clone, PartialEq)]
pub struct Residual {
    /// Each term and its coefficient, in the order first written; none is 0.
    pub terms: Vec<(Atom, f64)>,
    /// The px the terms are added to.
    pub px: f64,
}

/// What a term is.
#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    /// A base name.
    Base(String),
    /// `min()` of these, at least one a residual and at most one px.
    Min(Vec<Residual>),
    /// `max()` of these, likewise.
    Max(Vec<Residual>),
    /// `clamp()`'s low, middle and high, one at least a residual.
    Clamp(Box<[Residual; 3]>),
}

impl Residual {
    /// Base name `name`, once.
    pub(crate) fn base(name: &str) -> Residual {
        Residual::atom(Atom::Base(name.to_string()))
    }

    /// px alone.
    pub(crate) fn px(n: f64) -> Residual {
        Residual {
            terms: Vec::new(),
            px: n,
        }
    }

    fn atom(atom: Atom) -> Residual {
        Residual {
            terms: vec![(atom, 1.0)],
            px: 0.0,
        }
    }

    /// Its px, when no base is left in it (`space.md - space.md`).
    pub(crate) fn constant(&self) -> Option<f64> {
        self.terms.is_empty().then_some(self.px)
    }

    /// The base name it is, bare: an alias, which a host reads as it reads
    /// any bare base name.
    pub fn bare(&self) -> Option<&str> {
        match self.terms.as_slice() {
            [(Atom::Base(name), c)] if *c == 1.0 && self.px == 0.0 => Some(name),
            _ => None,
        }
    }

    /// `self + sign × other`, like terms added.
    pub(crate) fn plus(mut self, other: Residual, sign: f64) -> Residual {
        for (atom, c) in other.terms {
            self.add(atom, sign * c);
        }
        self.px += sign * other.px;
        self
    }

    fn add(&mut self, atom: Atom, c: f64) {
        match self.terms.iter_mut().find(|(a, _)| *a == atom) {
            Some(term) => term.1 += c,
            None => self.terms.push((atom, c)),
        }
        self.terms.retain(|(_, c)| *c != 0.0);
    }

    /// `self × n`, a term that underflows to 0 dropped (a NaN or an
    /// infinity kept, for [`Residual::finite`] to refuse).
    pub(crate) fn times(self, n: f64) -> Residual {
        self.scaled(|c| c * n)
    }

    /// `self ÷ n`, each coefficient divided (no reciprocal to overflow).
    pub(crate) fn over(self, n: f64) -> Residual {
        self.scaled(|c| c / n)
    }

    fn scaled(mut self, f: impl Fn(f64) -> f64) -> Residual {
        for (_, c) in &mut self.terms {
            *c = f(*c);
        }
        self.terms.retain(|(_, c)| *c != 0.0);
        self.px = f(self.px);
        self
    }

    /// `min()` (or `max()`) of `args`, its px ones folded into one; one
    /// left alone is itself.
    pub(crate) fn pick(args: Vec<Residual>, max: bool) -> Residual {
        let mut constant: Option<f64> = None;
        let mut rest: Vec<Residual> = Vec::new();
        for a in args {
            match a.constant() {
                Some(n) => {
                    constant = Some(constant.map_or(n, |c| if max { c.max(n) } else { c.min(n) }))
                }
                None if !rest.contains(&a) => rest.push(a),
                None => {}
            }
        }
        rest.extend(constant.map(Residual::px));
        if rest.len() == 1 {
            return rest.pop().expect("one");
        }
        Residual::atom(if max {
            Atom::Max(rest)
        } else {
            Atom::Min(rest)
        })
    }

    /// `clamp(low, middle, high)`, one at least a residual.
    pub(crate) fn clamp(low: Residual, middle: Residual, high: Residual) -> Residual {
        Residual::atom(Atom::Clamp(Box::new([low, middle, high])))
    }

    /// How many atoms it holds: each base, and each pick or clamp with
    /// what it holds ([`MAX_RESIDUAL_ATOMS`]).
    pub(crate) fn atoms(&self) -> usize {
        self.terms
            .iter()
            .map(|(atom, _)| match atom {
                Atom::Base(_) => 1,
                Atom::Min(args) | Atom::Max(args) => {
                    1 + args.iter().map(Residual::atoms).sum::<usize>()
                }
                Atom::Clamp(args) => 1 + args.iter().map(Residual::atoms).sum::<usize>(),
            })
            .sum()
    }

    /// Whether every coefficient and px is finite.
    pub(crate) fn finite(&self) -> bool {
        self.px.is_finite()
            && self.terms.iter().all(|(a, c)| {
                c.is_finite()
                    && match a {
                        Atom::Base(_) => true,
                        Atom::Min(args) | Atom::Max(args) => args.iter().all(Residual::finite),
                        Atom::Clamp(args) => args.iter().all(Residual::finite),
                    }
            })
    }

    /// The base names it reads, each once, in order.
    pub fn bases(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.each_base(&mut |n| {
            if !out.contains(&n) {
                out.push(n);
            }
        });
        out
    }

    fn each_base<'a>(&'a self, f: &mut dyn FnMut(&'a str)) {
        for (atom, _) in &self.terms {
            match atom {
                Atom::Base(n) => f(n),
                Atom::Min(args) | Atom::Max(args) => args.iter().for_each(|a| a.each_base(f)),
                Atom::Clamp(args) => args.iter().for_each(|a| a.each_base(f)),
            }
        }
    }

    /// Its px with each base name's px `base` gives, summed in the order
    /// [`Residual::css`] writes (the terms, then the px), as a host's
    /// `calc()` sums it; `None` when `base` gives none for one or the
    /// result isn't finite.
    /// A NaN result is 0, as CSS Values 4 §10.9 censors one at the top of
    /// a calculation and the kernel's does.
    pub fn fold(&self, base: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
        match self.sum_with(base)? {
            n if n.is_nan() => Some(0.0),
            n => n.is_finite().then_some(n),
        }
    }

    /// [`Residual::fold`] with infinities kept inside, as a host's `calc()`
    /// keeps them until its result (`min(space.md * 1e308, 20px)` is 20px).
    fn sum_with(&self, base: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
        let mut out = 0.0;
        for (atom, c) in &self.terms {
            let v = match atom {
                Atom::Base(n) => base(n)?,
                // A NaN argument makes the pick NaN (§10.9), where Rust's
                // `min` and `max` would drop it.
                Atom::Min(args) => args
                    .iter()
                    .map(|a| a.sum_with(base))
                    .try_fold(f64::INFINITY, |m, v| Some(nan_or(m, v?, f64::min)))?,
                Atom::Max(args) => args
                    .iter()
                    .map(|a| a.sum_with(base))
                    .try_fold(f64::NEG_INFINITY, |m, v| Some(nan_or(m, v?, f64::max)))?,
                Atom::Clamp(args) => {
                    let [lo, mid, hi] = &**args;
                    let (lo, mid, hi) =
                        (lo.sum_with(base)?, mid.sum_with(base)?, hi.sum_with(base)?);
                    nan_or(nan_or(mid, hi, f64::min), lo, f64::max)
                }
            };
            out += c * v;
        }
        Some(out + self.px)
    }

    /// The text the plan carries: a lone function as itself (`min(space.md,
    /// 20px)`), any other sum in `calc()` (`calc(space.md * 2 + 4px)`).
    pub fn css(&self) -> String {
        match self.terms.as_slice() {
            [(atom, c)] if *c == 1.0 && self.px == 0.0 => atom_css(atom),
            _ => format!("calc({})", self.sum()),
        }
    }

    /// Its sum as CSS writes one inside a math function: `space.md * 2 -
    /// 4px`.
    fn sum(&self) -> String {
        let mut out = String::new();
        for (i, (atom, c)) in self.terms.iter().enumerate() {
            let text = atom_css(atom);
            if i == 0 {
                out.push_str(&text);
                if *c != 1.0 {
                    out.push_str(&format!(" * {}", num(*c)));
                }
                continue;
            }
            out.push_str(if *c < 0.0 { " - " } else { " + " });
            out.push_str(&text);
            if c.abs() != 1.0 {
                out.push_str(&format!(" * {}", num(c.abs())));
            }
        }
        let n = self.px;
        if self.terms.is_empty() {
            out.push_str(&format!("{}px", num(n)));
        } else if n < 0.0 {
            out.push_str(&format!(" - {}px", num(-n)));
        } else if n > 0.0 {
            out.push_str(&format!(" + {}px", num(n)));
        }
        out
    }
}

/// `f(a, b)`, or NaN when either is.
fn nan_or(a: f64, b: f64, f: fn(f64, f64) -> f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        f(a, b)
    }
}

fn atom_css(atom: &Atom) -> String {
    let args = |f: &str, args: &[Residual]| {
        let each: Vec<String> = args.iter().map(Residual::sum).collect();
        format!("{f}({})", each.join(", "))
    };
    match atom {
        Atom::Base(n) => n.clone(),
        Atom::Min(a) => args("min", a),
        Atom::Max(a) => args("max", a),
        Atom::Clamp(a) => args("clamp", &a[..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md() -> Residual {
        Residual::base("space.md")
    }

    #[test]
    fn like_terms_add_and_constants_fold() {
        let r = md()
            .plus(Residual::px(2.0), 1.0)
            .plus(Residual::px(2.0), 1.0);
        assert_eq!(r.css(), "calc(space.md + 4px)");
        let r = md()
            .times(2.0)
            .plus(md(), 1.0)
            .plus(Residual::px(1.0), -1.0);
        assert_eq!(r.css(), "calc(space.md * 3 - 1px)");
        let r = md().plus(Residual::base("space.lg"), -1.0);
        assert_eq!(r.css(), "calc(space.md - space.lg)");
        assert_eq!(md().plus(md(), -1.0).constant(), Some(0.0));
        assert_eq!(md().times(-1.0).css(), "calc(space.md * -1)");
        assert_eq!(md().bare(), Some("space.md"));
        assert_eq!(md().times(2.0).bare(), None);
    }

    #[test]
    fn a_pick_folds_its_px_and_keeps_one_alone() {
        let r = Residual::pick(
            vec![md(), Residual::px(20.0), Residual::px(10.0), md()],
            false,
        );
        assert_eq!(r.css(), "min(space.md, 10px)");
        assert_eq!(Residual::pick(vec![md(), md()], true), md());
        let r = Residual::pick(vec![md().times(2.0), Residual::px(4.0)], true)
            .plus(Residual::px(1.0), 1.0);
        assert_eq!(r.css(), "calc(max(space.md * 2, 4px) + 1px)");
        let r = Residual::clamp(Residual::px(4.0), md(), Residual::px(24.0));
        assert_eq!(r.css(), "clamp(4px, space.md, 24px)");
    }

    #[test]
    fn it_folds_with_each_bases_px() {
        let px = |n: &str| match n {
            "space.md" => Some(16.0),
            "space.lg" => Some(20.0),
            _ => None,
        };
        let r = md().times(2.0).plus(Residual::px(4.0), 1.0);
        assert_eq!(r.fold(&px), Some(36.0));
        let r = Residual::pick(vec![md(), Residual::base("space.lg")], true);
        assert_eq!(r.fold(&px), Some(20.0));
        let r = Residual::clamp(Residual::px(4.0), md().times(3.0), Residual::px(24.0));
        assert_eq!(r.fold(&px), Some(24.0));
        assert_eq!(Residual::base("space.xl").fold(&px), None);
        // An infinity inside a pick is a host's: `min(inf, 20px)` is 20px.
        let r = Residual::pick(
            vec![md().times(1e308).times(10.0), Residual::px(20.0)],
            false,
        );
        assert_eq!(r.fold(&px), Some(20.0));
        assert_eq!(md().times(1e308).times(10.0).fold(&px), None);
        // Opposing infinities are NaN, which a pick keeps and the result
        // censors to 0 (the kernel's `min(inf - inf, 20px)` is 0px).
        let big = md().times(1e308).times(10.0);
        let nan = big.plus(Residual::base("space.lg").times(1e308).times(10.0), -1.0);
        let picked = Residual::pick(vec![nan, Residual::px(20.0)], false);
        assert_eq!(picked.fold(&px), Some(0.0));
        assert_eq!(r.bases(), ["space.md"]);
    }
}
