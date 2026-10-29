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

/// A Markdown node's `atomFaces` (LLP 1045.000 D2) with each face's `color`
/// and `backgroundColor`, which the runner wrote as the wire's text, as the
/// CSS the page sets on its pill; else empty, so the kind's paint stands.
/// The rest stays as the runner wrote it.
pub fn atom_faces(json: &str) -> String {
    let Ok(Value::Array(mut faces)) = serde_json::from_str(json) else {
        return json.to_string();
    };
    for face in faces.iter_mut().filter_map(Value::as_object_mut) {
        for key in ["color", "backgroundColor"] {
            if let Some(Value::String(text)) = face.get_mut(key) {
                let mut css = String::new();
                if let Some(color) = exact_runner::decorations::face_color(text) {
                    exact_kernel::gradient::color_css(&mut css, color);
                }
                *text = css;
            }
        }
    }
    Value::Array(faces).to_string()
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

    #[test]
    fn a_faces_paint_becomes_css_and_anything_else_empty() {
        let faces = serde_json::json!([
            {"source": "@a", "label": "A", "icon": "", "title": "",
             "color": "light-dark(#D52C36, #ff6762)", "backgroundColor": ""},
            {"source": "@b", "label": "", "icon": "", "title": "",
             "color": "not a colour", "backgroundColor": "-exact-platform-color(web Highlight, #fff)"},
        ]);
        let out: Value = serde_json::from_str(&atom_faces(&faces.to_string())).unwrap();
        assert_eq!(out[0]["color"], "light-dark(#d52c36ff, #ff6762ff)");
        assert_eq!(out[0]["backgroundColor"], "");
        assert_eq!(out[0]["label"], "A");
        // Not a colour; and a platform colour, which the runner admitted as one
        // of the plan's literals (LLP 1095 D3), as its web colour.
        assert_eq!(out[1]["color"], "");
        assert_eq!(out[1]["backgroundColor"], "Highlight");
        assert_eq!(atom_faces("not json"), "not json");
    }
}
