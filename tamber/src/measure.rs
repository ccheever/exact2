//! Length and number tokens (LLP 1121 D3, stage 6): a length is px, one
//! `rem` or `em`, or one named length (a text style, `-exact-hairline`);
//! `calc()`, `min()`, `max()` and `clamp()` over px and numbers fold to px.
//! A number is a number, or such a function over numbers.

use crate::css::Item;
use crate::fold::{fail, FoldError, Folded, Named, Names};

/// What QUEUE's `rem`/`em` item says a relative length inside a function
/// waits on.
const RELATIVE_IN_CALC: &str = "the kernel takes only px inside `calc()`, `min()`, `max()` and `clamp()` until QUEUE's item \"`rem`/`em` where the kernel still takes only `px`\"";

/// A token's kind (D3), as the IR and the plan name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A colour.
    Color,
    /// A length.
    Length,
    /// A number.
    Number,
}

impl Kind {
    /// The name the grammar, the IR and the plan write.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Color => "color",
            Kind::Length => "length",
            Kind::Number => "number",
        }
    }

    /// The kind a grammar name names, of those built.
    pub fn parse(name: &str) -> Option<Kind> {
        match name {
            "color" => Some(Kind::Color),
            "length" => Some(Kind::Length),
            "number" => Some(Kind::Number),
            _ => None,
        }
    }
}

/// A folded length, in the form the kernel reads back.
#[derive(Debug, Clone, PartialEq)]
pub enum Length {
    /// Pixels: a `px` value, or a function folded to px.
    Px(f64),
    /// Root font sizes.
    Rem(f64),
    /// The element's font sizes.
    Em(f64),
    /// A named length as written: a text style (`-exact-body`) or
    /// `-exact-hairline`, which the kernel's relative side table reads.
    Named(String),
}

impl Length {
    /// The text the plan carries.
    pub fn css(&self) -> String {
        match self {
            Length::Px(n) => format!("{}px", num(*n)),
            Length::Rem(n) => format!("{}rem", num(*n)),
            Length::Em(n) => format!("{}em", num(*n)),
            Length::Named(n) => n.clone(),
        }
    }
}

/// A folded token value, of any kind built.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A colour.
    Color(Folded),
    /// A length.
    Length(Length),
    /// A number.
    Number(f64),
}

impl Value {
    /// Its kind.
    pub fn kind(&self) -> Kind {
        match self {
            Value::Color(_) => Kind::Color,
            Value::Length(_) => Kind::Length,
            Value::Number(_) => Kind::Number,
        }
    }

    /// The text the plan carries, which the kernel parses to the form.
    pub fn css(&self) -> String {
        match self {
            Value::Color(f) => f.css(),
            Value::Length(l) => l.css(),
            Value::Number(n) => num(*n),
        }
    }

    /// The colour, when it is one.
    pub fn color(&self) -> Option<&Folded> {
        match self {
            Value::Color(f) => Some(f),
            _ => None,
        }
    }
}

/// A number as CSS writes it: its shortest decimal, with no exponent, and
/// `-0` as `0`.
pub(crate) fn num(n: f64) -> String {
    if n == 0.0 {
        return "0".into();
    }
    exact_num::Shortest(n).to_string()
}

/// The kind a value's text makes it, where the text says (§3: "a kind is
/// inferred from the value where it can be"): a dimension, a named length
/// or a function over dimensions is a length, a number or a function over
/// numbers a number; `None` otherwise (a colour, or a name to follow).
pub(crate) fn inferred(items: &[Item], named: &dyn Fn(&str) -> bool) -> Option<Kind> {
    fn dims(items: &[Item]) -> bool {
        items.iter().any(|i| match i {
            Item::Dim(..) => true,
            Item::Func(_, args, _) => args.iter().any(|a| dims(a)),
            _ => false,
        })
    }
    match items {
        [Item::Dim(_, unit)] if matches!(unit.as_str(), "px" | "rem" | "em") => Some(Kind::Length),
        [Item::Number(_)] => Some(Kind::Number),
        [Item::Ident(name)] if named(name) => Some(Kind::Length),
        [Item::Func(f, args, _)] if matches!(f.as_str(), "calc" | "min" | "max" | "clamp") => {
            Some(if args.iter().any(|a| dims(a)) {
                Kind::Length
            } else {
                Kind::Number
            })
        }
        _ => None,
    }
}

