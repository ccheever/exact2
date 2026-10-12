//! `state … persist` (LLP 1116 D5): which states a launch may keep.
//!
//! A persisted state is a small setting the host keeps across launches —
//! a number, string or bool, or an option or list of those — and only the
//! root's: a child's state lives as long as its instance. Everything else
//! is refused here, each refusal naming its repair.

use crate::{Sink, Ty, TypeError, Types};
use contract_syntax::{Expanded, File, Owner};

fn scalar(t: &Ty) -> bool {
    matches!(t, Ty::Number | Ty::String | Ty::Bool)
}

const KEEPS: &str =
    "`persist` keeps a small setting: a number, string or bool, or an option or list of those";

const SOURCE: &str = "keep app data in a source (a resource your data module answers and a \
                      mutation that saves it)";

/// Why a state of type `t` cannot be persisted, or `None` when it can.
fn refusal(state: &str, t: &Ty) -> Option<String> {
    let holds_action = |t: &Ty| match t {
        Ty::Action(_) => true,
        Ty::Option(e) | Ty::List(e) => matches!(**e, Ty::Action(_)),
        _ => false,
    };
    Some(match t {
        t if scalar(t) => return None,
        Ty::Option(e) | Ty::List(e) if scalar(e) => return None,
        // `type-cannot-infer` already refuses a type nothing completes.
        t if !t.is_complete() => return None,
        t if holds_action(t) => format!(
            "`{state}` holds an action, which is code, not a value a launch can keep: persist \
             the setting that chooses it (a string or a number) and pick the action from that"
        ),
        Ty::Record(shape) => format!(
            "`{state}` holds a record (`{shape}`): {SOURCE}, or persist each setting the record \
             holds as a state of its own. {KEEPS}"
        ),
        Ty::List(e) if matches!(**e, Ty::Record(_)) => {
            format!("`{state}` is a list of records (`{e}`), which is app data: {SOURCE}. {KEEPS}")
        }
        Ty::Unit => format!("`{state}` is `unit`: there is nothing to keep"),
        t => format!(
            "`{state}` is `{t}`: persist its parts as states of their own, or {SOURCE}. {KEEPS}"
        ),
    })
}

/// Refuse every `persist` that is not a root state of a type a launch can
/// keep.
pub(crate) fn check(file: &File, types: &Types, expanded: &Expanded, sink: &mut Sink) {
    let root = &file.components[0].name;
    for child in file.components.iter().skip(1) {
        for p in &child.persist {
            sink.push(TypeError {
                id: "type-persist-child",
                message: format!(
                    "only the root's state may say `persist`: `{child}` is a child, whose state \
                     lives as long as each of its instances. Keep the setting in `{root}` as \
                     `state {name} = … persist` and pass it to `{child}` as a prop",
                    child = child.name,
                    name = p.name,
                ),
                span: p.span,
            });
        }
    }
    let Some(ct) = types.components.first() else {
        return;
    };
    for p in &expanded.root.persist {
        let slot = expanded
            .root
            .states
            .iter()
            .zip(&expanded.owners)
            .position(|(s, o)| s.name == p.name && *o == Owner::Root);
        let Some(t) = slot.and_then(|i| ct.slots.get(i)) else {
            continue;
        };
        if let Some(message) = refusal(&p.name, t) {
            sink.push(TypeError {
                id: "type-persist-type",
                message,
                span: p.span,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_settings_are_kept_and_the_rest_name_a_repair() {
        let b = |t: Ty| Box::new(t);
        for t in [
            Ty::Number,
            Ty::String,
            Ty::Bool,
            Ty::Option(b(Ty::Number)),
            Ty::List(b(Ty::String)),
        ] {
            assert_eq!(refusal("s", &t), None, "{t}");
        }
        assert_eq!(
            refusal("s", &Ty::Option(b(Ty::Unknown))),
            None,
            "type-cannot-infer's"
        );
        let record = Ty::Record("Item".into());
        for (t, says) in [
            (record.clone(), "keep app data in a source"),
            (Ty::List(b(record)), "which is app data"),
            (Ty::Action(vec![]), "holds an action"),
            (Ty::List(b(Ty::List(b(Ty::Number)))), "parts as states"),
            (Ty::Unit, "nothing to keep"),
        ] {
            let why = refusal("s", &t).expect("refused");
            assert!(why.contains(says), "{t}: {why}");
        }
    }
}
