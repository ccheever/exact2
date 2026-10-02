//! Substitution is capture-free: over generated expressions whose binders
//! and free names share a small pool, substituting then evaluating agrees
//! with evaluating each replacement first and binding its value.

use super::substituted;
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
        let result = substituted(&e, &subst);
        renamed += format!("{result:?}").contains('@') as u32;
        assert_eq!(
            eval(&result, &env),
            eval(&e, &bound),
            "\n{e:?}\nunder {subst:?}\nbecame {result:?}"
        );
    }
    // The generator does produce captures for the renaming to avoid.
    assert!(renamed > 1_000, "{renamed} renamed binders");
}

/// A derive DAG in source form: each derive reads props, earlier derives,
/// branches, `and`, `match`, and `fail()` (which traps where evaluated).
fn derive_source(rng: &mut Rng, derives: usize) -> String {
    fn expr(rng: &mut Rng, depth: u32, earlier: usize) -> String {
        let pick = if depth == 0 {
            rng.below(4)
        } else {
            rng.below(10)
        };
        let sub = |rng: &mut Rng| expr(rng, depth.saturating_sub(1), earlier);
        match pick {
            0 => rng.below(3).to_string(),
            1 => "p".into(),
            2 if earlier > 0 => format!("d{}", rng.below(earlier as u64)),
            2 | 3 => "fail()".into(),
            4 | 5 => format!("({} + {})", sub(rng), sub(rng)),
            6 => format!(
                "({} == {} ? {} : {})",
                sub(rng),
                sub(rng),
                sub(rng),
                sub(rng)
            ),
            7 => format!("({} == {} and {} == 0)", sub(rng), sub(rng), sub(rng)),
            8 => format!(
                "match o {{ case some(v) => (v + {}), case none => {} }}",
                sub(rng),
                sub(rng)
            ),
            _ => format!(
                "(d{} + d{})",
                rng.below(earlier.max(1) as u64),
                rng.below(earlier.max(1) as u64)
            )
            .replace("d0", if earlier == 0 { "p" } else { "d0" }),
        }
    }
    let mut src = String::from("component C\n  props\n    p: number\n    o: option<number>\n");
    for i in 0..derives {
        src.push_str(&format!("  derive d{i} = {}\n", expr(rng, 3, i)));
    }
    let reads: Vec<String> = (0..derives).map(|i| format!("${{d{i}}}")).collect();
    src + &format!("  view\n    text `{}`\n", reads.join(" "))
}

/// Evaluate with a trap for `fail()` and for any operand a trap produced,
/// reading a derive by evaluating its authored body where it is read (the
/// meaning resolution must keep), or by what a `let` bound.
fn run(e: &Expr, env: &BTreeMap<String, V>, bodies: &BTreeMap<String, Expr>) -> Result<V, ()> {
    let num = |v: V| match v {
        V::Num(n) => Ok(n),
        _ => Err(()),
    };
    Ok(match e {
        Expr::Number(n, _) => V::Num(*n),
        Expr::Ident(n, _) => match env.get(n) {
            Some(v) => v.clone(),
            None => run(bodies.get(n).ok_or(())?, env, bodies)?,
        },
        Expr::Call(..) => return Err(()),
        Expr::Binary(BinOp::Add, a, b, _) => {
            V::Num(num(run(a, env, bodies)?)? + num(run(b, env, bodies)?)?)
        }
        Expr::Binary(BinOp::Eq, a, b, _) => {
            V::Num((run(a, env, bodies)? == run(b, env, bodies)?) as u8 as f64)
        }
        Expr::Binary(BinOp::And, a, b, _) => match run(a, env, bodies)? {
            V::Num(n) if n != 0.0 => run(b, env, bodies)?,
            other => other,
        },
        Expr::Ternary(c, a, b, _) => match run(c, env, bodies)? {
            V::Num(n) if n != 0.0 => run(a, env, bodies)?,
            _ => run(b, env, bodies)?,
        },
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => match run(subject, env, bodies)? {
            V::Opt(Some(v)) => {
                let mut inner = env.clone();
                inner.insert(var.clone(), *v);
                run(some, &inner, bodies)?
            }
            _ => run(none, env, bodies)?,
        },
        Expr::Let {
            name, value, body, ..
        } => {
            let mut inner = env.clone();
            inner.insert(name.clone(), run(value, env, bodies)?);
            run(body, &inner, bodies)?
        }
        other => unreachable!("not generated: {other:?}"),
    })
}

