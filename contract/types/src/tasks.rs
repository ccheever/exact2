//! A task's schedule and, for a gated task (LLP 1092 D7, D9), its gate and
//! key: `when` takes a bool, and a key is a string, number or bool, as an
//! `each` key is.

use super::{infer, Scope, Shapes, Sink, Ty, TypeError};
use contract_syntax::{Component, Expr};

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
        if let Some(gate) = &t.gate {
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
        if let Some(key) = &t.key {
            match infer(key, scope, shapes) {
                Ok(Ty::String | Ty::Number | Ty::Bool | Ty::Unknown) => {}
                Ok(ty) => sink.push(TypeError {
                    id: "type-task-key",
                    message: format!(
                        "a task's `key=` is a string, number or bool, as an `each` key is; this one is {ty}"
                    ),
                    span: key.span(),
                }),
                Err(e) => sink.push(e),
            }
        }
    }
}

/// "`when` takes a bool; `toast` is a string; write `toast != ""`".
fn gate_message(gate: &Expr, ty: &Ty) -> String {
    let (what, fix) = match gate {
        Expr::Ident(name, _) => (
            format!("`{name}` is a {ty}"),
            match ty {
                Ty::String => format!("; write `{name} != \"\"`"),
                Ty::Number => format!("; write `{name} != 0`"),
                Ty::List(_) => format!("; write `length({name}) > 0`"),
                _ => String::new(),
            },
        ),
        _ => (format!("this condition is a {ty}"), String::new()),
    };
    format!("`when` takes a bool; {what}{fix}")
}