/// Fold a whole value as a length.
pub(crate) fn length(
    items: &[Item],
    names: &mut dyn Names,
    named: &dyn Fn(&str) -> bool,
) -> Result<Length, FoldError> {
    let [one] = items else {
        return fail("tamber-kind", "a length is one value");
    };
    match one {
        Item::Dim(n, unit) => match unit.as_str() {
            "px" => Ok(Length::Px(*n)),
            "rem" => Ok(Length::Rem(*n)),
            "em" => Ok(Length::Em(*n)),
            other => fail(
                "tamber-kind",
                format!(
                    "`{}{other}`: a length is px, `rem`, `em` or a named length",
                    num(*n)
                ),
            ),
        },
        Item::Number(n) if *n == 0.0 => Ok(Length::Px(0.0)),
        Item::Number(n) => fail(
            "tamber-kind",
            format!(
                "`{}` is a number; a length says its unit (`{}px`)",
                num(*n),
                num(*n)
            ),
        ),
        Item::Ident(name) if named(name) => Ok(Length::Named(name.clone())),
        Item::Ident(name) => match names.named(name)? {
            Named::Length(l) => Ok(l),
            Named::Number(_) => fail("tamber-kind", format!("`{name}` is a number, not a length")),
            Named::Color(_) | Named::Role(_) => {
                fail("tamber-kind", format!("`{name}` is a colour, not a length"))
            }
            Named::Unknown => fail(
                "tamber-unknown-token",
                format!("`{name}` is no token, dial or named length"),
            ),
        },
        Item::Func(f, args, text) if matches!(f.as_str(), "calc" | "min" | "max" | "clamp") => {
            match func(f, args, text, names)? {
                Quantity::Px(n) => Ok(Length::Px(n)),
                Quantity::Number(_) => fail(
                    "tamber-kind",
                    format!("`{text}` is a number; a length's function folds to px"),
                ),
            }
        }
        Item::Func(f, _, _) if f == "var" => fail(
            "tamber-var",
            "a token is named bare (`space.row`), not through `var()` (LLP 1121 D3)",
        ),
        Item::Func(_, _, text) => fail("tamber-kind", format!("`{text}` is not a length")),
        Item::Percent(n) => fail(
            "tamber-kind",
            format!(
                "`{}%`: a length token is px, `rem`, `em` or a named length, not a percentage",
                num(*n)
            ),
        ),
        _ => fail("tamber-kind", "not a length"),
    }
}

/// Fold a whole value as a number.
pub(crate) fn number(items: &[Item], names: &mut dyn Names) -> Result<f64, FoldError> {
    let [one] = items else {
        return fail("tamber-kind", "a number is one value");
    };
    match one {
        Item::Number(n) => Ok(*n),
        Item::Ident(name) => match names.named(name)? {
            Named::Number(n) => Ok(n),
            Named::Length(_) => fail("tamber-kind", format!("`{name}` is a length, not a number")),
            Named::Color(_) | Named::Role(_) => {
                fail("tamber-kind", format!("`{name}` is a colour, not a number"))
            }
            Named::Unknown => fail(
                "tamber-unknown-token",
                format!("`{name}` is no token or dial"),
            ),
        },
        Item::Func(f, args, text) if matches!(f.as_str(), "calc" | "min" | "max" | "clamp") => {
            match func(f, args, text, names)? {
                Quantity::Number(n) => Ok(n),
                Quantity::Px(_) => {
                    fail("tamber-kind", format!("`{text}` is a length, not a number"))
                }
            }
        }
        Item::Percent(n) => fail(
            "tamber-kind",
            format!(
                "`{}%`: a number token is a number (`{}`)",
                num(*n),
                num(*n / 100.0)
            ),
        ),
        Item::Dim(n, unit) => fail(
            "tamber-kind",
            format!("`{}{unit}` is a length, not a number", num(*n)),
        ),
        _ => fail("tamber-kind", "not a number"),
    }
}

/// A function's operand: px or a plain number.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Quantity {
    Px(f64),
    Number(f64),
}

fn func(
    f: &str,
    args: &[Vec<Item>],
    text: &str,
    names: &mut dyn Names,
) -> Result<Quantity, FoldError> {
    let mut each = args
        .iter()
        .map(|a| sum(a, text, names))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let first = each.next();
    let rest: Vec<Quantity> = each.collect();
    let same = |a: Quantity, b: Quantity| match (a, b) {
        (Quantity::Px(_), Quantity::Px(_)) | (Quantity::Number(_), Quantity::Number(_)) => Ok(()),
        _ => fail(
            "tamber-kind",
            format!("`{text}` mixes a length and a number"),
        ),
    };
    let v = |q: Quantity| match q {
        Quantity::Px(n) | Quantity::Number(n) => n,
    };
    let Some(first) = first else {
        return fail("tamber-syntax", format!("`{text}` is empty"));
    };
    let pick = |pick: fn(f64, f64) -> f64| -> Result<Quantity, FoldError> {
        let mut out = first;
        for q in &rest {
            same(out, *q)?;
            out = with(out, pick(v(out), v(*q)));
        }
        Ok(out)
    };
    match (f, rest.len()) {
        // `calc()`, or a group in one: `(2px + 3px)`.
        ("calc" | "", 0) => Ok(first),
        ("calc", _) => fail(
            "tamber-syntax",
            format!("`{text}`: `calc()` takes one expression"),
        ),
        ("min", _) => pick(f64::min),
        ("max", _) => pick(f64::max),
        ("clamp", 2) => {
            same(first, rest[0])?;
            same(first, rest[1])?;
            let (lo, mid, hi) = (v(first), v(rest[0]), v(rest[1]));
            Ok(with(first, mid.min(hi).max(lo)))
        }
        ("clamp", _) => fail(
            "tamber-syntax",
            format!("`{text}`: `clamp()` takes three values"),
        ),
        _ => fail("tamber-kind", format!("`{text}` is not folded")),
    }
}

