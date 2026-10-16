//! `lower-resource-launch-identity` (LLP 1027.005 D6): an identifying
//! argument of a resource that is this launch's own — the `epochAtZero` or
//! `seed` of an `exactTime()` resource, through any derive or state that
//! holds it or the resource. It differs at every launch, so the answer a
//! store-reading resource kept at the last launch never matches it, and its
//! first frame never shows that answer (LLP 1027 D4); after `with` it still
//! reaches the source but does not identify the answer. A field of that
//! name on anything else (a saved recording's `epochAtZero`) is the app's
//! own value, and a time made from the epoch (`epochAtZero +
//! performanceNow()`) may be what a fetch is about: neither is named. The
//! runner says the second at the first answer it keeps under one.

use crate::LowerError;
use contract_syntax::{Component, Expr, ResourceDecl};
use std::collections::BTreeSet;

/// The fields of `exactTime()` that differ at every launch.
const OF_LAUNCH: &[&str] = &["epochAtZero", "seed"];

/// What the root's names hold, as far as the launch goes.
#[derive(PartialEq)]
struct Names {
    /// An `exactTime()` resource, or a name holding one.
    clocks: BTreeSet<String>,
    /// A launch's own value, or a name holding one.
    launch: BTreeSet<String>,
}

impl Names {
    fn of(root: &Component) -> Names {
        let mut names = Names {
            clocks: root
                .resources
                .iter()
                .filter(|r| r.source == "exactTime")
                .map(|r| r.name.clone())
                .collect(),
            launch: BTreeSet::new(),
        };
        // Each round adds the bindings the last one made known; the
        // bindings bound the rounds.
        for _ in 0..=root.states.len() + root.derives.len() {
            let mut next = Names {
                clocks: names.clocks.clone(),
                launch: names.launch.clone(),
            };
            for b in root.states.iter().chain(&root.derives) {
                if names.clock(&b.expr) {
                    next.clocks.insert(b.name.clone());
                }
                if names.of_launch(&b.expr) {
                    next.launch.insert(b.name.clone());
                }
            }
            if next == names {
                break;
            }
            names = next;
        }
        names
    }

    fn clock(&self, e: &Expr) -> bool {
        match e {
            Expr::Ident(n, _) => self.clocks.contains(n),
            Expr::Typed(inner, _, _) => self.clock(inner),
            _ => false,
        }
    }

    fn of_launch(&self, e: &Expr) -> bool {
        match e {
            Expr::Member(inner, field, _) => {
                OF_LAUNCH.contains(&field.as_str()) && self.clock(inner)
            }
            Expr::Ident(n, _) => self.launch.contains(n),
            Expr::Typed(inner, _, _) => self.of_launch(inner),
            _ => false,
        }
    }
}

pub(super) fn launch_identity(root: &Component, out: &mut Vec<LowerError>) {
    let names = Names::of(root);
    for r in &root.resources {
        let identity = &r.args[..r.identity.unwrap_or(r.args.len())];
        for (k, arg) in identity.iter().enumerate() {
            if names.of_launch(arg) {
                out.push(LowerError {
                    id: "lower-resource-launch-identity",
                    message: message(r, identity, k),
                    span: arg.span(),
                });
            }
        }
    }
}

/// The warning for `r`'s identifying argument `k`, with the declaration it
/// would be. The source receives the call's arguments and then the ones
/// after `with`, in order: an argument moved from the end of the call keeps
/// its place, one moved from before another changes the source's
/// parameters too.
fn message(r: &ResourceDecl, identity: &[Expr], k: usize) -> String {
    let said = written(&identity[k]);
    let rest: Vec<String> = identity
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != k)
        .map(|(_, e)| written(e))
        .collect();
    let context: Vec<String> = std::iter::once(said.clone())
        .chain(r.args[identity.len()..].iter().map(written))
        .collect();
    let declaration = format!(
        "resource {} = {}({}) with {}",
        r.name,
        r.source,
        rest.join(", "),
        context.join(", ")
    );
    let order = if k + 1 == identity.len() {
        "the source receives it where it did".to_string()
    } else {
        format!(
            "the source then receives it after the call's other arguments, so change `{}`'s parameters to that order too",
            r.source
        )
    };
    format!(
        "`{name}`'s kept answer is never shown at launch because `{said}` changes every launch: a store-reading resource's first frame shows the answer kept at the last launch only when its identifying arguments match (LLP 1027 D4). Pass it after `with` (`{declaration}`), which still reaches the source but is not counted; {order}",
        name = r.name,
    )
}

/// An argument as written, for the declaration the warning suggests:
/// names, members, literals and calls of them; anything else `…`.
fn written(e: &Expr) -> String {
    match e {
        Expr::Member(inner, field, _) => format!("{}.{field}", written(inner)),
        Expr::Ident(n, _) => n.clone(),
        Expr::Number(n, _) => n.to_string(),
        Expr::Str(s, _) => format!("{s:?}"),
        Expr::Bool(b, _) => b.to_string(),
        Expr::Call(name, args, _) => format!(
            "{name}({})",
            args.iter().map(written).collect::<Vec<_>>().join(", ")
        ),
        Expr::Typed(inner, _, _) => written(inner),
        _ => "…".into(),
    }
}
