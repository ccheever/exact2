//! A keyframe's value through palette functions (LLP 1062 D9).
//!
//! A keyframe is constant (LLP 1055 D5), and a palette is written once: a
//! keyframe value may call a function of literal arguments
//! (`color=accent()`, `color=tone("strong", 0.4)`), which folds here to the
//! literal the plan's `@keyframes` rule carries.

use contract_syntax::{Expr, FnDecl, TemplatePart};

/// A keyframe's value as literal text (a number as CSS writes it), folded
/// through calls to functions of literal arguments; `None` when anything
/// along the way is not known when the app compiles.
pub(crate) fn constant(e: &Expr, fns: &[FnDecl]) -> Option<String> {
    text(fold(e, fns, &[], 0)?)
}

/// A value a keyframe knows when the app compiles.
#[derive(Debug, Clone, PartialEq)]
enum Constant {
    Str(String),
    Number(f64),
    Bool(bool),
}

/// Bound depth: a palette function may call another, not recurse forever.
const FOLD_DEPTH: usize = 16;

/// A keyframe's value as written, through calls to functions of literal
/// arguments (`color=accent()`, `color=tone("strong", 0.4)`, where each `fn`
/// is a palette of `light-dark()` strings, conditions and templates): a
/// palette is written once, and a keyframe is still known when the app
/// compiles. `None` when anything along the way is not.
fn fold(e: &Expr, fns: &[FnDecl], env: &[(String, Constant)], depth: usize) -> Option<Constant> {
    use contract_syntax::{BinOp, UnOp};
    let at = |e: &Expr| fold(e, fns, env, depth);
    Some(match e {
        Expr::Str(s, _) => Constant::Str(s.clone()),
        Expr::Number(n, _) => Constant::Number(*n),
        Expr::Bool(b, _) => Constant::Bool(*b),
        Expr::Ident(name, _) => env.iter().rev().find(|(n, _)| n == name)?.1.clone(),
        Expr::Template(parts, _) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    TemplatePart::Text(t) => out.push_str(t),
                    TemplatePart::Expr(x) => out.push_str(&text(at(x)?)?),
                }
            }
            Constant::Str(out)
        }
        Expr::Ternary(c, yes, no, _) => match at(c)? {
            Constant::Bool(true) => at(yes)?,
            Constant::Bool(false) => at(no)?,
            _ => return None,
        },
        Expr::Let {
            name, value, body, ..
        } => {
            let mut inner = env.to_vec();
            inner.push((name.clone(), at(value)?));
            fold(body, fns, &inner, depth)?
        }
        Expr::Unary(op, x, _) => match (op, at(x)?) {
            (UnOp::Neg, Constant::Number(n)) => Constant::Number(-n),
            (UnOp::Not, Constant::Bool(b)) => Constant::Bool(!b),
            _ => return None,
        },
        Expr::Binary(op, a, b, _) => {
            let (a, b) = (at(a)?, at(b)?);
            match (op, a, b) {
                (BinOp::Add, Constant::Str(a), Constant::Str(b)) => Constant::Str(a + &b),
                (BinOp::Eq, a, b) => Constant::Bool(a == b),
                (BinOp::Ne, a, b) => Constant::Bool(a != b),
                (BinOp::And, Constant::Bool(a), Constant::Bool(b)) => Constant::Bool(a && b),
                (BinOp::Or, Constant::Bool(a), Constant::Bool(b)) => Constant::Bool(a || b),
                (op, Constant::Number(a), Constant::Number(b)) => match op {
                    BinOp::Add => Constant::Number(a + b),
                    BinOp::Sub => Constant::Number(a - b),
                    BinOp::Mul => Constant::Number(a * b),
                    BinOp::Div => Constant::Number(a / b),
                    BinOp::Rem => Constant::Number(a % b),
                    BinOp::Lt => Constant::Bool(a < b),
                    BinOp::Le => Constant::Bool(a <= b),
                    BinOp::Gt => Constant::Bool(a > b),
                    BinOp::Ge => Constant::Bool(a >= b),
                    _ => return None,
                },
                _ => return None,
            }
        }
        Expr::Call(name, args, _) if depth < FOLD_DEPTH => {
            let f = fns.iter().find(|f| &f.name == name)?;
            if f.params.len() != args.len() {
                return None;
            }
            let bound = f
                .params
                .iter()
                .zip(args)
                .map(|(p, a)| Some((p.name.clone(), at(a)?)))
                .collect::<Option<Vec<_>>>()?;
            fold(&f.body, fns, &bound, depth + 1)?
        }
        _ => return None,
    })
}

/// A constant as a template writes it: the runner's `toString`, for the
/// numbers it writes as plain decimals.
fn text(c: Constant) -> Option<String> {
    Some(match c {
        Constant::Str(s) => s,
        Constant::Bool(b) => b.to_string(),
        Constant::Number(0.0) => "0".into(),
        Constant::Number(n) if n.is_finite() && (1e-6..1e21).contains(&n.abs()) => {
            exact_num::Shortest(n).to_string()
        }
        Constant::Number(_) => return None,
    })
}
