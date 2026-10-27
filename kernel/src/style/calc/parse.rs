//! Parse and type-check CSS calculation trees, then simplify them
//! (CSS Values 4 §10.8–§10.10). `cssparser` handles CSS tokenization; numeric
//! values are read from the source as `f64` because its tokens store `f32`.
//!
//! Unsupported: `em`/`rem` (font resolution happens after commit) and unit
//! algebra beyond multiplying or dividing a length by a number.

use super::super::env::{self, EnvRefusal};
use super::super::{Dimension, ViewportUnit};
use super::{EnvTerm, Node, Unit, MAX_DEPTH, MAX_VALUES};
use cssparser::{
    match_ignore_ascii_case, BasicParseErrorKind, Delimiter, ParseError, ParseErrorKind, Parser,
    ParserInput, Token,
};

/// Why a length text that names a math function is not one the kernel
/// holds.
pub(crate) mod refusal {
    pub(crate) const RELATIVE: &str =
        "rem and em are not supported inside calc(), min(), max() or clamp(); write px";
    pub(crate) const UNITLESS: &str = "a number where a length goes inside calc(), min(), max() or clamp() takes a unit, as CSS requires, zero too: 0px, 15px";
    pub(crate) const NOT_A_LENGTH: &str = "calc(), min(), max() and clamp() take px (or in, cm, mm, q, pt, pc, ch, lh) lengths, percentages, env(safe-area-inset-*), env(viewport-segment-* x y), env(titlebar-area-*), viewport lengths (vw, vh, vmin, vmax, svh, lvh, dvh, …), numbers and e, pi, infinity and NaN";
    pub(crate) const FUNCTION: &str =
        "inside a length the functions are calc(), min(), max(), clamp() and env()";
    pub(crate) const NONE: &str =
        "`none` is clamp()'s first or third argument only: clamp(none, 20px, 50px)";
    pub(crate) const CLAMP_ARITY: &str =
        "clamp() takes exactly three arguments: clamp(<min>, <value>, <max>)";
    pub(crate) const EMPTY: &str =
        "min() and max() take one or more arguments, separated by commas";
    pub(crate) const SPACING: &str = "`+` and `-` take white space on both sides, as CSS requires: calc(max(15px, env(safe-area-inset-bottom)) + 44px)";
    pub(crate) const PRODUCT: &str =
        "`*` multiplies by a number and `/` divides by one: a length times or over a length is not supported";
    pub(crate) const SYNTAX: &str =
        "a math function takes arguments separated by commas, inside balanced parentheses";
    pub(crate) const TOP_SUM: &str = "a sum is a length only inside calc() or a comparison: calc(max(15px, env(safe-area-inset-bottom)) + 44px)";
    pub(crate) const DEPTH: &str =
        "calc(), min(), max() and clamp() nest at most 32 deep in one length, parentheses included";
    pub(crate) const SIZE: &str = "one length holds at most 32 values";
    pub(crate) const OWNER_NAME: &str = "var() in a length names a content owner's value: --exact-safe-area-inset-<edge>, --exact-<region>-inset-<edge> or --exact-viewport-segment-<field>-<x>-<y> (LLP 1123 D9)";
    pub(crate) const FALLBACK: &str =
        "an env() with a fallback is read once substituted, by the property it is a value of";
    pub(crate) const RELATIVE_FALLBACK: &str =
        "rem and em are not supported in an env() fallback, which is read where no font size is known; write px";
    pub(crate) const TOO_LONG: &str = "a length with an env() fallback is held as written, and the wire carries at most 16 MiB of it; shorten the value";
}

/// Why a length text is refused: an `env()` variable named wrongly, or a
/// math function outside the grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refused {
    Env(EnvRefusal),
    Comparison(&'static str),
}

/// Why parsing stopped.
pub(super) enum Fail {
    Env(EnvRefusal),
    Why(&'static str),
}

impl From<EnvRefusal> for Fail {
    fn from(e: EnvRefusal) -> Fail {
        Fail::Env(e)
    }
}

/// A calculation's type (§10.9), as far as lengths need one: a number, or a
/// length (a percentage of one included).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Number,
    Length,
}

