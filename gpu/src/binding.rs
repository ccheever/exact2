use crate::{json, Module};

impl Module {
    /// Bind a positional JSON array or named JSON object through the same typed
    /// bind operation. Unknown names and malformed input leave the surface intact.
    pub fn bind_json(&mut self, id: u32, text: &str, at_ms: Option<f64>) -> bool {
        let result = self
            .instances
            .get(&id)
            .ok_or_else(|| "no such canvas".to_owned())
            .and_then(|inst| values(inst.surface.as_ref(), text));
        match result {
            Ok(values) => self.bind(id, &values, at_ms),
            Err(error) => {
                self.error = error;
                false
            }
        }
    }
}

pub(crate) fn values(
    surface: &dyn crate::Surface,
    text: &str,
) -> Result<Vec<crate::Value>, String> {
    let (names, values) = json::parse_bindings(text)?;
    let mut fields = surface.arguments();
    let Some(names) = names else {
        if fields.is_empty() {
            return Ok(values);
        }
        if values.len() > fields.len() {
            return Err(format!(
                "expected at most {} surface arguments, got {}",
                fields.len(),
                values.len()
            ));
        }
        for ((_, target), value) in fields.iter_mut().zip(values) {
            *target = value;
        }
        return Ok(fields.into_iter().map(|(_, value)| value).collect());
    };
    for (name, value) in names.iter().zip(values) {
        let Some((_, target)) = fields.iter_mut().find(|(field, _)| field == name) else {
            return Err(format!("unknown surface argument `{name}`"));
        };
        *target = value;
    }
    Ok(fields.into_iter().map(|(_, value)| value).collect())
}
