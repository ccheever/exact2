use super::*;
impl World {
    pub(super) fn edit_component<C: Component>(
        &mut self,
        entity: Entity,
        field: &str,
        text: &str,
    ) -> Result<(), DataError> {
        let current = self
            .get::<C>(entity)
            .ok_or_else(|| DataError::new(format!("entity has no {}", C::NAME)))?;
        let mut candidate: C = crate::bin::from_slice(&crate::bin::to_vec(&*current))?;
        drop(current);
        candidate.set_field(field, text)?;
        // Re-run the complete component decoder, including custom domain validation,
        // before acquiring its writable row or changing any mutation epoch.
        let candidate = crate::bin::from_slice::<C>(&crate::bin::to_vec(&candidate))?;
        if self.insert(entity, candidate) {
            Ok(())
        } else {
            Err(DataError::new("component combination refused"))
        }
    }
    pub(crate) fn apply_set(&mut self, text: &str, development: bool) {
        self.set_seen.set(true);
        let apply = |w: &mut World| -> Result<(), DataError> {
            if !development {
                return Err(DataError::new("set is unavailable in a production bake"));
            }
            let mut words = text.splitn(4, char::is_whitespace);
            let _set = words.next();
            let name = words
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| DataError::new("set needs an entity name"))?;
            let path = words.next().unwrap_or_default();
            let value = words.next().unwrap_or_default().trim();
            let (component, field) = path
                .split_once('.')
                .ok_or_else(|| DataError::new("set needs Component.field"))?;
            let entity = w
                .named(name)
                .ok_or_else(|| DataError::new(format!("unknown entity {name}")))?;
            let edit = w
                .registry
                .get(component)
                .and_then(|r| r.edit)
                .ok_or_else(|| {
                    DataError::new(format!("unknown or uneditable component {component}"))
                })?;
            edit(w, entity, field, value).map_err(|e| e.at(format!("{name}.{component}.{field}")))
        };
        match apply(self) {
            Ok(()) => self.log(text),
            Err(error) => self.log(format_args!("set refused: {text}: {error}")),
        }
    }
}
pub(crate) fn is_set(text: &str) -> bool {
    text.split_whitespace().next() == Some("set")
}