/// The math functions a length may open with.
const FUNCTIONS: [&str; 4] = ["calc", "min", "max", "clamp"];

/// The function a text opens with, by its first token, lowercased.
fn top_function(text: &str) -> Option<String> {
    let mut input = ParserInput::new(text);
    let mut p = Parser::new(&mut input);
    match p.next() {
        Ok(Token::Function(name)) => Some(name.to_ascii_lowercase()),
        _ => None,
    }
}

/// Whether a length text is a math function or `env()` at its top, so a
/// refusal of it says why.
pub(crate) fn names_math(text: &str) -> bool {
    top_function(text).is_some_and(|f| f == "env" || FUNCTIONS.contains(&f.as_str()))
}

/// Whether a length text is `env()` at its top, a substitution rather than
/// a math function.
pub(crate) fn is_env(text: &str) -> bool {
    top_function(text).is_some_and(|f| f == "env")
}

/// Whether `text` is the keyword `word` alone, white space and comments
/// around it, ASCII case-insensitively, as CSS reads a keyword.
pub(super) fn is_keyword(text: &str, word: &str) -> bool {
    let mut input = ParserInput::new(text);
    let mut p = Parser::new(&mut input);
    p.expect_ident_matching(word).is_ok() && p.is_exhausted()
}

/// Whether a math function or parentheses stand among the top-level tokens
/// of a text that is no math function itself (`max(…) + 4px`, `(max(…))`),
/// so its refusal says where one goes. One level only: the work is bounded.
fn misplaced(text: &str) -> bool {
    let mut input = ParserInput::new(text);
    let mut p = Parser::new(&mut input);
    while let Ok(token) = p.next() {
        match token {
            Token::Function(name) if FUNCTIONS.iter().any(|f| name.eq_ignore_ascii_case(f)) => {
                return true
            }
            Token::ParenthesisBlock => return true,
            _ => {}
        }
    }
    false
}

/// A math function's or `env()`'s text as the dimension it simplifies to;
/// `Ok(None)` when the text is neither.
pub(crate) fn parse(text: &str) -> Result<Option<Dimension>, Refused> {
    let t = text.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    if !names_math(t) {
        // A math function inside something else: say where one goes
        return match misplaced(t) {
            true => Err(Refused::Comparison(refusal::TOP_SUM)),
            false => Ok(None),
        };
    }
    match calculation(t, true, false) {
        Ok(node) => Ok(Some(super::dimension(super::simplify::simplify(node)))),
        Err(Fail::Env(e)) => Err(Refused::Env(e)),
        Err(Fail::Why(why)) => Err(Refused::Comparison(why)),
    }
}

/// Grid's length-percentage calculations use the same grammar and limits,
/// without viewport or environment dependencies. Canonical CSS can add
/// parentheses, so the wire checks author limits on the simplified tree.
pub(crate) fn grid(text: &str, canonical: bool) -> Option<super::Comparison> {
    let tree = super::simplify::simplify(calculation(text, !canonical, true).ok()?);
    if canonical && !super::wire::within_author_limits(&tree) {
        return None;
    }
    Some(super::Comparison::intern(tree))
}

/// The simplified tree a text parses to, for the wire's check that a tree
/// is one the parser makes. The limits are an author's, on what was
/// written; the wire checks them on the tree itself ([`super::wire`]), since
/// the text the kernel writes may open parentheses an author did not.
pub(super) fn tree(text: &str) -> Option<Node> {
    calculation(text, false, false)
        .ok()
        .map(super::simplify::simplify)
}

/// One math function or `env()`, the whole of `text`, as a length's tree.
fn calculation(text: &str, limited: bool, grid: bool) -> Result<Node, Fail> {
    let mut input = ParserInput::new(text);
    let mut p = Parser::new(&mut input);
    let mut calc = Calc {
        values: 0,
        limited,
        grid,
        comparisons: 0,
    };
    let (node, ty) = calc.value(&mut p, 0).map_err(fail)?;
    match p.next() {
        Err(_) => {}
        Ok(Token::Delim('+' | '-' | '*' | '/')) => return Err(Fail::Why(refusal::TOP_SUM)),
        Ok(_) => return Err(Fail::Why(refusal::SYNTAX)),
    }
    if ty != Ty::Length {
        return Err(Fail::Why(refusal::UNITLESS));
    }
    Ok(node)
}

