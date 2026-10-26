//! The root's `tool` lines (LLP 1111 D2) as the plan's `tools` table, and
//! the checks of D3 and D4a over each tool's reach: its action, the actions
//! that action calls, and the `then` of each mutation they send into.
//!
//! - **`tool-root`**: a tool names a root action, so only the root
//!   declares one. **`tool-duplicate`**, **`tool-action`**: one tool a
//!   name, naming an action of the root.
//! - **`tool-condition-type`**: `fails when` and `available when` are bool.
//! - **`tool-command`**: a command in the reach that D4a neither skips nor
//!   runs acts in the person's name, waits for them or reaches outside the
//!   app. **`tool-reads-command`**: a `reads` tool runs only
//!   `preventDefault` and `stopPropagation` of the run class.

use crate::error::{err_one, LowerError};
use crate::Lowerer;
use contract_syntax::tool::{Consequence, Tool};
use contract_syntax::{Component, File, Span, Stmt};
use contract_types::Ty;

/// The commands a call's commit does not run: each moves the person's view
/// (LLP 1111 D4a; the runner's `modules/calls.rs` skips them).
pub const SKIPPED: &[&str] = &[
    "focus",
    "blur",
    "scrollIntoView",
    "selectText",
    "setSelectionRange",
    "fastSeek",
    "load",
    "haptic",
];
/// The commands it runs as a press would.
pub const RUN: &[&str] = &[
    "postMessage",
    "stopSounds",
    "preventDefault",
    "stopPropagation",
];

impl Lowerer<'_> {
    /// Each root `tool` line as a row of the plan's `tools` (module doc).
    pub(crate) fn tools(&mut self, file: &File) -> Result<(), Vec<LowerError>> {
        for c in file.components.iter().skip(1) {
            if let Some(t) = c.tools.first() {
                return Err(err_one(
                    "tool-root",
                    format!(
                        "`tool {}` is in component `{}`: a tool names a root action, so only the root component declares one (LLP 1111 D2)",
                        t.name, c.name
                    ),
                    t.span,
                ));
            }
        }
        let root = self.root;
        for (k, t) in root.tools.iter().enumerate() {
            if root.tools[..k].iter().any(|o| o.name == t.name) {
                return Err(err_one(
                    "tool-duplicate",
                    format!("`tool {}` is declared twice", t.name),
                    t.span,
                ));
            }
            let Some(a) = root.actions.iter().position(|x| x.name == t.action.0) else {
                let names: Vec<&str> = root.actions.iter().map(|x| x.name.as_str()).collect();
                return Err(err_one(
                    "tool-action",
                    format!(
                        "`tool {}` runs `{}`, which is not an action of the root component; its actions: {}",
                        t.name,
                        t.action.0,
                        names.join(", ")
                    ),
                    t.action.1,
                ));
            };
            let derive = |clause: &str| {
                let name = Tool::derive(&t.name, clause);
                root.derives.iter().position(|d| d.name == name)
            };
            for clause in ["fails", "available"] {
                let Some(i) = derive(clause) else { continue };
                let ty = &self.types.components[0].derives[i];
                if *ty != Ty::Bool {
                    return Err(err_one(
                        "tool-condition-type",
                        format!(
                            "`{clause} when` of `tool {}` is a condition, a bool; this is {ty:?}",
                            t.name
                        ),
                        t.span,
                    ));
                }
            }
            let mut commands = Vec::new();
            reach(root, a, &mut vec![false; root.actions.len()], &mut commands);
            for (name, span) in commands {
                if SKIPPED.contains(&name.as_str()) || RUN.contains(&name.as_str()) {
                    let pure = matches!(name.as_str(), "preventDefault" | "stopPropagation");
                    if t.consequence == Consequence::Reads && RUN.contains(&name.as_str()) && !pure
                    {
                        return Err(err_one(
                            "tool-reads-command",
                            format!(
                                "`tool {}` says it `reads`, and its action reaches `{name}`, which acts: say `writes`",
                                t.name
                            ),
                            span,
                        ));
                    }
                    continue;
                }
                return Err(err_one(
                    "tool-command",
                    format!(
                        "`tool {}` reaches `{name}`, which acts in the person's name, waits for them or reaches outside the app, and a caller is not the person (LLP 1111 D4a): write the tool's own action without it",
                        t.name
                    ),
                    span,
                ));
            }
            let at = |d: Option<usize>| d.map(|i| self.derives[i]);
            let (returns, fails, available) = (
                at(derive("returns")),
                at(derive("fails")),
                at(derive("available")),
            );
            self.b.tool(
                &t.name,
                t.consequence.word(),
                [t.additive, t.idempotent, t.closed],
                &t.description,
                self.actions[a],
                [returns, fails, available],
            );
        }
        Ok(())
    }
}

/// The commands action `a`'s reach names (module doc), each with its span.
fn reach(root: &Component, a: usize, seen: &mut Vec<bool>, out: &mut Vec<(String, Span)>) {
    if std::mem::replace(&mut seen[a], true) {
        return;
    }
    let mut next = Vec::new();
    walk(root, &root.actions[a].body, &mut next, out);
    for b in next {
        reach(root, b, seen, out);
    }
}

fn walk(root: &Component, body: &[Stmt], next: &mut Vec<usize>, out: &mut Vec<(String, Span)>) {
    let action = |name: &str| root.actions.iter().position(|x| x.name == name);
    for s in body {
        match s {
            Stmt::Command { name, span, .. } => out.push((name.clone(), *span)),
            Stmt::Send { target, .. } => {
                let then = (root.mutations.iter().find(|m| &m.name == target))
                    .and_then(|m| m.then.as_ref())
                    .and_then(|(name, _)| action(name));
                next.extend(then);
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                walk(root, then, next, out);
                walk(root, otherwise, next, out);
            }
            Stmt::Match { some, none, .. } => {
                walk(root, &some.1, next, out);
                walk(root, none, next, out);
            }
            Stmt::Call {
                action: name, body, ..
            } => {
                next.extend(action(name));
                walk(root, body, next, out);
            }
            Stmt::Let { .. } | Stmt::Assign { .. } | Stmt::Refresh { .. } => {}
        }
    }
}
