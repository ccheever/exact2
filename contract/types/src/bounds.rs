//! Two refusals that bound a value by its type (LLP 1090 D2): an option
//! directly inside an option, and a type nested deeper than a value may.
//!
//! The web's JS target erases `some`, so `some(none)` and `none` are one
//! value there; refusing the type keeps every target counting the same
//! values, and leaves at most one `some` above each value the runner counts.
//! A type's depth bounds its values' (no value has type `?`), so the
//! runner's `MAX_VALUE_DEPTH` becomes a rule every target checks once.

use crate::{err, Shapes, Ty, TypeError};
use contract_syntax::Span;
use std::collections::BTreeMap;

/// The deepest a value may nest: the runner's `MAX_VALUE_DEPTH`.
pub const MAX_TYPE_DEPTH: u32 = 64;

impl Shapes {
    /// How deep a value of `t` can nest: 0 for a scalar, one more than what
    /// it holds for an option, list or record. `?` and an action are 0. A
    /// shape not yet measured counts 1, the least it can be.
    pub fn depth(&self, t: &Ty) -> u32 {
        match t {
            Ty::Option(inner) | Ty::List(inner) => 1 + self.depth(inner),
            Ty::Record(name) => self.depths.get(name).copied().unwrap_or(1),
            _ => 0,
        }
    }

    /// Measure every shape once its fields are known; shapes are acyclic
    /// (`type-shape-recursive`), so each is measured from the ones it holds.
    pub(crate) fn measure_shapes(&mut self) {
        fn shape(
            name: &str,
            map: &BTreeMap<String, Vec<(String, Ty)>>,
            depths: &mut BTreeMap<String, u32>,
        ) -> u32 {
            if let Some(d) = depths.get(name) {
                return *d;
            }
            let fields = map.get(name).map(Vec::as_slice).unwrap_or_default();
            let inner = fields.iter().map(|(_, t)| ty(t, map, depths)).max();
            let d = 1 + inner.unwrap_or(0);
            depths.insert(name.to_owned(), d);
            d
        }
        fn ty(
            t: &Ty,
            map: &BTreeMap<String, Vec<(String, Ty)>>,
            depths: &mut BTreeMap<String, u32>,
        ) -> u32 {
            match t {
                Ty::Option(inner) | Ty::List(inner) => 1 + ty(inner, map, depths),
                Ty::Record(name) => shape(name, map, depths),
                _ => 0,
            }
        }
        let mut depths = BTreeMap::new();
        for name in self.map.keys() {
            shape(name, &self.map, &mut depths);
        }
        self.depths = depths;
    }

    /// Refuse `t` if no target could hold a value of it as the runner does.
    pub fn bounded(&self, t: &Ty, span: Span) -> Result<(), TypeError> {
        if let Some(inner) = option_in_option(t) {
            return err(
                "type-option-option",
                format!("`{t}` holds `option<{inner}>` directly inside an option, and `some(none)` would be `none` on the web: hold the inner option in a record field instead (`shape Maybe` with a field `value: option<{inner}>`)"),
                span,
            );
        }
        let depth = self.depth(t);
        if depth > MAX_TYPE_DEPTH {
            return err(
                "type-too-deep",
                format!(
                    "`{t}` nests {depth} levels deep, and a value nests at most {MAX_TYPE_DEPTH}"
                ),
                span,
            );
        }
        Ok(())
    }
}

/// What the first option directly inside an option in `t` holds.
fn option_in_option(t: &Ty) -> Option<&Ty> {
    match t {
        Ty::Option(inner) => match &**inner {
            Ty::Option(held) => Some(held),
            inner => option_in_option(inner),
        },
        Ty::List(inner) => option_in_option(inner),
        _ => None,
    }
}
