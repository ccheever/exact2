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
    pub(crate) fn apply_set(&mut self, text: &str, development: bool) -> bool {
        let apply = |w: &mut World| -> Result<(), DataError> {
            if !development {
                return Err(DataError::new("set is unavailable in a production bake"));
            }
            let (name, component, field, value) = command(text)
                .ok_or_else(|| DataError::new("set needs entity Component.field value"))?;
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
            Ok(()) => {
                self.set_seen.set(true);
                self.log(text);
                true
            }
            Err(error) => {
                self.log(format_args!("set refused: {text}: {error}"));
                false
            }
        }
    }
}
/// Recognize the whole command before reserving a gameplay message.
pub(crate) fn command(text: &str) -> Option<(&str, &str, &str, &str)> {
    let mut rest = text.trim();
    let mut word = || {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let value = &rest[..end];
        rest = rest[end..].trim_start();
        value
    };
    if word() != "set" {
        return None;
    }
    let name = word();
    let path = word();
    let (component, field) = path.split_once('.')?;
    if name.is_empty()
        || component.is_empty()
        || field.is_empty()
        || rest.is_empty()
        || field.contains('.')
    {
        return None;
    }
    Some((name, component, field, rest))
}