type Parsed<'i, T> = Result<T, ParseError<'i, Fail>>;

/// The reason in a parse error: the grammar's own, or a token out of place.
fn fail(e: ParseError<'_, Fail>) -> Fail {
    match e.kind {
        ParseErrorKind::Custom(fail) => fail,
        ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(Token::Delim('+' | '-'))) => {
            Fail::Why(refusal::SPACING)
        }
        ParseErrorKind::Basic(_) => Fail::Why(refusal::SYNTAX),
    }
}

/// A refusal at `p`.
fn why<'i, T>(p: &Parser<'i, '_>, why: &'static str) -> Parsed<'i, T> {
    Err(p.new_custom_error(Fail::Why(why)))
}

/// What a parse counts as it goes: the values written, against §10.8's
/// limits when `limited`.
struct Calc {
    values: usize,
    limited: bool,
    grid: bool,
    /// The comparisons open around the token being read: a wrong `env()`
    /// inside one says why as the comparison's refusal.
    comparisons: usize,
}

impl Calc {
    fn count<'i>(&mut self, p: &Parser<'i, '_>) -> Parsed<'i, ()> {
        self.values += 1;
        let limit = if self.limited {
            MAX_VALUES
        } else {
            super::wire::MAX_NODES
        };
        if self.values > limit {
            return why(p, refusal::SIZE);
        }
        Ok(())
    }

    /// A level deeper: a function or parentheses.
    fn deeper<'i>(&self, p: &Parser<'i, '_>, depth: usize) -> Parsed<'i, usize> {
        let limit = if self.limited {
            MAX_DEPTH
        } else {
            super::wire::MAX_LEVELS
        };
        if depth >= limit {
            return why(p, refusal::DEPTH);
        }
        Ok(depth + 1)
    }

    /// `<calc-sum>`: products joined by `+` and `-`, which take white space
    /// on both sides (§10.8); a `-` negates what follows it.
    fn sum<'i>(&mut self, p: &mut Parser<'i, '_>, depth: usize) -> Parsed<'i, (Node, Ty)> {
        let (first, ty) = self.product(p, depth)?;
        let mut items = vec![first];
        loop {
            let before = p.state();
            let spaced = space(p);
            let minus = match p.next_including_whitespace() {
                Ok(Token::Delim('+')) => false,
                Ok(Token::Delim('-')) => true,
                // `1px +2px`: a signed number where an operator goes
                Ok(
                    Token::Number { has_sign: true, .. }
                    | Token::Dimension { has_sign: true, .. }
                    | Token::Percentage { has_sign: true, .. },
                ) => return why(p, refusal::SPACING),
                _ => {
                    p.reset(&before);
                    break;
                }
            };
            if !spaced || !space(p) {
                return why(p, refusal::SPACING);
            }
            let (rhs, rty) = self.product(p, depth)?;
            if rty != ty {
                return why(p, refusal::UNITLESS);
            }
            items.push(if minus {
                Node::Negate(Box::new(rhs))
            } else {
                rhs
            });
        }
        Ok((one_or(items, Node::Sum), ty))
    }

    /// `<calc-product>`: values joined by `*` and `/`; `/` inverts what
    /// follows it. One length at most, and a divisor is a number.
    fn product<'i>(&mut self, p: &mut Parser<'i, '_>, depth: usize) -> Parsed<'i, (Node, Ty)> {
        let (first, mut ty) = self.value(p, depth)?;
        let mut items = vec![first];
        loop {
            let before = p.state();
            let divide = match p.next() {
                Ok(Token::Delim('*')) => false,
                Ok(Token::Delim('/')) => true,
                _ => {
                    p.reset(&before);
                    break;
                }
            };
            let (rhs, rty) = self.value(p, depth)?;
            match (ty, rty, divide) {
                (_, Ty::Number, _) => {}
                (Ty::Number, Ty::Length, false) => ty = Ty::Length,
                _ => return why(p, refusal::PRODUCT),
            }
            items.push(if divide {
                Node::Invert(Box::new(rhs))
            } else {
                rhs
            });
        }
        Ok((one_or(items, Node::Product), ty))
    }

