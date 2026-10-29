//! The decorations record at the producer/host boundary (LLP 1114.007).
//! The kernel owns range reconciliation; this module only reads and writes
//! the named JSON fields the hosts receive, and the declaration table.

use exact_kernel::text::decorations::{Decorations, Range, Span};
use exact_kernel::{StyleId, StyleProps, StyleValue};
use exact_plan::Value;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

/// The `decoration` declarations visible to one field, decoded with the
/// same style codecs as node rows.
#[derive(Debug, Clone, Default)]
pub struct StyleTable(BTreeMap<String, StyleProps>);

impl StyleTable {
    /// Read the compiler's ordered `[kernelStyleId, literal]` rows.
    pub fn decode(text: &str) -> Result<Self, String> {
        let table: BTreeMap<String, Json> =
            serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut styles = BTreeMap::new();
        for (name, rows) in table {
            let Some(rows) = rows.as_array() else {
                return Err("decoration styles: expected rows".into());
            };
            let mut patch = StyleProps::default();
            for row in rows {
                let Some(row) = row.as_array().filter(|row| row.len() == 2) else {
                    return Err("decoration styles: invalid row".into());
                };
                let id = row[0]
                    .as_u64()
                    .and_then(|id| StyleId::from_bit(u32::try_from(id).ok()?))
                    .ok_or("decoration styles: unknown row")?;
                let value = match &row[1] {
                    Json::String(text) => StyleValue::Text(text.as_str().into()),
                    Json::Number(number) => StyleValue::Number(
                        number.as_f64().ok_or("decoration styles: invalid number")?,
                    ),
                    _ => return Err("decoration styles: expected literal".into()),
                };
                patch
                    .set_dynamic(id, &value)
                    .map_err(|error| format!("decoration styles: {error:?}"))?;
            }
            styles.insert(name, patch);
        }
        Ok(Self(styles))
    }

    /// A declaration's rows; only the rows its mask names were written.
    pub fn get(&self, name: &str) -> Option<&StyleProps> {
        self.0.get(name)
    }

