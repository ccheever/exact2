//! State parts (LLP 1121 D8, stage 8 A): a node's or its class's
//! `:disabled`, `:checked`, `:active`, `:hover` and `:focus-visible`
//! blocks, carried as attributes named for the state and the row
//! (`:disabled background-color`, `contract_syntax::state`).
//!
//! - A runner-known state (`:disabled`, `:checked`) folds into the rows an
//!   author can write today: per longhand, `disabled ? D : (checked ? C : B)`,
//!   where B is the base the class and the node compose (a `border-color` its
//!   four sides, a side a `border` writes its component), so a state row
//!   overrides each side the base wrote, longhand or not.
//! - `:active` lowers to the press paint (LLP 1061 D7, D8 as amended
//!   2026-10-18): its `background-color`, `color` and `-exact-press-scale` to
//!   the node's `-exact-press-` rows, its `opacity` to the press's absolute
//!   target (`-exact-press-opacity-to`, lowering's alone), each row keeping
//!   its `:active` span; the host's one press shows them.
//! - `:hover` and `:focus-visible` are recorded for the hosts to apply on
//!   their own tracking: their rows by longhand ([`Folded::looks`]).
//! - A host-known state's rows are literal only (a token is a literal).
//! - Paint rows only, never on a native control or a list's row.

use super::*;
use contract_syntax::state::state_of;
use contract_syntax::BinOp;

/// The rows a runner-known state takes, as written (a `border-color` is its
/// sides').
const RUNNER_ROWS: [&str; 6] = [
    "background-color",
    "color",
    "border-color",
    "opacity",
    "box-shadow",
    "-exact-tint-color",
];

/// The rows `:hover` and `:focus-visible` take: none that inherits.
const HOST_ROWS: [&str; 4] = ["background-color", "border-color", "opacity", "box-shadow"];

/// The rows `:active` takes, and the press paint's row each lowers to: the
/// press's colour reaches the subtree, so `color` is one (LLP 1061 D7).
const ACTIVE_ROWS: [(&str, &str); 4] = [
    ("background-color", "-exact-press-background-color"),
    ("color", "-exact-press-color"),
    ("opacity", PRESS_OPACITY_TO),
    ("-exact-press-scale", "-exact-press-scale"),
];

/// The press's absolute opacity, lowering's row for `:active`'s `opacity`
/// (never an author's: `lower-unknown-attr`).
pub(crate) const PRESS_OPACITY_TO: &str = "-exact-press-opacity-to";

/// The four sides' colour rows, top, right, bottom, left.
const SIDES: [&str; 4] = [
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
];

/// A node's attributes with its state parts folded.
pub(crate) struct Folded {
    /// The base rows, then each runner-known state's folded longhand.
    pub attrs: Vec<Attr>,
    /// Each host-known state's rows by longhand, the class's then the
    /// node's, in the order written: `(state, rows)`.
    pub looks: Vec<(String, Vec<Attr>)>,
    /// The author's own disabled look: a `:disabled` part, or what the
    /// heuristic reads in the base (never a fold's own rows).
    pub disabled_look: bool,
    /// How many of `attrs` are the sheet's and class's base rows.
    pub class_len: usize,
    /// How many of `attrs` are base rows; each after is a fold's, and
    /// `by_class` says whether its class wrote it.
    pub base_len: usize,
    pub by_class: Vec<bool>,
}

/// Whether a row `name` (as written) is one `state` takes, a side's colour
/// as `border-color`.
fn admitted(name: &str, state: &str) -> bool {
    let name = if SIDES.contains(&name) {
        "border-color"
    } else {
        name
    };
    match state {
        "disabled" | "checked" => RUNNER_ROWS.contains(&name),
        "active" => ACTIVE_ROWS.iter().any(|(r, _)| *r == name),
        _ => HOST_ROWS.contains(&name),
    }
}

