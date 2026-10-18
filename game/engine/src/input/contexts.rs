//! Input contexts and rebinding: world state the tick changes, which input
//! reads from the next tick. Both are resources, so they save and hash; a
//! world that never uses them has neither.
use super::{pad, Action, Actions, Input};
use crate::{Data, Resource, World};

/// One action's keys after a rebind.
#[derive(Clone, Debug, Default, PartialEq, Eq, Data)]
pub struct Binding {
    /// The declared action.
    pub action: String,
    /// Its keys and pad buttons, replacing the declared ones.
    pub keys: Vec<String>,
}
/// The active input contexts, sorted. Change them with `World::input_context`.
#[derive(Clone, Debug, Default, PartialEq, Resource)]
pub struct InputContexts {
    /// Context names.
    pub active: Vec<String>,
}
/// The actions this world rebound, sorted by action. Change them with
/// `World::rebind`.
#[derive(Clone, Debug, Default, PartialEq, Resource)]
pub struct Bindings {
    /// One per rebound action.
    pub actions: Vec<Binding>,
}
impl Bindings {
    /// Parse `action key…` entries separated by newlines or `;`, the form a
    /// settings screen stores and posts: `jump GamepadNorth; fire KeyF MouseLeft`.
    pub fn parse(text: &str) -> Result<Vec<Binding>, String> {
        let mut out = Vec::new();
        for entry in text.split(['\n', ';']) {
            let mut words = entry.split_whitespace();
            let Some(action) = words.next() else {
                continue;
            };
            let keys: Vec<String> = words.map(str::to_owned).collect();
            if let Some(code) = keys
                .iter()
                .find(|k| k.starts_with("Gamepad") && pad::pad_index(k).is_none())
            {
                return Err(format!(
                    "binding `{action}`: unknown pad button `{code}`; pad buttons: {}",
                    pad::PAD_BUTTONS.join(", ")
                ));
            }
            out.push(Binding {
                action: action.into(),
                keys,
            });
        }
        Ok(out)
    }
}
impl World {
    /// Turn an input context on or off. Its actions read from the next tick:
    /// turning it off releases the ones held then, and turning it on reads a
    /// key already down only after it lifts and goes down again.
    pub fn input_context(&mut self, name: &str, on: bool) {
        if self.input_context_active(name) == on {
            return;
        }
        if self.try_resource::<InputContexts>().is_none() {
            self.insert_resource(InputContexts::default());
        }
        {
            let mut contexts = self.resource_mut::<InputContexts>();
            match contexts.active.binary_search_by(|c| c.as_str().cmp(name)) {
                Err(i) if on => contexts.active.insert(i, name.into()),
                Ok(i) if !on => {
                    contexts.active.remove(i);
                }
                _ => {}
            }
        }
        self.log(format_args!(
            "context {name} {}",
            if on { "on" } else { "off" }
        ));
    }
    /// Whether the named input context is active.
    pub fn input_context_active(&self, name: &str) -> bool {
        self.try_resource::<InputContexts>()
            .is_some_and(|c| c.active.iter().any(|n| n == name))
    }
    /// Replace a declared action's keys for this world, from the next tick. An
    /// action held then releases; the new keys take effect on the next press.
    /// A stick's keys are not rebound.
    pub fn rebind(&mut self, action: &str, keys: &[&str]) {
        for code in keys {
            pad::check_code(code);
        }
        let binding = Binding {
            action: action.into(),
            keys: keys.iter().map(|k| (*k).into()).collect(),
        };
        if self.try_resource::<Bindings>().is_none() {
            self.insert_resource(Bindings::default());
        }
        {
            let mut bindings = self.resource_mut::<Bindings>();
            match bindings
                .actions
                .binary_search_by(|b| b.action.as_str().cmp(action))
            {
                Ok(i) => bindings.actions[i] = binding,
                Err(i) => bindings.actions.insert(i, binding),
            }
        }
        self.log(format_args!("bind {action} {}", keys.join(" ")));
    }
    /// Rebind from `Bindings::parse`'s text, as a settings screen posts it.
    pub fn rebind_text(&mut self, text: &str) -> Result<(), String> {
        for b in Bindings::parse(text)? {
            let keys: Vec<&str> = b.keys.iter().map(String::as_str).collect();
            self.rebind(&b.action, &keys);
        }
        Ok(())
    }
}
/// A new game's bindings: the player's stored ones (`Game::bindings`), copied
/// into the fresh world so a replay of it needs nothing outside the save.
pub(crate) fn seed(world: &mut World, actions: &Actions, text: &str) {
    match Bindings::parse(text) {
        Ok(bindings) => {
            for b in bindings {
                if actions.find(&b.action).is_some_and(|a| a.stick.is_none()) {
                    let keys: Vec<&str> = b.keys.iter().map(String::as_str).collect();
                    world.rebind(&b.action, &keys);
                } else {
                    world.log(format_args!(
                        "bind {}: not a declared button; declared actions: {}",
                        b.action,
                        actions.names()
                    ));
                }
            }
        }
        Err(error) => world.log(format_args!("stored bindings refused: {error}")),
    }
}
impl Input {
    /// Bring the world's contexts and bindings in at a tick's start. A context
    /// that went off releases its held actions; an action rebound while held
    /// releases; and an action whose keys are down as it enters a context or
    /// is rebound reads up until they lift. Returns the journal lines.
    pub(crate) fn sync(&mut self, contexts: &[String], bindings: &[Binding]) -> Vec<String> {
        let mut lines = Vec::new();
        if contexts == self.contexts.as_slice() && bindings == self.bindings.as_slice() {
            return lines;
        }
        let Some(actions) = self.actions.clone() else {
            self.contexts = contexts.to_vec();
            self.bindings = bindings.to_vec();
            return lines;
        };
        let before: Vec<(bool, bool, Vec<String>)> = actions
            .entries
            .iter()
            .map(|a| (self.active(a), self.in_context(a), self.keys_of(a).to_vec()))
            .collect();
        self.contexts = contexts.to_vec();
        self.bindings = bindings.to_vec();
        for b in bindings {
            if !actions.find(&b.action).is_some_and(|a| a.stick.is_none()) {
                lines.push(format!(
                    "bind {}: not a declared button, ignored; declared actions: {}",
                    b.action,
                    actions.names()
                ));
            }
        }
        for (a, (was, in_context, keys)) in actions.entries.iter().zip(before) {
            let off = in_context && !self.in_context(a);
            let on = !in_context && self.in_context(a);
            let rebound = keys.as_slice() != self.keys_of(a);
            if off || rebound {
                if was {
                    self.release(&a.name);
                    lines.push(match &a.context {
                        Some(c) if off => format!("context {c} off: released {}", a.name),
                        _ => format!("bind {}: released, rebound while held", a.name),
                    });
                }
                self.reset_trigger(&a.name);
                self.latched.retain(|n| *n != a.name);
            }
            if (on || rebound) && self.latches(a) {
                self.latched.push(a.name.clone());
            }
        }
        lines
    }
    fn latches(&self, a: &Action) -> bool {
        a.stick.is_none() && self.in_context(a) && self.raw(a) && !self.latched.contains(&a.name)
    }
}
