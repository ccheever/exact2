//! A task's schedule and, for a gated task (LLP 1092 D7, D9), its gate and
//! key: `when` takes a bool, and a key is a string, number or bool, as a
//! `for` key is.

use super::{infer, Scope, Shapes, Sink, Ty, TypeError};
use contract_syntax::{Component, Expr};

pub(crate) mod sequences;

/// `type-child-resource`'s message for `child`, which declares a resource,
/// a mutation or a task, under the root `root`. A child's task most often
/// means "when this sheet or route opens": the root's gated task does that,
/// keyed on the route (the euxy port's icon picker).
pub(crate) fn child_owns(root: &str, child: &Component) -> String {
    let name = &child.name;
    if child.resources.is_empty() && child.mutations.is_empty() {
        return format!(
            "only the root, the root file's first component (`{root}`), may declare a `task`: a timer is the app's; `{name}` is a child, which may own state, derives, and actions. Move the task into `{root}`, gated on what shows `{name}` and keyed on what opens it again, its action one of `{root}`'s: to act when a sheet or route opens, `task reveal when top(nav).name == \"icons\" key=top(nav).id` with `after(1, reveal)` under it. Or, if `{name}` is the app, move it above `{root}`"
        );
    }
    format!(
        "only the root, the root file's first component (`{root}`), may declare a `resource`, `mutation`, or `task`; `{name}` is a child, which may own state, derives, and actions. Move this declaration into `{root}` and pass what `{name}` needs as props; or, if `{name}` is the app, move it above `{root}`"
    )
}

/// Check every task of `c` in the component's scope.
pub(crate) fn check_tasks(c: &Component, scope: &Scope, shapes: &Shapes, sink: &mut Sink) {
    for t in &c.tasks {
        match infer(&t.timer.0, scope, shapes) {
            Ok(Ty::Number) => {}
            Ok(_) => sink.push(TypeError {
                id: "type-timer",
                message: "a task needs a number of milliseconds".into(),
                span: t.timer.2,
            }),
            Err(e) => sink.push(e),
        }
        check_gate(t.gate.as_ref(), t.key.as_ref(), "task", scope, shapes, sink);
    }
}

/// A gate takes a bool, and a key is a string, number or bool, as a `for`
/// key is: a task's, and a sequence's (LLP 1125 D2).
pub(crate) fn check_gate(
    gate: Option<&Expr>,
    key: Option<&Expr>,
    what: &str,
    scope: &Scope,
    shapes: &Shapes,
    sink: &mut Sink,
) {
    if let Some(gate) = gate {
        match infer(gate, scope, shapes) {
            Ok(Ty::Bool | Ty::Unknown) => {}
            Ok(ty) => sink.push(TypeError {
                id: "type-task-gate",
                message: gate_message(gate, &ty),
                span: gate.span(),
            }),
            Err(e) => sink.push(e),
        }
    }
    if let Some(key) = key {
        match infer(key, scope, shapes) {
            Ok(Ty::String | Ty::Enum(_) | Ty::Number | Ty::Bool | Ty::Unknown) => {}
            Ok(ty) => sink.push(TypeError {
                id: "type-task-key",
                message: format!(
                    "a {what}'s `key=` is a string, number or bool, as a `for` key is; this one is {ty}"
                ),
                span: key.span(),
            }),
            Err(e) => sink.push(e),
        }
    }
}

/// "`when` takes a bool; `toast` is a string; write `toast != ""`".
fn gate_message(gate: &Expr, ty: &Ty) -> String {
    let (what, fix) = match gate {
        Expr::Ident(name, _) => (
            format!("`{name}` is a {ty}"),
            match ty {
                Ty::String | Ty::Enum(_) => format!("; write `{name} != \"\"`"),
                Ty::Number => format!("; write `{name} != 0`"),
                Ty::List(_) => format!("; write `{name}.length > 0`"),
                _ => String::new(),
            },
        ),
        _ => (format!("this condition is a {ty}"), String::new()),
    };
    format!("`when` takes a bool; {what}{fix}")
}