    /// `<calc-value>`: a number, a dimension, a percentage, a keyword, a
    /// parenthesised sum, a math function or `env()`.
    fn value<'i>(&mut self, p: &mut Parser<'i, '_>, depth: usize) -> Parsed<'i, (Node, Ty)> {
        // The token's own text, after any white space and comments
        p.skip_whitespace();
        let start = p.position();
        let token = p.next()?.clone();
        let text = p.slice_from(start);
        if self.grid
            && matches!(&token, Token::Dimension { unit, .. } if !unit.eq_ignore_ascii_case("px"))
        {
            return why(p, "grid calculations take only px and %");
        }
        match token {
            Token::Function(name) => {
                let name = name.to_ascii_lowercase();
                match name.as_str() {
                    // `env()` substitutes a length: no level of the calculation
                    "env" if !self.grid => p.parse_nested_block(|p| self.env(p, depth)),
                    // A content owner's value (LLP 1123 D9) substitutes a length too
                    "var" if !self.grid => p.parse_nested_block(|p| self.var(p)),
                    "calc" => {
                        let depth = self.deeper(p, depth)?;
                        p.parse_nested_block(|p| self.whole(p, depth))
                    }
                    "min" | "max" | "clamp" => {
                        let depth = self.deeper(p, depth)?;
                        self.comparisons += 1;
                        let v = p.parse_nested_block(|p| self.function(p, &name, depth));
                        self.comparisons -= 1;
                        v
                    }
                    _ => why(p, refusal::FUNCTION),
                }
            }
            Token::ParenthesisBlock => {
                let depth = self.deeper(p, depth)?;
                p.parse_nested_block(|p| self.whole(p, depth))
            }
            Token::Number { .. } => {
                self.count(p)?;
                Ok((Node::Value(number_of(text), Unit::Number), Ty::Number))
            }
            Token::Percentage { .. } => {
                self.count(p)?;
                Ok((Node::Value(number_of(text), Unit::Percent), Ty::Length))
            }
            Token::Dimension { unit, .. } => {
                self.count(p)?;
                let n = number_of(text);
                let lower = unit.to_ascii_lowercase();
                let px_per = match lower.as_str() {
                    "px" => Some(1.0),
                    "in" => Some(96.0),
                    "cm" => Some(96.0 / 2.54),
                    "mm" => Some(96.0 / 25.4),
                    "q" => Some(96.0 / 101.6),
                    "pt" => Some(96.0 / 72.0),
                    "pc" => Some(16.0),
                    "ch" => Some(f64::from(super::super::cells::COLUMN)),
                    "lh" => Some(f64::from(super::super::cells::ROW)),
                    _ => None,
                };
                if let Some(scale) = px_per {
                    Ok((Node::Value(n * scale, Unit::Px), Ty::Length))
                } else if let Some(unit) = ViewportUnit::ALL.into_iter().find(|u| u.name() == lower)
                {
                    Ok((Node::Value(n, Unit::Viewport(unit)), Ty::Length))
                } else if lower == "em" || lower == "rem" {
                    why(p, refusal::RELATIVE)
                } else {
                    why(p, refusal::NOT_A_LENGTH)
                }
            }
            Token::Ident(word) => {
                self.count(p)?;
                let constant = match_ignore_ascii_case! { &word,
                    "e" => std::f64::consts::E,
                    "pi" => std::f64::consts::PI,
                    "infinity" => f64::INFINITY,
                    "-infinity" => f64::NEG_INFINITY,
                    "nan" => f64::NAN,
                    "none" => return why(p, refusal::NONE),
                    _ => return why(p, refusal::NOT_A_LENGTH),
                };
                Ok((Node::Value(constant, Unit::Number), Ty::Number))
            }
            _ => why(p, refusal::NOT_A_LENGTH),
        }
    }