/// `row`'s value split into its longhands: a `border-color` its four
/// sides' (a computed list refused, as on the node), anything else itself.
fn longhands(row: &Attr) -> Result<Vec<(String, Expr)>, LowerError> {
    if row.name != "border-color" {
        return Ok(vec![(row.name.clone(), row.value.clone())]);
    }
    let sides =
        values::sides("border-color", &row.value)?.unwrap_or_else(|| vec![row.value.clone(); 4]);
    Ok(SIDES
        .iter()
        .zip(sides)
        .map(|(s, v)| (s.to_string(), v))
        .collect())
}

/// The base's value for longhand `l`: the last of `base` that writes it,
/// a shorthand's part for it (`border-color`, `border`, `border-top` …);
/// `None` where none does.
fn base_of(base: &[Attr], l: &str) -> Result<Option<Expr>, LowerError> {
    let mut out = None;
    for a in base {
        if a.name == l {
            out = Some(a.value.clone());
            continue;
        }
        let Some(side) = SIDES.iter().position(|s| *s == l) else {
            continue;
        };
        let edge = ["border-top", "border-right", "border-bottom", "border-left"][side];
        match a.name.as_str() {
            "border-color" => {
                let sides = values::sides("border-color", &a.value)?;
                out = Some(sides.map_or_else(|| a.value.clone(), |s| s[side].clone()));
            }
            "border" | "border-top" | "border-right" | "border-bottom" | "border-left"
                if a.name == "border" || a.name == edge =>
            {
                use exact_kernel::StyleId::*;
                let colour = [
                    BorderColorTop,
                    BorderColorRight,
                    BorderColorBottom,
                    BorderColorLeft,
                ][side];
                if let Some(index) = shorthands::rows(&a.name).iter().position(|r| *r == colour) {
                    out = Some(shorthands::component(&a.value, &a.name, index)?);
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// `value` with each arm that is no value (`none`, a conditional class's
/// side without the row) taken by `under`: a state that does not write a
/// row in some arm leaves the base's. Where `under` reads the name a
/// `match`'s `some` arm binds, it is bound once beside the `match` under a
/// name no source spells, so the arm's own binding hides it not.
fn over(value: Expr, under: &Expr) -> Expr {
    match value {
        Expr::None(_) => under.clone(),
        Expr::Ternary(c, a, b, span) => Expr::Ternary(
            c,
            Box::new(over(*a, under)),
            Box::new(over(*b, under)),
            span,
        ),
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let mut reads = Vec::new();
            crate::disabled_look::free_reads(under, &mut Vec::new(), &mut reads);
            if reads.contains(&var) {
                let base = Expr::Ident(BASE.into(), span);
                return Expr::Let {
                    name: BASE.into(),
                    value: Box::new(under.clone()),
                    body: Box::new(Expr::Match {
                        subject,
                        var,
                        some: Box::new(over(*some, &base)),
                        none: Box::new(over(*none, &base)),
                        span,
                    }),
                    span,
                };
            }
            Expr::Match {
                subject,
                var,
                some: Box::new(over(*some, under)),
                none: Box::new(over(*none, under)),
                span,
            }
        }
        other => other,
    }
}

/// The name a state's base is bound to where a `match` arm would hide it.
const BASE: &str = "$state-base";

/// Whether `e` names a metric the compiler writes anywhere in it (a grouped
/// list's ground), which no author's row may.
fn metric_in(e: &Expr) -> bool {
    match e {
        Expr::Str(t, _) => exact_kernel::style::metrics::named(t).is_some(),
        Expr::Ternary(_, a, b, _) => metric_in(a) || metric_in(b),
        Expr::Match { some, none, .. } => metric_in(some) || metric_in(none),
        Expr::Let { body, .. } => metric_in(body),
        _ => false,
    }
}

/// Whether `e` is a choice with no value somewhere in it.
fn unset_in(e: &Expr) -> bool {
    match e {
        Expr::None(_) => true,
        Expr::Ternary(_, a, b, _) => unset_in(a) || unset_in(b),
        Expr::Match { some, none, .. } => unset_in(some) || unset_in(none),
        _ => false,
    }
}

/// `value` as the cases it chooses between, each with the condition beside
/// `condition` that picks it, in order; false where a `match` holds an arm
/// with no value, which no chain can say.
fn cases(
    value: &Expr,
    condition: Option<Expr>,
    span: Span,
    out: &mut Vec<(Option<Expr>, Expr)>,
) -> bool {
    let and = |a: &Option<Expr>, b: Expr| match a {
        Some(a) => Expr::Binary(BinOp::And, Box::new(a.clone()), Box::new(b), span),
        None => b,
    };
    match value {
        Expr::None(_) => true,
        Expr::Ternary(c, a, b, _) => {
            let not = Expr::Unary(contract_syntax::UnOp::Not, c.clone(), span);
            cases(a, Some(and(&condition, (**c).clone())), span, out)
                && cases(b, Some(and(&condition, not)), span, out)
        }
        e if unset_in(e) => false,
        e => {
            out.push((condition, e.clone()));
            true
        }
    }
}

/// `condition ? value : under`, a state's row over its base, written as a
/// node's row may be: an arm of `value` that is no value is `under`'s, and
/// where `under` is none too the choice is a chain whose last arm alone is
/// none (`busy && on ? 0.5 : none`), the one place a row takes no value.
fn choose(condition: Expr, value: Expr, under: Expr, span: Span) -> Result<Expr, LowerError> {
    if !matches!(under, Expr::None(_)) || !unset_in(&value) {
        return Ok(Expr::Ternary(
            Box::new(condition),
            Box::new(over(value, &under)),
            Box::new(under),
            span,
        ));
    }
    let mut chosen = Vec::new();
    if !cases(&value, Some(condition), span, &mut chosen) {
        return err(
            "lower-state-row",
            "a state's row chooses no value in a `match` arm, and the node writes no base row for it to keep: write the row on the node too, or give each arm a value",
            value.span(),
        );
    }
    Ok(chosen
        .into_iter()
        .rev()
        .fold(under, |rest, (c, v)| match c {
            Some(c) => Expr::Ternary(Box::new(c), Box::new(v), Box::new(rest), span),
            None => v,
        }))
}

/// The CSS name of a row a state takes, for a tag's fixed row.
pub(crate) fn row_name(id: exact_kernel::StyleId) -> Option<&'static str> {
    use exact_kernel::StyleId::*;
    Some(match id {
        TextColor => "color",
        BackgroundColor => "background-color",
        Opacity => "opacity",
        BoxShadow => "box-shadow",
        TintColor => "-exact-tint-color",
        PressScale => "-exact-press-scale",
        BorderColorTop => SIDES[0],
        BorderColorRight => SIDES[1],
        BorderColorBottom => SIDES[2],
        BorderColorLeft => SIDES[3],
        _ => return None,
    })
}

impl Lowerer<'_> {
    /// Fold `expanded`'s state parts; `None` when it has none. `target` is
    /// why the node takes none (a native control, a list's row); `implied`
    /// the rows the node takes when its author writes none (a link's text
    /// colour), a base a state's row falls back to; `class_len` how many of
    /// `expanded` are its sheet's and class's.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fold_states(
        &mut self,
        expanded: &[Attr],
        target: Option<&str>,
        implied: &[(&str, Expr)],
        class_len: usize,
        scope: &Scope,
        locals: u16,
    ) -> Result<Option<Folded>, LowerError> {
        // Each part's row, and whether its class (or sheet) wrote it.
        let parts: Vec<(&str, Attr, bool)> = expanded
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                let (state, row) = state_of(&a.name)?;
                let row = Attr {
                    name: row.to_string(),
                    ..a.clone()
                };
                Some((state, row, i < class_len))
            })
            .collect();
        let Some((_, first, _)) = parts.first() else {
            return Ok(None);
        };
        if let Some(why) = target {
            return err(
                "lower-state-target",
                format!("a state part on {why}: its states are the platform's (LLP 1121 D8); style it with `accent-color`, or a box of your own"),
                first.span,
            );
        }
        let base: Vec<Attr> = expanded
            .iter()
            .filter(|a| state_of(&a.name).is_none())
            .cloned()
            .collect();
        for (state, row, _) in &parts {
            let host = !matches!(*state, "disabled" | "checked");
            if row.name == "border-color" {
                let fns = &self.fns;
                values::computed_list(&row.name, &row.value, &|f| fns.get(f).map(|(_, b)| *b))?;
            }
            if !admitted(&row.name, state) {
                let rows: Vec<&str> = match *state {
                    "disabled" | "checked" => RUNNER_ROWS.to_vec(),
                    "active" => ACTIVE_ROWS.iter().map(|(r, _)| *r).collect(),
                    _ => HOST_ROWS.to_vec(),
                };
                let why = match (*state, row.name.as_str()) {
                    ("active", "border-color" | "box-shadow") => " and is the host's press, whose paint draws no border or shadow yet (LLP 1061 D7)".to_string(),
                    (_, "-exact-press-scale") => " and a press scale is `:active`'s".to_string(),
                    ("active", _) => " and is the host's press (LLP 1061 D7)".to_string(),
                    _ if host => " and never changes what a child inherits".to_string(),
                    _ => String::new(),
                };
                return err(
                    "lower-state-row",
                    format!(
                        "`{}` in `:{state}`: a state part takes paint rows only, so a state never lays out{why}; `:{state}` takes {}",
                        row.name,
                        rows.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(", ")
                    ),
                    row.span,
                );
            }
            if host && !matches!(row.value, Expr::Str(..) | Expr::Number(..) | Expr::Bool(..)) {
                return err(
                    "lower-state-literal",
                    format!("`{}` in `:{state}` is computed: the host applies its state on its own, with no round trip to the runner, so its rows are literals (a token's name is one)", row.name),
                    row.value.span(),
                );
            }
        }
        // `:active` is the press D7's pressed paint shows on (theme2's 05:28
        // ruling (3)): the same paint said twice is refused, naming both.
        for (state, row, _) in parts.iter().filter(|(s, ..)| *s == "active") {
            let paint = match row.name.as_str() {
                "opacity" => "-exact-press-opacity",
                name => ACTIVE_ROWS
                    .iter()
                    .find(|(r, _)| *r == name)
                    .map_or("", |(_, p)| *p),
            };
            if base.iter().any(|a| a.name == paint) {
                return err(
                    "lower-state-press-paint",
                    format!("`{}` in `:{state}` and `{paint}` both say the press's look: keep one spelling (LLP 1121 D8, LLP 1061 D7)", row.name),
                    row.span,
                );
            }
        }
        // The conditions, from what the node writes; a state it never
        // writes never holds (`lower-state-dead`, a lint).
        let written = |name: &str| {
            base.iter()
                .rev()
                .find(|a| a.name == name)
                .map(|a| a.value.clone())
        };
        let span = first.span;
        let disabled = written("disabled");
        let mut checked: Option<Expr> = None;
        for name in ["checked", "aria-checked", "aria-selected", "aria-pressed"] {
            let Some(v) = written(name) else { continue };
            let v = match (name, &v) {
                ("aria-pressed", Expr::Str(s, sp)) => Expr::Bool(s == "true", *sp),
                ("aria-pressed", Expr::Bool(..)) => v,
                ("aria-pressed", _) => {
                    let mut asm = Asm::new();
                    let mut locals = locals;
                    match expr::compile(self, &mut asm, &v, scope, &mut locals)? {
                        Ty::Bool => v,
                        _ => Expr::Binary(
                            BinOp::Eq,
                            Box::new(v.clone()),
                            Box::new(Expr::Str("true".into(), v.span())),
                            v.span(),
                        ),
                    }
                }
                _ => v,
            };
            checked = Some(match checked {
                Some(c) => Expr::Binary(BinOp::Or, Box::new(c), Box::new(v), span),
                None => v,
            });
        }
        // Each folded longhand: its value, where, written, and whether its
        // class wrote every state row in it.
        let mut folded: Vec<(String, Expr, Span, bool, bool)> = Vec::new();
        let mut looks: Vec<(String, Vec<Attr>)> = Vec::new();
        for (state, row, _) in &parts {
            if !matches!(*state, "disabled" | "checked" | "active") {
                let rows = match looks.iter_mut().find(|(s, _)| s == state) {
                    Some((_, rows)) => rows,
                    None => {
                        looks.push((state.to_string(), Vec::new()));
                        &mut looks.last_mut().expect("just pushed").1
                    }
                };
                for (l, v) in longhands(row)? {
                    rows.retain(|r| r.name != l);
                    rows.push(Attr {
                        name: l,
                        value: v,
                        ..row.clone()
                    });
                }
            }
        }
        // `:checked` under `:disabled`: each state's rows in their order,
        // the class's then the node's.
        for state in ["checked", "disabled"] {
            let condition = match state {
                "disabled" => disabled.clone(),
                _ => checked.clone(),
            };
            let Some(condition) = condition else { continue };
            for (_, row, class) in parts.iter().filter(|(s, ..)| *s == state) {
                for (l, v) in longhands(row)? {
                    let (under, by_class) = match folded.iter().rposition(|(n, ..)| *n == l) {
                        Some(i) => {
                            let f = folded.remove(i);
                            (f.1, f.4 && *class)
                        }
                        // The base's, else what the node takes unwritten (a
                        // link's colour), else no row.
                        None => {
                            let base = base_of(&base, &l)?
                                .or_else(|| {
                                    implied
                                        .iter()
                                        .rfind(|(n, _)| *n == l)
                                        .map(|(_, e)| e.clone())
                                })
                                .unwrap_or(Expr::None(row.span));
                            (base, *class)
                        }
                    };
                    // The author's row owns what it folds, unless its base is
                    // a metric the compiler writes, which no author may (a
                    // grouped list's ground): then the fold stays implied.
                    let metric = metric_in(&under);
                    let value = choose(condition.clone(), v, under, row.span)?;
                    folded.push((l, value, row.span, row.written && !metric, by_class));
                }
            }
        }
        let disabled_look = parts.iter().any(|(s, ..)| *s == "disabled");
        let base_styled = disabled_look::styled(&base);
        let class_len = expanded[..class_len.min(expanded.len())]
            .iter()
            .filter(|a| state_of(&a.name).is_none())
            .count();
        let mut attrs = base;
        let base_len = attrs.len();
        let mut by_class = Vec::new();
        for (name, value, span, written, class) in folded {
            attrs.push(Attr {
                name,
                value,
                span,
                written,
            });
            by_class.push(class);
        }
        // `:active`'s rows as the press paint's, the class's then the
        // node's (the node's wins), each at its `:active` row's span.
        let pressed = attrs.len();
        for (_, row, class) in parts.iter().filter(|(s, ..)| *s == "active") {
            let (_, paint) = ACTIVE_ROWS
                .iter()
                .find(|(r, _)| *r == row.name)
                .expect("admitted");
            if let Some(at) = (pressed..attrs.len()).find(|i| attrs[*i].name == *paint) {
                attrs.remove(at);
                by_class.remove(at - base_len);
            }
            attrs.push(Attr {
                name: paint.to_string(),
                ..row.clone()
            });
            by_class.push(*class);
        }
        Ok(Some(Folded {
            attrs,
            looks,
            disabled_look: disabled_look || base_styled,
            class_len,
            base_len,
            by_class,
        }))
    }
}
