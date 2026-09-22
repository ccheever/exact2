//! Substitution is capture-free: over generated expressions whose binders
//! and free names share a small pool, substituting then evaluating agrees
//! with evaluating each replacement first and binding its value.

use super::subst_expr;
use crate::ast::{BinOp, Expr};
use crate::Span;
use std::collections::BTreeMap;

const NAMES: [&str; 4] = ["a", "b", "o", "p"];

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
    fn name(&mut self) -> String {
        NAMES[self.below(NAMES.len() as u64) as usize].to_owned()
    }
}

fn expr(rng: &mut Rng, depth: u32) -> Expr {
    let s = Span::default();
    let pick = if depth == 0 {
        rng.below(3)
    } else {
        rng.below(8)
    };
    match pick {
        0 => Expr::Number(rng.below(4) as f64, s),
        1 | 2 => Expr::Ident(rng.name(), s),
        3 => Expr::Some(Box::new(expr(rng, depth - 1)), s),
        4 => Expr::None(s),
        5 => Expr::Binary(
            BinOp::Add,
            Box::new(expr(rng, depth - 1)),
            Box::new(expr(rng, depth - 1)),
            s,
        ),
        6 => Expr::Ternary(
            Box::new(Expr::Binary(
                BinOp::Eq,
                Box::new(expr(rng, depth - 1)),
                Box::new(expr(rng, depth - 1)),
                s,
            )),
            Box::new(expr(rng, depth - 1)),
            Box::new(expr(rng, depth - 1)),
            s,
        ),
        _ => Expr::Match {
            subject: Box::new(expr(rng, depth - 1)),
            var: rng.name(),
            some: Box::new(expr(rng, depth - 1)),
            none: Box::new(expr(rng, depth - 1)),
            span: s,
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
enum V {
    Num(f64),
    Opt(Option<Box<V>>),
    /// A type error or an unbound name: poisons whatever reads it.
    Wrong,
}

fn eval(e: &Expr, env: &BTreeMap<String, V>) -> V {
    match e {
        Expr::Number(n, _) => V::Num(*n),
        Expr::Ident(n, _) => env.get(n).cloned().unwrap_or(V::Wrong),
        Expr::Some(x, _) => match eval(x, env) {
            V::Wrong => V::Wrong,
            v => V::Opt(Some(Box::new(v))),
        },
        Expr::None(_) => V::Opt(None),
        Expr::Binary(BinOp::Add, a, b, _) => match (eval(a, env), eval(b, env)) {
            (V::Num(a), V::Num(b)) => V::Num(a + b),
            _ => V::Wrong,
        },
        Expr::Ternary(c, a, b, _) => {
            let Expr::Binary(BinOp::Eq, l, r, _) = &**c else {
                unreachable!()
            };
            match (eval(l, env), eval(r, env)) {
                (V::Wrong, _) | (_, V::Wrong) => V::Wrong,
                (l, r) if l == r => eval(a, env),
                _ => eval(b, env),
            }
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => match eval(subject, env) {
            V::Opt(Some(v)) => {
                let mut inner = env.clone();
                inner.insert(var.clone(), *v);
                eval(some, &inner)
            }
            V::Opt(None) => eval(none, env),
            _ => V::Wrong,
        },
        other => unreachable!("not generated: {other:?}"),
    }
}

fn value(rng: &mut Rng) -> V {
    match rng.below(3) {
        0 => V::Num(rng.below(4) as f64),
        1 => V::Opt(None),
        _ => V::Opt(Some(Box::new(V::Num(rng.below(4) as f64)))),
    }
}

#[test]
fn substitution_never_captures_a_replacements_free_name() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut renamed = 0;
    for _ in 0..20_000 {
        let e = expr(&mut rng, 4);
        let mut subst = BTreeMap::new();
        for name in NAMES {
            if rng.below(2) == 0 {
                subst.insert(name.to_owned(), expr(&mut rng, 2));
            }
        }
        let env: BTreeMap<String, V> = NAMES
            .iter()
            .map(|n| (n.to_string(), value(&mut rng)))
            .collect();
        let mut bound = env.clone();
        for (name, replacement) in &subst {
            bound.insert(name.clone(), eval(replacement, &env));
        }
        let substituted = subst_expr(&e, &subst);
        renamed += format!("{substituted:?}").contains('@') as u32;
        assert_eq!(
            eval(&substituted, &env),
            eval(&e, &bound),
            "\n{e:?}\nunder {subst:?}\nbecame {substituted:?}"
        );
    }
    // The generator does produce captures for the renaming to avoid.
    assert!(renamed > 1_000, "{renamed} renamed binders");
}
