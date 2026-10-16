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

/// A generated constant (`contract tamber gen rust`, LLP 1121 D12): a
/// token's name and kind, which a session resolves by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<K> {
    /// The token's name as the design writes it.
    pub name: &'static str,
    kind: std::marker::PhantomData<K>,
}

impl<K> Token<K> {
    /// The token named `name`.
    pub const fn new(name: &'static str) -> Token<K> {
        Token {
            name,
            kind: std::marker::PhantomData,
        }
    }
}

/// A colour token's kind: [`Tamber::get`] answers its value as a row reads
/// it. (Lengths, numbers, fonts and symbols come with stage 6.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color;

/// A token kind, and what a token of it resolves to.
pub trait Kind {
    /// The value.
    type Value;
    /// `name`'s value at `place`, as [`Tamber::color`] reads a colour.
    fn get<D: DataSource>(
        tamber: &mut Tamber<'_, D>,
        name: &str,
        place: Place,
    ) -> Option<Self::Value>;
}

impl Kind for Color {
    type Value = ColorValue;
    fn get<D: DataSource>(
        tamber: &mut Tamber<'_, D>,
        name: &str,
        place: Place,
    ) -> Option<ColorValue> {
        tamber.color(name, place)
    }
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

    /// Generated constant `token` at `place` (`SURFACE_CARD`): its value,
    /// or `None` when the plan lacks it, said once ([`Tamber::color`]).
    pub fn get<K: Kind>(&mut self, token: Token<K>, place: Place) -> Option<K::Value> {
        K::get(self, token.name, place)
    }

    /// Whether the plan has token `name`.
    pub fn has(&self, name: &str) -> bool {
        self.row_name(name).is_some()
    }

    /// The plan's name for token `name`: itself, or a role override's as the
    /// design wrote it (`secondary-label`) when `name` is the role's spelling
    /// (`-exact-secondary-label`, as the IR and the generators name it).
    fn row_name(&self, name: &str) -> Option<String> {
        let plan = &self.runner.plan;
        let row = plan
            .theme
            .iter()
            .find(|t| plan.str(t.name) == name)
            .or_else(|| {
                plan.theme
                    .iter()
                    .find(|t| !plan.str(t.role).is_empty() && plan.str(t.role) == name)
            })?;
        Some(plan.str(row.name).to_string())
    }

    /// Colour token `name` at `place`: what a row naming it there reads (a
    /// colour, a `light-dark()` pair, a role the host resolves), with the
    /// live table's edits and the session's role overrides. `None` for a
    /// name the plan lacks, or a value its row would refuse (a live edit
    /// that does not parse), each said once in the journal; the caller
    /// keeps its fallback.
    pub fn color(&mut self, name: &str, place: Place) -> Option<ColorValue> {
        let Some(row) = self.row_name(name) else {
            self.say_once(name, "no token `{name}` in this plan");
            return None;
        };
        let tuple = self.tuple(place);
        let runner = &*self.runner;
        let _roles = runner.bind_roles();
        let before = crate::bridge::set_tuple(tuple);
        let mut patch = StyleProps::default();
        // Read aside: what the runner's own parses recorded stays theirs.
        let (set, _) = exact_kernel::style::theme::recorded(|| {
            crate::bridge::set_plan_style(
                &mut patch,
                StyleId::TextColor as u16,
                &exact_plan::Value::str(&row),
                &runner.plan,
            )
        });
        crate::bridge::set_tuple(before);
        if set.is_err() {
            self.say_once(name, "`{name}`'s value is no colour a row takes");
            return None;
        }
        Some(patch.text_color)
    }

    /// Whether `place` draws dark: the nearest `color-scheme` at or above
    /// it, else the window's.
    pub fn dark(&self, place: Place) -> bool {
        let scheme = match place {
            Place::Node(id) => self
                .runner
                .kernel()
                .node(id)
                .and_then(|n| n.color_scheme_dark()),
            Place::Root => None,
        };
        scheme.unwrap_or(self.runner.viewport().preferences.dark)
    }

    /// What a binding at `place` reads besides the token's name, as one
    /// number that moves when any of it does: its app tuple (`setMode`, a
    /// pin above), the design's values (a token edit, a live batch, a role
    /// override's), the platform's colours a role reads (a system accent),
    /// its scheme (the window's, a `color-scheme` above), the contrast, and
    /// the root font size. A host compares it after each batch
    /// to tell its observers (`observe`); it costs no walk of the design.
    pub fn inputs(&self, place: Place) -> u64 {
        use std::hash::{Hash, Hasher};
        let runner = &*self.runner;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.tuple(place).hash(&mut h);
        runner.design_moves.hash(&mut h);
        // The session's role overrides and the platform's reported colours
        // (a system accent moved), as a host compares them to re-present.
        {
            let _roles = runner.bind_roles();
            exact_kernel::style::roles::generation().hash(&mut h);
        }
        self.dark(place).hash(&mut h);
        format!("{:?}", runner.viewport().preferences.contrast).hash(&mut h);
        runner.root_font_size().to_bits().hash(&mut h);
        h.finish()
    }

    /// Say `why` (its `{name}` the token's) once for this runner.
    fn say_once(&mut self, name: &str, why: &str) {
        let line = format!(
            "tamber: {}: its binding keeps its fallback (LLP 1121 D12)",
            why.replace("{name}", name)
        );
        if !self.runner.tamber_said.contains(&line) {
            self.runner.tamber_said.push(line.clone());
            self.runner.log(line);
        }
    }
}
