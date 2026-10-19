//! A `decoration` declaration's rows as the body of its `::highlight()` rule
//! (LLP 1114.007), through the same style parser and CSS writer as nodes.

use exact_kernel::{StyleId, StyleProps, StyleValue};
use serde_json::{Map, Value};

/// Convert the compiler's per-field style table to named CSS declarations.
pub fn decoration_styles(json: &str, fonts: &[String]) -> String {
    let Ok(Value::Object(table)) = serde_json::from_str(json) else {
        return "{}".into();
    };
    let mut result = Map::new();
    for (name, rows) in table {
        let mut style = StyleProps::default();
        for row in rows.as_array().into_iter().flatten() {
            let Some(pair) = row.as_array().filter(|pair| pair.len() == 2) else {
                continue;
            };
            let Some(id) = pair[0]
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .and_then(StyleId::from_bit)
            else {
                continue;
            };
            let value = match &pair[1] {
                Value::String(text) => StyleValue::Text(text.as_str().into()),
                Value::Number(number) => match number.as_f64() {
                    Some(value) => StyleValue::Number(value),
                    None => continue,
                },
                _ => continue,
            };
            let _ = style.set_dynamic(id, &value);
        }
        result.insert(
            name,
            Value::String(super::css_text_relative(&style, fonts).0),
        );
    }
    Value::Object(result).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_become_highlight_rule_bodies() {
        let input = serde_json::json!({"mention":[
            [StyleId::TextColor as u16,"#1083fe"],
            [StyleId::BackgroundColor as u16,"#eeeeff"],
            [StyleId::TextDecorationLine as u16,"underline"]
        ]});
        let output: Value =
            serde_json::from_str(&decoration_styles(&input.to_string(), &[])).unwrap();
        let css = output["mention"].as_str().unwrap();
        assert!(css.contains("color:"), "{css}");
        assert!(css.contains("background-color:"), "{css}");
        assert!(css.contains("text-decoration-line:underline"), "{css}");
    }
}
