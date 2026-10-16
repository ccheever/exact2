//! The binding core (LLP 1121 D12, stage 5): a design token by its name,
//! resolved in this session for a place. Code outside a plan (a native
//! module's view, a hatch's object, a host screen) holds a token's name and
//! kind, never a row's index, so a plan that reloads, updates or takes a
//! live batch keeps answering. A name the plan lacks is said once and the
//! caller keeps its fallback.

use super::{DataSource, Runner};
use exact_kernel::{ColorValue, StyleId, StyleProps, ViewId};

/// Where a binding resolves: a node, whose app tuple is the window's under
/// the nearest `-exact-mode` pin at or above it, or the session's root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    /// A host screen outside any node: the window's tuple.
    Root,
    /// A node's view (a native module's, a hatch's).
    Node(ViewId),
}

/// The session's tokens for bindings ([`Runner::tamber`]).
pub struct Tamber<'a, D: DataSource> {
    runner: &'a mut Runner<D>,
}

impl<D: DataSource> Runner<D> {
    /// This session's design tokens, by name, for code outside the plan
    /// (LLP 1121 D12): `session.tamber()`.
    pub fn tamber(&mut self) -> Tamber<'_, D> {
        Tamber { runner: self }
    }
}

impl<D: DataSource> Tamber<'_, D> {
    /// The app tuple at `place`.
    pub fn tuple(&self, place: Place) -> u32 {
        let kernel = self.runner.kernel();
        let mut at = match place {
            Place::Node(id) => kernel.node(id),
            Place::Root => None,
        };
        while let Some(node) = at {
            // A pin's row holds the tuple it makes, plus one (bridge.rs).
            if node.style.mask.has(StyleId::Mode) && node.style.mode > 0 {
                return node.style.mode - 1;
            }
            at = node.parent.and_then(|p| kernel.node(p));
        }
        self.runner.ids.theme.window
    }

    /// Whether the plan has token `name`.
    pub fn has(&self, name: &str) -> bool {
        let plan = &self.runner.plan;
        plan.theme.iter().any(|t| plan.str(t.name) == name)
    }

    /// Colour token `name` at `place`: what a row naming it there reads (a
    /// colour, a `light-dark()` pair, a role the host resolves), with the
    /// live table's edits and the session's role overrides. `None` for a
    /// name the plan lacks, said once in the journal; the caller keeps its
    /// fallback.
    pub fn color(&mut self, name: &str, place: Place) -> Option<ColorValue> {
        if !self.has(name) {
            self.missing(name);
            return None;
        }
        let tuple = self.tuple(place);
        let runner = &*self.runner;
        let _roles = runner.bind_roles();
        let before = crate::bridge::set_tuple(tuple);
        let mut patch = StyleProps::default();
        let set = crate::bridge::set_plan_style(
            &mut patch,
            StyleId::TextColor as u16,
            &exact_plan::Value::str(name),
            &runner.plan,
        );
        exact_kernel::style::theme::take_named();
        crate::bridge::set_tuple(before);
        set.ok().map(|_| patch.text_color)
    }

    /// What a binding at `place` reads besides the token's name, as one
    /// number that moves when any of it does: its app tuple (`setMode`, a
    /// pin above), the design (a live batch, its role overrides), the
    /// scheme and contrast, and the root font size. A host compares it after
    /// each batch to tell its observers (`observe`).
    pub fn inputs(&self, place: Place) -> u64 {
        use std::hash::{Hash, Hasher};
        let runner = &*self.runner;
        let preferences = runner.viewport().preferences;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.tuple(place).hash(&mut h);
        runner.design().hash(&mut h);
        runner.roles().generation().hash(&mut h);
        preferences.dark.hash(&mut h);
        format!("{:?}", preferences.contrast).hash(&mut h);
        runner.root_font_size().to_bits().hash(&mut h);
        h.finish()
    }

    fn missing(&mut self, name: &str) {
        if !self.runner.missing_tokens.iter().any(|n| n == name) {
            self.runner.missing_tokens.push(name.to_string());
            self.runner.log(format!(
                "tamber: no token `{name}` in this plan: its binding keeps its fallback (LLP 1121 D12)"
            ));
        }
    }
}
