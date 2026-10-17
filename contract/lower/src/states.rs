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
//! - A host-known state (`:active`, `:hover`, `:focus-visible`) is recorded
//!   for the hosts to apply on their own tracking: its rows, literal only
//!   (a token is a literal), by longhand ([`Folded::looks`]).
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

/// The rows a host-known state takes: none that inherits.
const HOST_ROWS: [&str; 5] = [
    "background-color",
    "border-color",
    "opacity",
    "box-shadow",
    "-exact-press-scale",
];

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
    /// A `:disabled` part was written: the author's own disabled look.
    pub disabled_look: bool,
}

/// Whether a row `name` (as written) is one a state of `host` kind takes,
/// a side's colour as `border-color`.
fn admitted(name: &str, host: bool) -> bool {
    let name = if SIDES.contains(&name) {
        "border-color"
    } else {
        name
    };
    match host {
        true => HOST_ROWS.contains(&name),
        false => RUNNER_ROWS.contains(&name),
    }
}

/// `row`'s value split into its longhands: a `border-color` its four
/// sides', anything else itself.
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

/// The elements a list's `children` make its rows, through its `for`,
/// `if` and `match` regions, each by its span.
pub(crate) fn rows_of(children: &[Node], out: &mut Vec<Span>) {
    for child in children {
        match child {
            Node::When {
                then, otherwise, ..
            } => {
                rows_of(then, out);
                rows_of(otherwise, out);
            }
            Node::Each { body, .. } => rows_of(body, out),
            Node::Match { some, none, .. } => {
                rows_of(&some.1, out);
                rows_of(none, out);
            }
            other => out.push(other.span()),
        }
    }
}

impl Lowerer<'_> {
    /// Fold `expanded`'s state parts; `None` when it has none. `target` is
    /// why the node takes none (a native control, a list's row).
    pub(crate) fn fold_states(
        &mut self,
        expanded: &[Attr],
        target: Option<&str>,
        scope: &Scope,
        locals: u16,
    ) -> Result<Option<Folded>, LowerError> {
        let parts: Vec<(&str, Attr)> = expanded
            .iter()
            .filter_map(|a| {
                let (state, row) = state_of(&a.name)?;
                Some((
                    state,
                    Attr {
                        name: row.to_string(),
                        ..a.clone()
                    },
                ))
            })
            .collect();
        let Some((_, first)) = parts.first() else {
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
        for (state, row) in &parts {
            let host = !matches!(*state, "disabled" | "checked");
            if !admitted(&row.name, host) {
                let rows = if host {
                    &HOST_ROWS[..]
                } else {
                    &RUNNER_ROWS[..]
                };
                return err(
                    "lower-state-row",
                    format!(
                        "`{}` in `:{state}`: a state part takes paint rows only, so a state never lays out{}; `:{state}` takes {}",
                        row.name,
                        if host { " and never changes what a child inherits" } else { "" },
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
        let mut folded: Vec<(String, Expr, Span, bool)> = Vec::new();
        let mut looks: Vec<(String, Vec<Attr>)> = Vec::new();
        for (state, row) in &parts {
            if !matches!(*state, "disabled" | "checked") {
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
            for (_, row) in parts.iter().filter(|(s, _)| *s == state) {
                for (l, v) in longhands(row)? {
                    let under = match folded.iter().rposition(|(n, ..)| *n == l) {
                        Some(i) => folded.remove(i).1,
                        None => base_of(&base, &l)?.unwrap_or(Expr::None(row.span)),
                    };
                    let value = Expr::Ternary(
                        Box::new(condition.clone()),
                        Box::new(v),
                        Box::new(under),
                        row.span,
                    );
                    folded.push((l, value, row.span, row.written));
                }
            }
        }
        let disabled_look = parts.iter().any(|(s, _)| *s == "disabled");
        let mut attrs = base;
        attrs.extend(folded.into_iter().map(|(name, value, span, written)| Attr {
            name,
            value,
            span,
            written,
        }));
        Ok(Some(Folded {
            attrs,
            looks,
            disabled_look,
        }))
    }
}
