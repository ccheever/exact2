//! Code consistency for observed suggestions, not a proof (LLP 1151 D6, D9).

mod values;

use contract_syntax::{
    Component, Expanded, File, ModuleKind, Owner, PropertyDecl, PropertyForm, PropertyKind,
    PropertyTest, Stmt,
};
use std::collections::{BTreeMap, BTreeSet};

/// The code's disposition of one observed property block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionCheck {
    /// The walk found no contradicting or unknown write.
    Keep,
    /// Replace an observed frame with this finite static write set.
    Widen(Vec<String>),
    /// Do not print the block; the rule and reason belong in the receipt.
    Drop {
        /// The suggestion form: frame, set, stays or whole.
        rule: &'static str,
        /// The write or analysis limit that prevents the suggestion.
        reason: String,
    },
}

/// Check candidates against the expanded program, including all callers and branches.
/// The loaded file retains module-event entries removed from the view.
/// This function reads no observations, files or host state.
pub fn check_suggestions(
    expanded: &Expanded,
    file: &File,
    properties: &[PropertyDecl],
) -> Vec<SuggestionCheck> {
    let root = &expanded.root;
    let slots: BTreeSet<_> = root
        .states
        .iter()
        .zip(&expanded.owners)
        .filter(|(_, owner)| !matches!(owner, Owner::Arm { .. }))
        .map(|(s, _)| s.name.clone())
        .chain(root.mutations.iter().map(|m| m.name.clone()))
        .collect();
    let mut writes = Writes {
        root,
        slots,
        commands: contract_syntax::HOST_COMMANDS
            .iter()
            .map(|s| s.to_string())
            .chain(
                file.modules
                    .iter()
                    .filter(|m| m.kind == ModuleKind::Send)
                    .map(|m| m.name.clone()),
            )
            .collect(),
        active: BTreeSet::new(),
        cache: BTreeMap::new(),
    };
    let facts = values::Facts::new(root, file);
    properties
        .iter()
        .map(|p| {
            let mut result = SuggestionCheck::Keep;
            for line in &p.lines {
                let (rule, reason) = match &line.form {
                    PropertyForm::OnlyWrites(observed) => {
                        let found = match &p.kind {
                            PropertyKind::After(a) => writes.action(a),
                            PropertyKind::Clock => writes.clock(),
                            _ => Err("a frame needs an action or the clock".into()),
                        };
                        match found {
                            Ok(names) => {
                                let seen: BTreeSet<_> = observed.iter().cloned().collect();
                                if !seen.is_subset(&names) {
                                    (
                                        "frame",
                                        Some("observed writes exceed the static write set".into()),
                                    )
                                } else if names.iter().any(|n| !identifier(n)) {
                                    (
                                        "frame",
                                        Some("a static write has no property identifier".into()),
                                    )
                                } else {
                                    if names != seen {
                                        result =
                                            SuggestionCheck::Widen(names.into_iter().collect());
                                    }
                                    ("frame", None)
                                }
                            }
                            Err(why) => ("frame", Some(why)),
                        }
                    }
                    PropertyForm::Hold { name, test } => match test {
                        PropertyTest::In(vs) => ("set", facts.check(name, Some(vs))),
                        PropertyTest::Whole => ("whole", facts.check(name, None)),
                        PropertyTest::Stays(v) => ("stays", facts.latch(name, v)),
                        _ => ("set", Some("this predicate has no suggestion walk".into())),
                    },
                    PropertyForm::Command { .. } => {
                        ("frame", Some("commands have no suggestion walk".into()))
                    }
                };
                if let Some(reason) = reason {
                    return SuggestionCheck::Drop { rule, reason };
                }
            }
            result
        })
        .collect()
}

fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
}

type WriteSet = Result<BTreeSet<String>, String>;

struct Writes<'a> {
    root: &'a Component,
    slots: BTreeSet<String>,
    commands: BTreeSet<String>,
    active: BTreeSet<String>,
    cache: BTreeMap<String, WriteSet>,
}
impl Writes<'_> {
    fn action(&mut self, name: &str) -> WriteSet {
        if let Some(found) = self.cache.get(name) {
            return found.clone();
        }
        if !self.active.insert(name.into()) {
            return Err(format!(
                "call/send cycle through `{name}` may write any slot"
            ));
        }
        let found = if let Some(a) = self.root.actions.iter().find(|a| a.name == name) {
            self.body(&a.body)
        } else {
            Err(format!("unknown action `{name}` may write any slot"))
        };
        self.active.remove(name);
        self.cache.insert(name.into(), found.clone());
        found
    }

    fn body(&mut self, body: &[Stmt]) -> WriteSet {
        let mut out = BTreeSet::new();
        for stmt in body {
            match stmt {
                Stmt::Assign { target, .. } => {
                    if self.slots.contains(target) {
                        out.insert(target.clone());
                    }
                }
                Stmt::Send { target, .. } => {
                    if self.slots.contains(target) {
                        out.insert(target.clone());
                    }
                    let Some(m) = self.root.mutations.iter().find(|m| m.name == *target) else {
                        return Err(format!("unknown send `{target}` may write any slot"));
                    };
                    if let Some((then, _)) = &m.then {
                        out.extend(self.action(then)?);
                    }
                }
                Stmt::Call { action, .. } => out.extend(self.action(action)?),
                Stmt::If {
                    then, otherwise, ..
                } => {
                    out.extend(self.body(then)?);
                    out.extend(self.body(otherwise)?);
                }
                Stmt::Match { some, none, .. } => {
                    out.extend(self.body(&some.1)?);
                    out.extend(self.body(none)?);
                }
                Stmt::Command { name, .. } => {
                    if self.root.actions.iter().any(|a| a.name == *name) {
                        out.extend(self.action(name)?);
                    } else if !self.commands.contains(name) {
                        return Err(format!("unknown call `{name}` may write any slot"));
                    }
                }
                Stmt::Let { .. } | Stmt::Refresh { .. } => {}
            }
        }
        Ok(out)
    }

    fn clock(&mut self) -> WriteSet {
        let mut out = BTreeSet::new();
        for task in &self.root.tasks {
            out.extend(self.action(&task.timer.1)?);
        }
        for sequence in &self.root.sequences {
            if let Some((ended, _)) = &sequence.ended {
                out.extend(self.action(ended)?);
            }
        }
        // A standalone land can run continuations armed by an earlier input;
        // a queue can send its next request during that same due-work step.
        for m in &self.root.mutations {
            if m.queue && self.slots.contains(&m.name) {
                out.insert(m.name.clone());
            }
            if let Some((then, _)) = &m.then {
                out.extend(self.action(then)?);
            }
        }
        Ok(out)
    }
}