fn with(q: Quantity, n: f64) -> Quantity {
    match q {
        Quantity::Px(_) => Quantity::Px(n),
        Quantity::Number(_) => Quantity::Number(n),
    }
}

/// `a + b - c …`, each term a product.
fn sum(items: &[Item], text: &str, names: &mut dyn Names) -> Result<Quantity, FoldError> {
    let mut terms = items.split(|i| matches!(i, Item::Op('+' | '-'))).peekable();
    let ops: Vec<char> = items
        .iter()
        .filter_map(|i| match i {
            Item::Op(c @ ('+' | '-')) => Some(*c),
            _ => None,
        })
        .collect();
    let first = product(terms.next().unwrap_or(&[]), text, names)?;
    let mut out = first;
    for (op, term) in ops.into_iter().zip(terms) {
        let t = product(term, text, names)?;
        out = match (out, t) {
            (Quantity::Px(a), Quantity::Px(b)) => {
                Quantity::Px(if op == '+' { a + b } else { a - b })
            }
            (Quantity::Number(a), Quantity::Number(b)) => {
                Quantity::Number(if op == '+' { a + b } else { a - b })
            }
            _ => {
                return fail(
                    "tamber-kind",
                    format!("`{text}` adds a length and a number"),
                )
            }
        };
    }
    Ok(out)
}

/// `a * b / c …`: at most one side of each a length.
fn product(items: &[Item], text: &str, names: &mut dyn Names) -> Result<Quantity, FoldError> {
    let mut out: Option<Quantity> = None;
    // The operator before the next operand; none before the first, and one
    // between each two.
    let mut op: Option<char> = None;
    for item in items {
        match item {
            Item::Op('*') | Item::Slash if out.is_some() && op.is_none() => {
                op = Some(if matches!(item, Item::Slash) {
                    '/'
                } else {
                    '*'
                });
                continue;
            }
            Item::Op('*') | Item::Slash => {
                return fail("tamber-syntax", format!("`{text}`: an operand is missing"))
            }
            _ if out.is_some() && op.is_none() => {
                return fail(
                    "tamber-syntax",
                    format!("`{text}`: two operands need an operator between them"),
                )
            }
            _ => {}
        }
        let op = op.take().unwrap_or('*');
        let q = operand(item, text, names)?;
        out = Some(match out {
            None => q,
            Some(a) => match (a, q, op) {
                (Quantity::Number(a), Quantity::Number(b), '*') => Quantity::Number(a * b),
                (Quantity::Number(a), Quantity::Number(b), _) => Quantity::Number(a / b),
                (Quantity::Px(a), Quantity::Number(b), '*')
                | (Quantity::Number(b), Quantity::Px(a), '*') => Quantity::Px(a * b),
                (Quantity::Px(a), Quantity::Number(b), _) => Quantity::Px(a / b),
                _ => {
                    return fail(
                        "tamber-kind",
                        format!("`{text}`: a length times or over a length is no length"),
                    )
                }
            },
        });
    }
    if op.is_some() {
        return fail("tamber-syntax", format!("`{text}`: an operand is missing"));
    }
    match out {
        Some(Quantity::Px(n) | Quantity::Number(n)) if !n.is_finite() => fail(
            "tamber-kind",
            format!("`{text}` does not fold to a finite value"),
        ),
        Some(q) => Ok(q),
        None => fail("tamber-syntax", format!("`{text}`: an operand is missing")),
    }
}

fn operand(item: &Item, text: &str, names: &mut dyn Names) -> Result<Quantity, FoldError> {
    match item {
        Item::Number(n) => Ok(Quantity::Number(*n)),
        Item::Dim(n, unit) if unit == "px" => Ok(Quantity::Px(*n)),
        Item::Dim(_, unit) if unit == "rem" || unit == "em" => {
            fail("tamber-kind", format!("`{text}`: {RELATIVE_IN_CALC}"))
        }
        Item::Dim(n, unit) => fail(
            "tamber-kind",
            format!("`{}{unit}` in `{text}`: a length is px here", num(*n)),
        ),
        Item::Func(f, args, inner)
            if matches!(f.as_str(), "calc" | "min" | "max" | "clamp" | "") =>
        {
            func(f, args, inner, names)
        }
        Item::Ident(name) => match names.named(name)? {
            Named::Number(n) => Ok(Quantity::Number(n)),
            Named::Length(Length::Px(n)) => Ok(Quantity::Px(n)),
            Named::Length(_) => fail(
                "tamber-kind",
                format!("`{name}` in `{text}`: {RELATIVE_IN_CALC}"),
            ),
            Named::Color(_) | Named::Role(_) => {
                fail("tamber-kind", format!("`{name}` is a colour, in `{text}`"))
            }
            Named::Unknown => fail(
                "tamber-unknown-token",
                format!("`{name}` in `{text}` is no token or dial"),
            ),
        },
        Item::Percent(_) => fail(
            "tamber-kind",
            format!("`{text}`: a percentage has no px at build"),
        ),
        _ => fail("tamber-syntax", format!("`{text}` is not a calculation")),
    }
}