#[test]
fn a_shared_derive_is_evaluated_exactly_where_its_readers_evaluated_it() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let (mut shared, mut traps) = (0, 0);
    for _ in 0..3_000 {
        let derives = 1 + rng.below(5) as usize;
        let src = derive_source(&mut rng, derives);
        let file = crate::parse(&src).unwrap_or_else(|e| panic!("{e}\n{src}"));
        let c = &file.components[0];
        let resolved = super::resolved_derives(c).unwrap();
        let authored: BTreeMap<String, Expr> = c
            .derives
            .iter()
            .map(|d| (d.name.clone(), d.expr.clone()))
            .collect();
        for p in [0.0, 1.0, 2.0] {
            for o in [V::Opt(None), V::Opt(Some(Box::new(V::Num(1.0))))] {
                let env = BTreeMap::from([("p".to_owned(), V::Num(p)), ("o".to_owned(), o)]);
                for (derive, expr) in &resolved {
                    let lazily = run(&derive.expr, &env, &authored);
                    // Every derive a resolved form reads is bound by a `let`.
                    let now = run(expr, &env, &BTreeMap::new());
                    assert_eq!(
                        now, lazily,
                        "\n{src}\n`{}` resolved to {expr:?}",
                        derive.name
                    );
                    traps += lazily.is_err() as u32;
                }
            }
        }
        shared += resolved
            .iter()
            .filter(|(_, e)| format!("{e:?}").contains("Let {"))
            .count() as u32;
    }
    // The generator does share values, and does skip traps.
    assert!(
        shared > 500 && traps > 500,
        "{shared} shared, {traps} traps"
    );
}

/// A statement `let` (LLP 1035.005.000 D2) binds for the rest of its block:
/// a replacement that mentions its name renames it apart there, and the
/// statements before it and after its block keep the outer name.
#[test]
fn a_statement_let_is_renamed_apart_from_a_replacement_for_the_rest_of_its_block() {
    use super::subst::{subst_stmts, Subst};
    use crate::ast::Stmt;
    let s = Span::default();
    let id = |n: &str| Expr::Ident(n.into(), s);
    let assign = |target: &str, expr: Expr| Stmt::Assign {
        target: target.into(),
        expr,
        span: s,
    };
    let add = |a: Expr, b: Expr| Expr::Binary(BinOp::Add, Box::new(a), Box::new(b), s);
    // `x = b; if c { let a = 1; x = a + b }; x = a`, with `b` replaced by the
    // parent's `a`.
    let body = vec![
        assign("x", id("b")),
        Stmt::If {
            cond: id("c"),
            then: vec![
                Stmt::Let {
                    name: "a".into(),
                    expr: Expr::Number(1.0, s),
                    span: s,
                },
                assign("x", add(id("a"), id("b"))),
            ],
            otherwise: Vec::new(),
            span: s,
        },
        assign("x", id("a")),
    ];
    let map = BTreeMap::from([("b".to_owned(), id("a"))]);
    let out = subst_stmts(&body, &mut Subst::new(&map), &BTreeMap::new());
    let Stmt::If { then, .. } = &out[1] else {
        panic!("{out:?}")
    };
    assert_eq!(
        then,
        &vec![
            Stmt::Let {
                name: "a@1".into(),
                expr: Expr::Number(1.0, s),
                span: s,
            },
            assign("x", add(id("a@1"), id("a"))),
        ]
    );
    assert_eq!(out[0], assign("x", id("a")));
    assert_eq!(out[2], assign("x", id("a")));
}