    /// A sum that is the whole of a block: `calc()`'s or parentheses'.
    fn whole<'i>(&mut self, p: &mut Parser<'i, '_>, depth: usize) -> Parsed<'i, (Node, Ty)> {
        let v = self.sum(p, depth)?;
        p.expect_exhausted()?;
        Ok(v)
    }

    /// The arguments of `min(`, `max(` or `clamp(`, all of one type; `none`
    /// stands for `clamp()`'s first or third.
    fn function<'i>(
        &mut self,
        p: &mut Parser<'i, '_>,
        name: &str,
        depth: usize,
    ) -> Parsed<'i, (Node, Ty)> {
        let clamp = name == "clamp";
        if p.is_exhausted() {
            return why(
                p,
                if clamp {
                    refusal::CLAMP_ARITY
                } else {
                    refusal::EMPTY
                },
            );
        }
        let mut args: Vec<Option<Node>> = Vec::new();
        let mut ty = None;
        loop {
            let none = p.try_parse(|p| p.expect_ident_matching("none")).is_ok();
            if none {
                if !clamp || args.len() == 1 {
                    return why(p, refusal::NONE);
                }
                args.push(None);
            } else {
                let (arg, aty) = self.sum(p, depth)?;
                if ty.is_some_and(|t| t != aty) {
                    return why(p, refusal::UNITLESS);
                }
                ty = Some(aty);
                args.push(Some(arg));
            }
            match p.next() {
                Ok(Token::Comma) => {}
                Err(_) => break,
                Ok(_) => return why(p, refusal::SYNTAX),
            }
        }
        if clamp && args.len() != 3 {
            return why(p, refusal::CLAMP_ARITY);
        }
        let Some(ty) = ty else {
            return why(p, refusal::NONE);
        };
        let node = match name {
            "min" => Node::Min(args.into_iter().flatten().collect()),
            "max" => Node::Max(args.into_iter().flatten().collect()),
            _ => {
                let mut a = args.into_iter();
                Node::Clamp(Box::new([
                    a.next().flatten(),
                    a.next().flatten(),
                    a.next().flatten(),
                ]))
            }
        };
        Ok((node, ty))
    }

    /// `var(--exact-…)`: a content owner's value (LLP 1123 D9). One with a
    /// fallback makes the whole value a substitution ([`substitutes`]).
    fn var<'i>(&mut self, p: &mut Parser<'i, '_>) -> Parsed<'i, (Node, Ty)> {
        self.count(p)?;
        let name = match p.next() {
            Ok(Token::Ident(name)) => name.to_string(),
            _ => return why(p, refusal::SYNTAX),
        };
        let Some(var) = crate::style::OwnerVar::from_name(&name) else {
            return why(p, refusal::OWNER_NAME);
        };
        if p.next().is_ok() {
            return why(p, refusal::FALLBACK);
        }
        Ok((Node::Env(EnvTerm::Owner(var)), Ty::Length))
    }

    /// `env(<name> <index>*)`: the variable. One with a fallback makes the
    /// whole value a substitution, which a row holds as written and reads
    /// only once substituted ([`substitutes`]), never a tree.
    fn env<'i>(&mut self, p: &mut Parser<'i, '_>, depth: usize) -> Parsed<'i, (Node, Ty)> {
        self.count(p)?;
        let (term, fallback) = env_parts(p, depth).map_err(|fail| match fail {
            // Inside a comparison, or unlinked, as the comparison's refusal
            Fail::Env(e) if self.comparisons > 0 && e != EnvRefusal::Unlinked => {
                p.new_custom_error(Fail::Why(e.reason()))
            }
            fail => p.new_custom_error(fail),
        })?;
        if fallback.is_some() {
            return why(p, refusal::FALLBACK);
        }
        Ok((Node::Env(term), Ty::Length))
    }
}