    /// Visible names in stable order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

/// Read a field's `decorations` prop. An invalid record is refused as a
/// whole; a span naming no visible declaration is the host's to report.
pub fn decode(text: &str) -> Result<Decorations, String> {
    let value: Json = serde_json::from_str(text).map_err(|e| format!("decorations: {e}"))?;
    let source = value
        .get("value")
        .and_then(Json::as_str)
        .ok_or("decorations: value must be a string")?;
    let spans = value
        .get("spans")
        .and_then(Json::as_array)
        .ok_or("decorations: spans must be a list")?
        .iter()
        .map(|item| {
            let offset = |name| {
                item.get(name)
                    .and_then(Json::as_f64)
                    .filter(|n| n.is_finite())
                    .map(|n| n.trunc().clamp(0., f64::from(u32::MAX)) as u32)
                    .ok_or_else(|| format!("decorations: {name} must be a finite offset"))
            };
            Ok(Span {
                name: item
                    .get("name")
                    .and_then(Json::as_str)
                    .ok_or("decorations: name must be a string")?
                    .into(),
                range: Range {
                    start: offset("start")?,
                    end: offset("end")?,
                },
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Decorations {
        value: source.into(),
        spans,
    })
}

/// Write the named record, with no style definitions or host state mixed in.
pub fn encode(record: &Decorations) -> String {
    json!({
        "value": record.value,
        "spans": record.spans.iter().map(|span| {
            json!({"name": span.name, "start": span.range.start, "end": span.range.end})
        }).collect::<Vec<_>>(),
    })
    .to_string()
}

/// Convert the compiler's positional `TextDecorations(value, spans)` to the
/// named prop. Field order is `contract_types::decorations`'.
pub fn from_value(value: &Value) -> Result<String, String> {
    fn record(value: &Value, count: usize) -> Result<&[Value], String> {
        match value {
            Value::Record(fields) if fields.len() == count => Ok(fields),
            _ => Err("decorations: invalid record shape".into()),
        }
    }
    fn offset(value: &Value) -> Result<u32, String> {
        value
            .as_number()
            .filter(|n| n.is_finite())
            .map(|n| n.trunc().clamp(0., f64::from(u32::MAX)) as u32)
            .ok_or_else(|| "decorations: offset must be finite".into())
    }
    fn text(value: &Value) -> Result<&str, String> {
        value
            .as_str()
            .ok_or_else(|| "decorations: expected text".into())
    }
    let fields = record(value, 2)?;
    let Value::List(spans) = &fields[1] else {
        return Err("decorations: expected a list".into());
    };
    let decorations = Decorations {
        value: text(&fields[0])?.into(),
        spans: spans
            .iter()
            .map(|v| {
                let f = record(v, 3)?;
                Ok(Span {
                    name: text(&f[0])?.into(),
                    range: Range {
                        start: offset(&f[1])?,
                        end: offset(&f[2])?,
                    },
                })
            })
            .collect::<Result<_, String>>()?,
    };
    Ok(encode(&decorations))
}

/// `AtomFace`'s fields in the compiler's order (LLP 1045.000 D2): the
/// record [`atom_faces`] reads, each a text, by its JSON name. A plan that
/// binds `atomFaces` puts this list in its compatibility id (`exact_bake`),
/// so a binary that reads other fields is never given it; a test ties it,
/// with each field's type (text, or `direction`'s three words), to the
/// compiler's shape.
pub const ATOM_FACE_FIELDS: [&str; 7] = [
    "source",
    "label",
    "icon",
    "title",
    "color",
    "backgroundColor",
    "direction",
];

/// Convert the compiler's positional `AtomFace` list to the named JSON of
/// the `atomFaces` prop (LLP 1045.000 D2), the colours as written; the plan
/// reads them next (`bridge::atom_face_paint`).
pub fn atom_faces(value: &Value) -> Result<String, String> {
    let Value::List(faces) = value else {
        return Err("atomFaces: expected a list".into());
    };
    let rows = faces
        .iter()
        .map(|face| match face {
            Value::Record(f) if f.len() == ATOM_FACE_FIELDS.len() => {
                let mut row = serde_json::Map::new();
                for (name, field) in ATOM_FACE_FIELDS.iter().zip(f.iter()) {
                    let text = field.as_str().ok_or("atomFaces: expected text")?;
                    row.insert((*name).into(), Json::String(text.into()));
                }
                Ok(Json::Object(row))
            }
            _ => Err("atomFaces: invalid record shape".to_string()),
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(serde_json::Value::Array(rows).to_string())
}

/// A face's `color` or `backgroundColor` (LLP 1045.000 D2) as a colour row
/// reads a text, with whatever theme is bound: `None` when empty, so its
/// kind's paint stands, or when it is no colour. A host reads the wire's
/// text the runner wrote (`bridge::atom_face_paint`).
pub fn face_color(text: &str) -> Option<exact_kernel::ColorValue> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    exact_kernel::ColorValue::parse_light_dark(text)
        .or_else(|| exact_kernel::Color::parse(text).map(exact_kernel::ColorValue::Fixed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_decode_their_paint_rows_by_name() {
        let table = json!({"mention":[
            [StyleId::TextColor as u16,"#1083fe"],
            [StyleId::TextDecorationLine as u16,"underline"]
        ]});
        let table = StyleTable::decode(&table.to_string()).unwrap();
        let style = table.get("mention").unwrap();
        assert!(style.mask.has(StyleId::TextColor));
        assert_eq!(
            style.text_decoration_line,
            exact_kernel::TextDecorationLine::Underline
        );
        assert!(!style.mask.has(StyleId::BackgroundColor));
        assert!(table.get("unknown").is_none());
        assert_eq!(table.names().collect::<Vec<_>>(), ["mention"]);
        assert!(StyleTable::decode(r#"{"bad":[[65535,1]]}"#).is_err());
    }

    #[test]
    fn a_faces_paint_is_a_colour_or_none() {
        use exact_kernel::{Color, ColorValue};
        assert_eq!(
            face_color(" light-dark(#d52c36, #ff6762) "),
            Some(ColorValue::LightDark(
                Color::parse("#d52c36").unwrap(),
                Color::parse("#ff6762").unwrap()
            ))
        );
        assert_eq!(
            face_color("#1a85d4"),
            Some(ColorValue::Fixed(Color::parse("#1a85d4").unwrap()))
        );
        // What the JS target refuses too (`shape.js`): a cascade keyword
        // anywhere, and a substitution but a role's web form.
        for none in [
            "",
            "  ",
            "not a colour",
            "light-dark(#d52c36 #ff6762)",
            "currentcolor",
            "inherit",
            "light-dark(currentcolor, #000)",
            "var(--x)",
            "var(--x, #fff)",
            "env(safe-area-inset-top)",
            "attr(data-c type(<color>))",
            "if(media(print): red)",
            "light-dark(if(style(--x: 1): red), #000)",
            "inherit(--x)",
            "v\\61r(--exact-separator)",
            "var(--exact-lin\u{212A}text)",
        ] {
            assert_eq!(face_color(none), None, "{none:?}");
        }
        for role in [
            "var(--exact-separator)",
            "VAR(--exact-separator, inherit)",
            "var(--exact-accent, var(--exact-linktext))",
        ] {
            assert!(
                matches!(face_color(role), Some(ColorValue::Role(_))),
                "{role:?}"
            );
        }
    }

    #[test]
    fn producer_record_serializes_named_fields() {
        let span = Value::record(vec![
            Value::str("mention"),
            Value::Number(1.),
            Value::Number(3.),
        ]);
        let record = Value::record(vec![Value::str("a😀z"), Value::list(vec![span])]);
        let text = from_value(&record).unwrap();
        let parsed: Json = serde_json::from_str(&text).unwrap();
        assert_eq!(
            parsed,
            json!({"value":"a😀z","spans":[{"name":"mention","start":1,"end":3}]})
        );
        let decoded = decode(&text).unwrap();
        assert_eq!(decoded.spans[0].range, Range { start: 1, end: 3 });
        assert_eq!(decode(&encode(&decoded)).unwrap(), decoded);
    }

    #[test]
    fn malformed_records_fail_as_a_whole_and_offsets_are_finite_unsigned_integers() {
        let mut record = json!({"value":"abc","spans":[{"name":"unknown","start":-4.2,"end":3.9}]});
        let parsed = decode(&record.to_string()).unwrap();
        assert_eq!(parsed.spans[0].range, Range { start: 0, end: 3 });
        record["spans"][0]["start"] = json!("bad");
        assert!(decode(&record.to_string()).is_err());
        for malformed in ["{}", "null", "[]", "{", r#"{"value":"a"}"#] {
            assert!(decode(malformed).is_err(), "{malformed}");
        }
        for malformed in [
            Value::Unit,
            Value::record(vec![]),
            Value::record(vec![
                Value::str("a"),
                Value::list(vec![Value::record(vec![
                    Value::str("h"),
                    Value::Number(f64::NAN),
                    Value::Number(2.),
                ])]),
            ]),
        ] {
            assert!(from_value(&malformed).is_err(), "{malformed:?}");
        }
    }
}
