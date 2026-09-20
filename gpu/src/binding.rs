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
    admit(text)?;
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

pub(crate) fn admit(text: &str) -> Result<(), String> {
    if text.len() > 16_384 {
        return Err("surface request exceeds 16384 bytes".into());
    }
    let (mut depth, mut string, mut escape) = (0u32, false, false);
    for b in text.bytes() {
        if string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                string = false;
            }
        } else {
            match b {
                b'"' => string = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 64 {
                        return Err("surface request exceeds depth 64".into());
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}
