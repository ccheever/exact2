//! Simulation rows read by type name, for a declaration that names the game's
//! types as data rather than linking them (LLP 1046.009 §3.2, an experiment):
//! what is registered, each type's `Default` written, a strict read to check a
//! value against, and rows written through `Data` as saves write them.
use super::*;

/// One registered type, as a checker sees it.
#[derive(Clone, Copy)]
pub struct Registered {
    /// The type's registered name.
    pub name: &'static str,
    /// Registered as a component.
    pub component: bool,
    /// Registered as a resource.
    pub resource: bool,
    /// A presentation component (never read by a present).
    pub presentation: bool,
    write_default: fn(&mut dyn Writer),
    read_json: fn(&str, &mut dyn Writer) -> Result<(), DataError>,
}
impl Registered {
    /// Write the type's `Default` (its fields, their shapes and names).
    pub fn write_default(&self, w: &mut dyn Writer) {
        (self.write_default)(w);
    }
    /// Read JSON over the type's `Default`, strictly, and write the result: an
    /// unknown enum arm is refused, so a checker can ask whether
    /// `{"mind":{"Chase":{}}}` names one, and learn the arm's index.
    pub fn read_json(&self, text: &str, w: &mut dyn Writer) -> Result<(), DataError> {
        (self.read_json)(text, w)
    }
}

pub(super) fn write_default<C: Data>(w: &mut dyn Writer) {
    C::default().write(w);
}
pub(super) fn read_json<C: Data>(text: &str, w: &mut dyn Writer) -> Result<(), DataError> {
    let mut value = C::default();
    crate::json::read_into(text, &mut value)?;
    value.write(w);
    Ok(())
}

impl World {
    /// Every registered component and resource, by name.
    pub fn registered(&self) -> Vec<Registered> {
        self.registry
            .iter()
            .map(|(&name, r)| Registered {
                name,
                component: r.make.is_some(),
                resource: r.make_resource.is_some(),
                presentation: r.presentation,
                write_default: r.write_default,
                read_json: r.read_json,
            })
            .collect()
    }
    fn simulation_storage(&self, name: &str) -> Option<&dyn Erased> {
        if self.registry.get(name).is_some_and(|r| r.presentation) {
            return None;
        }
        self.components.get(name).map(|s| &**s)
    }
    /// Write `e`'s `name` row through `w`; false if it has none. Presentation
    /// rows are never written here.
    pub fn write_component(&self, name: &str, e: Entity, w: &mut dyn Writer) -> bool {
        self.contains(e)
            && self
                .simulation_storage(name)
                .is_some_and(|s| s.write_one(e.index() as usize, w))
    }
    /// Whether `e` has a `name` simulation row.
    pub fn has_component(&self, name: &str, e: Entity) -> bool {
        self.contains(e)
            && self
                .simulation_storage(name)
                .is_some_and(|s| s.has(e.index() as usize))
    }
    /// Write the `name` resource through `w`; false if it is absent.
    pub fn write_resource(&self, name: &str, w: &mut dyn Writer) -> bool {
        self.resources.get(name).is_some_and(|s| s.write_one(0, w))
    }
    /// Visit every entity with a `name` row, in entity order.
    pub fn visit_component(&self, name: &str, each: &mut dyn FnMut(Entity)) {
        if let Some(s) = self.simulation_storage(name) {
            s.visit_indices(&mut |index| each(self.entity_at(index)));
        }
    }
    /// The storage's identity and write revision, for caches keyed on what a
    /// reader read: either changes when any row of it may have.
    pub fn version_of(&self, name: &str) -> Option<(u64, u64)> {
        self.simulation_storage(name)
            .or_else(|| self.resources.get(name).map(|s| &**s))
            .map(|s| (s.instance(), s.revision()))
    }
}