/// Whether a length text holds an `env()` with a fallback, so CSS
/// Environment Variables 1 §3 substitutes it before the property's grammar
/// reads it: the text is held as written, only its syntax checked now (each
/// `env()` names a variable, each fallback is a `<declaration-value>`, and
/// the whole is one too), nested at most [`MAX_DEPTH`] deep.
pub(crate) fn substitutes(text: &str) -> Result<bool, Refused> {
    fn walk(p: &mut Parser<'_, '_>, depth: usize, top: bool, found: &mut bool) -> Result<(), Fail> {
        let syntax = || Fail::Why(refusal::SYNTAX);
        while let Ok(token) = p.next_including_whitespace_and_comments() {
            match token.clone() {
                Token::Function(name)
                    if name.eq_ignore_ascii_case("env") || name.eq_ignore_ascii_case("var") =>
                {
                    let parts = if name.eq_ignore_ascii_case("env") {
                        env_parts
                    } else {
                        var_parts
                    };
                    let (_, fallback) = p
                        .parse_nested_block(|p| parts(p, depth).map_err(|f| p.new_custom_error(f)))
                        .map_err(fail)?;
                    *found |= fallback.is_some();
                }
                Token::Semicolon | Token::Delim('!') if top => return Err(syntax()),
                Token::BadString(_)
                | Token::BadUrl(_)
                | Token::CloseParenthesis
                | Token::CloseSquareBracket
                | Token::CloseCurlyBracket => return Err(syntax()),
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    if depth == MAX_DEPTH {
                        return Err(Fail::Why(refusal::DEPTH));
                    }
                    p.parse_nested_block(|p| {
                        walk(p, depth + 1, false, found).map_err(|f| p.new_custom_error(f))
                    })
                    .map_err(fail)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    let mut input = ParserInput::new(text);
    let mut found = false;
    match walk(&mut Parser::new(&mut input), 0, true, &mut found) {
        Ok(()) => Ok(found),
        // A fallback's `rem` or `em` is refused as itself; syntax nothing
        // substitutes is the parse's to refuse, with its reason
        Err(Fail::Why(why)) if why == refusal::RELATIVE_FALLBACK => Err(Refused::Comparison(why)),
        Err(_) if !found => Ok(false),
        Err(Fail::Env(e)) => Err(Refused::Env(e)),
        Err(Fail::Why(why)) => Err(Refused::Comparison(why)),
    }
}

/// An `env()` block's variable and its fallback's tokens, as written; the
/// block's contents, after `env(`.
pub(super) fn env_parts(
    p: &mut Parser<'_, '_>,
    depth: usize,
) -> Result<(EnvTerm, Option<String>), Fail> {
    // The variable's name, as CSS decodes it, and its indices, from their
    // tokens: never joined as text and split again, so an escaped space or
    // no-break space stays in the name it belongs to
    let (name, indices) = p
        .parse_until_before(Delimiter::Comma, |p| {
            let (mut name, mut indices) = (None, Vec::new());
            while let Ok(token) = p.next() {
                let word = match token {
                    Token::Ident(text) if name.is_none() => text.to_string(),
                    // An index, `+0` as CSS's integers allow; a negative one
                    // as written, which `env::named` refuses
                    Token::Number {
                        int_value: Some(n), ..
                    } if *n >= 0 => n.to_string(),
                    // Anything else as written, which `env::named` refuses
                    t => cssparser::ToCss::to_css_string(t),
                };
                match name {
                    None => name = Some(word),
                    Some(_) => indices.push(word),
                }
            }
            Ok::<_, ParseError<'_, ()>>((name, indices))
        })
        .map_err(|_| Fail::Why(refusal::SYNTAX))?;
    let name = name.ok_or(EnvRefusal::UnknownVariable)?;
    let indices: Vec<&str> = indices.iter().map(String::as_str).collect();
    let term = match env::TitleBarVar::from_name(&name) {
        Some(var) if indices.is_empty() => EnvTerm::TitleBar(var),
        _ => match env::named(&name, &indices)? {
            Dimension::Env(edge, _) => EnvTerm::Inset(edge),
            Dimension::Segment(var, x, y, _) => EnvTerm::Segment(var, x, y),
            Dimension::Region(region, edge, _) => EnvTerm::Region(region, edge),
            _ => return Err(Fail::Why(refusal::NOT_A_LENGTH)),
        },
    };
    Ok((term, fallback(p, depth)?))
}

/// `var(--exact-…)`'s parts, as [`env_parts`]: a content owner's value
/// (LLP 1123 D9) and its fallback's tokens, if it has one.
pub(super) fn var_parts(
    p: &mut Parser<'_, '_>,
    depth: usize,
) -> Result<(EnvTerm, Option<String>), Fail> {
    let name = p
        .parse_until_before(Delimiter::Comma, |p| {
            let name = p.expect_ident()?.to_string();
            p.expect_exhausted()?;
            Ok::<_, ParseError<'_, ()>>(name)
        })
        .map_err(|_| Fail::Why(refusal::SYNTAX))?;
    let var = crate::style::OwnerVar::from_name(&name).ok_or(Fail::Why(refusal::OWNER_NAME))?;
    Ok((EnvTerm::Owner(var), fallback(p, depth)?))
}

/// What follows a variable's name: nothing, or a comma and its fallback,
/// whose tokens are returned as written.
fn fallback(p: &mut Parser<'_, '_>, depth: usize) -> Result<Option<String>, Fail> {
    match p.next() {
        Err(_) => Ok(None),
        Ok(Token::Comma) => {
            let start = p.position();
            declaration_value(p, depth, true)?;
            let tokens = p
                .slice_from(start)
                .trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
            Ok(Some(tokens.to_string()))
        }
        Ok(_) => Err(Fail::Why(refusal::SYNTAX)),
    }
}

/// Read a `<declaration-value>` to the end of its block (CSS Syntax 3
/// §7.2): no bad string or URL, no closing bracket without its opener, and
/// at the top no `;` or `!`; nested at most [`MAX_DEPTH`] deep, so the work
/// is bounded. Empty is one too. A `rem` or `em` length is refused: a
/// fallback is read where no font size is known.
fn declaration_value(p: &mut Parser<'_, '_>, depth: usize, top: bool) -> Result<(), Fail> {
    let syntax = || Fail::Why(refusal::SYNTAX);
    while let Ok(token) = p.next_including_whitespace_and_comments() {
        match token {
            Token::Dimension { unit, .. }
                if unit.eq_ignore_ascii_case("rem") || unit.eq_ignore_ascii_case("em") =>
            {
                return Err(Fail::Why(refusal::RELATIVE_FALLBACK))
            }
            Token::Semicolon | Token::Delim('!') if top => return Err(syntax()),
            Token::BadString(_)
            | Token::BadUrl(_)
            | Token::CloseParenthesis
            | Token::CloseSquareBracket
            | Token::CloseCurlyBracket => return Err(syntax()),
            // A fallback's own `env()` or `var()` names a variable too
            Token::Function(name)
                if name.eq_ignore_ascii_case("env") || name.eq_ignore_ascii_case("var") =>
            {
                if depth == MAX_DEPTH {
                    return Err(syntax());
                }
                let parts = if name.eq_ignore_ascii_case("env") {
                    env_parts
                } else {
                    var_parts
                };
                p.parse_nested_block(|p| {
                    parts(p, depth + 1)
                        .map(|_| ())
                        .map_err(|f| p.new_custom_error(f))
                })
                .map_err(fail)?;
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                if depth == MAX_DEPTH {
                    return Err(syntax());
                }
                p.parse_nested_block(|p| {
                    declaration_value(p, depth + 1, false).map_err(|f| p.new_custom_error(f))
                })
                .map_err(fail)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Skip white space, comments among it; whether there was white space,
/// which a comment is not.
fn space(p: &mut Parser<'_, '_>) -> bool {
    let mut white = false;
    loop {
        let before = p.state();
        if matches!(p.next_including_whitespace(), Ok(Token::WhiteSpace(_))) {
            white = true;
        } else {
            p.reset(&before);
            return white;
        }
    }
}

/// One item alone, or the node `many` makes of several.
fn one_or(mut items: Vec<Node>, many: impl FnOnce(Vec<Node>) -> Node) -> Node {
    if items.len() == 1 {
        items.pop().expect("one item")
    } else {
        many(items)
    }
}

/// A numeric token's value from its source text, as an `f64`.
fn number_of(text: &str) -> f64 {
    number(text.trim_start_matches(['\t', '\n', '\u{c}', '\r', ' '])).map_or(f64::NAN, |(n, _)| n)
}

/// A CSS `<number>` at the start of `text` and where it ends: an optional
/// sign, digits with an optional fraction (a `.` needs digits after it) or a
/// fraction alone, and an optional exponent. One past what an `f64` holds is
/// the closest it does (CSS Values 4 §5).
fn number(text: &str) -> Option<(f64, usize)> {
    let b = text.as_bytes();
    let digits = |mut k: usize| {
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        k
    };
    let sign = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let mut end = digits(sign);
    if b.get(end) == Some(&b'.') {
        let fraction = digits(end + 1);
        if fraction == end + 1 {
            return None;
        }
        end = fraction;
    } else if end == sign {
        return None;
    }
    if matches!(b.get(end), Some(b'e' | b'E')) {
        let at = end + 1 + usize::from(matches!(b.get(end + 1), Some(b'+' | b'-')));
        if b.get(at).is_some_and(u8::is_ascii_digit) {
            end = digits(at);
        }
    }
    // A zero as written is the unsigned zero, `-0` too (§10.9.2)
    let n = exact_num::parse_f64(&text[..end]).ok()? + 0.0;
    Some((n.clamp(f64::MIN, f64::MAX), end))
}

/// Whether CSS gives the row no negative values: a negative literal there is
/// invalid, and a negative math function is clamped to zero (CSS Values 4
/// §9.1).
pub fn nonnegative_row(style: crate::StyleId) -> bool {
    use crate::StyleId::*;
    matches!(
        style,
        Width
            | Height
            | MinWidth
            | MinHeight
            | MaxWidth
            | MaxHeight
            | FlexBasis
            | ColumnWidth
            | PaddingTop
            | PaddingRight
            | PaddingBottom
            | PaddingLeft
            | FlexGrow
            | FlexShrink
            | RowGap
            | ColumnGap
            | ColumnRuleWidth
            | BorderWidthTop
            | BorderWidthRight
            | BorderWidthBottom
            | BorderWidthLeft
    )
}

/// A dimension's sign checked for its row: a negative literal on a row with
/// no negative values is invalid (§5.1); a math function's result is
/// clamped to the row's range instead, never invalid (§9.1), at once where
/// it is a number and where it is used otherwise.
pub(crate) fn signed(
    style: crate::StyleId,
    value: Dimension,
    math: bool,
) -> Result<Dimension, crate::StyleValueError> {
    use crate::StyleId::*;
    let negative = matches!(
        value,
        Dimension::Points(n) | Dimension::Percent(n) | Dimension::Viewport(_, n) if n < 0.0
    );
    if negative && math && super::no_negative_lengths(style) {
        return Ok(match value {
            Dimension::Percent(_) => Dimension::Percent(0.0),
            Dimension::Viewport(unit, _) => Dimension::Viewport(unit, 0.0),
            _ => Dimension::Points(0.0),
        });
    }
    if negative && !math && nonnegative_row(style) {
        return Err(crate::StyleValueError::WrongKind {
            style,
            expected: "nonnegative length or percentage (a negative calc() clamps to 0)",
        });
    }
    let radius = matches!(
        style,
        BorderRadiusTopLeft
            | BorderRadiusTopRight
            | BorderRadiusBottomRight
            | BorderRadiusBottomLeft
    );
    if radius && (!value.is_finite() || negative) {
        return Err(crate::StyleValueError::WrongKind {
            style,
            expected: "nonnegative finite length or percentage",
        });
    }
    Ok(value)
}
