//! The decorations record at the producer/host boundary (LLP 1114.007).
//! The kernel owns range reconciliation; this module only reads and writes
//! the named JSON fields that native and browser editors receive.

use exact_kernel::text::decorations::{Atom, Block, Decorations, Hidden, Marker, Range, Span};
use exact_kernel::{StyleId, StyleProps, StyleValue};
use exact_plan::Value;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

/// Compiled declaration aliases, decoded with the same style codecs as node rows.
#[derive(Debug, Clone, Default)]
pub struct StyleTable(BTreeMap<String, StyleProps>);

impl StyleTable {
    /// Read the compiler's internal ordered `[kernelStyleId, literal]` rows.
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

    /// A declaration's authored rows in a field's root/font/display context.
    pub fn resolve(&self, name: &str, root: f32, font: f32, scale: f32) -> Option<StyleProps> {
        let mut patch = self.0.get(name)?.clone();
        let font_size = patch.relative.get(StyleId::FontSize).map_or_else(
            || {
                if patch.mask.has(StyleId::FontSize) {
                    patch.font_size
                } else {
                    font
                }
            },
            |(unit, number)| number * unit.basis(root, font, 1.0 / scale.max(1.0)),
        );
        for id in patch.mask.iter().collect::<Vec<_>>() {
            if let Some((unit, number)) = patch.relative.get(id) {
                let reference = if id == StyleId::FontSize {
                    font
                } else {
                    font_size
                };
                let px = number * unit.basis(root, reference, 1.0 / scale.max(1.0));
                let value = if id == StyleId::LineHeight {
                    StyleValue::Text(format!("{px}px").into())
                } else {
                    StyleValue::Number(f64::from(px))
                };
                patch.set_dynamic(id, &value).ok()?;
            }
        }
        Some(patch)
    }

    /// Visible aliases in stable order, for a host's declaration dictionary.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

/// Read a field's `decorations` prop. An invalid record is refused as a whole.
/// Unknown marker words draw no marker; unknown declaration names remain for
/// the host to report once when resolving its per-field style table.
pub fn decode(text: &str) -> Result<Decorations, String> {
    let value: Json = serde_json::from_str(text).map_err(|e| format!("decorations: {e}"))?;
    let text = string(&value, "value")?;
    let spans = list(&value, "spans")?
        .iter()
        .map(named_range)
        .collect::<Result<_, _>>()?;
    let blocks = list(&value, "blocks")?
        .iter()
        .map(|item| {
            Ok(Block {
                style: named_range(item)?,
                marker: match item.get("marker") {
                    None | Some(Json::Null) => None,
                    Some(Json::String(word)) => Marker::parse(word),
                    _ => return Err("decorations: marker must be a string".into()),
                },
            })
        })
        .collect::<Result<_, String>>()?;
    let hidden = list(&value, "hidden")?
        .iter()
        .map(|item| {
            Ok(Hidden {
                range: source_range(item)?,
                reveal: match item.get("reveal") {
                    None | Some(Json::Null) => None,
                    Some(range) => Some(source_range(range)?),
                },
            })
        })
        .collect::<Result<_, String>>()?;
    let atoms = list(&value, "atoms")?
        .iter()
        .map(|item| {
            Ok(Atom {
                style: named_range(item)?,
                label: string(item, "label")?.into(),
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Decorations {
        value: text.into(),
        spans,
        blocks,
        hidden,
        atoms,
    })
}

/// Write the named record, with no style definitions or host state mixed in.
pub fn encode(record: &Decorations) -> String {
    let named =
        |span: &Span| json!({"name": span.name, "start": span.range.start, "end": span.range.end});
    json!({
        "value": record.value,
        "spans": record.spans.iter().map(named).collect::<Vec<_>>(),
        "blocks": record.blocks.iter().map(|block| {
            let mut item = named(&block.style);
            item["marker"] = block.marker.map_or(Json::Null, |marker| Json::String(marker.as_str().into()));
            item
        }).collect::<Vec<_>>(),
        "hidden": record.hidden.iter().map(|hidden| json!({
            "start": hidden.range.start,
            "end": hidden.range.end,
            "reveal": hidden.reveal.map(|range| json!({"start":range.start,"end":range.end}))
        })).collect::<Vec<_>>(),
        "atoms": record.atoms.iter().map(|atom| {
            let mut item = named(&atom.style);
            item["label"] = Json::String(atom.label.clone());
            item
        }).collect::<Vec<_>>()
    })
    .to_string()
}

/// Convert the compiler's canonical positional record to the named prop.
/// Field order is declared by `contract_types::decorations`; no app JSON
/// stringification or new VM instruction is required.
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
    fn source_range(fields: &[Value]) -> Result<Range, String> {
        Ok(Range {
            start: offset(&fields[0])?,
            end: offset(&fields[1])?,
        })
    }
    fn named(fields: &[Value]) -> Result<Span, String> {
        Ok(Span {
            name: text(&fields[0])?.into(),
            range: source_range(&fields[1..])?,
        })
    }
    fn list(value: &Value) -> Result<&[Value], String> {
        match value {
            Value::List(items) => Ok(items),
            _ => Err("decorations: expected a list".into()),
        }
    }
    let fields = record(value, 5)?;
    let decorations = Decorations {
        value: text(&fields[0])?.into(),
        spans: list(&fields[1])?
            .iter()
            .map(|v| named(record(v, 3)?))
            .collect::<Result<_, String>>()?,
        blocks: list(&fields[2])?
            .iter()
            .map(|v| {
                let f = record(v, 4)?;
                Ok(Block {
                    style: named(f)?,
                    marker: Marker::parse(text(&f[3])?),
                })
            })
            .collect::<Result<_, String>>()?,
        hidden: list(&fields[3])?
            .iter()
            .map(|v| {
                let f = record(v, 3)?;
                let reveal = match &f[2] {
                    Value::Option(None) => None,
                    Value::Option(Some(value)) => Some(source_range(record(value, 2)?)?),
                    _ => return Err("decorations: reveal must be an optional range".into()),
                };
                Ok(Hidden {
                    range: source_range(f)?,
                    reveal,
                })
            })
            .collect::<Result<_, String>>()?,
        atoms: list(&fields[4])?
            .iter()
            .map(|v| {
                let f = record(v, 4)?;
                Ok(Atom {
                    style: named(f)?,
                    label: text(&f[3])?.into(),
                })
            })
            .collect::<Result<_, String>>()?,
    };
    Ok(encode(&decorations))
}

fn string<'a>(value: &'a Json, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("decorations: {name} must be a string"))
}
fn list<'a>(value: &'a Json, name: &str) -> Result<&'a [Json], String> {
    value
        .get(name)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("decorations: {name} must be a list"))
}
fn source_range(value: &Json) -> Result<Range, String> {
    let offset = |name| {
        value
            .get(name)
            .and_then(Json::as_f64)
            .filter(|n| n.is_finite())
            .map(|n| n.trunc().clamp(0., f64::from(u32::MAX)) as u32)
            .ok_or_else(|| format!("decorations: {name} must be a finite offset"))
    };
    Ok(Range {
        start: offset("start")?,
        end: offset("end")?,
    })
}
fn named_range(value: &Json) -> Result<Span, String> {
    Ok(Span {
        name: string(value, "name")?.into(),
        range: source_range(value)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_lengths_resolve_against_field_font_root_and_display_scale() {
        let table = json!({"heading":[
            [StyleId::FontSize as u16,"1.5em"],
            [StyleId::PaddingLeft as u16,"2em"],
            [StyleId::MarginBottom as u16,"0.5rem"],
            [StyleId::LineHeight as u16,"1.25em"]
        ]});
        let table = StyleTable::decode(&table.to_string()).unwrap();
        let style = table.resolve("heading", 20., 16., 2.).unwrap();
        assert_eq!(style.font_size, 24.);
        assert_eq!(style.padding_left, exact_kernel::Dimension::Points(48.));
        assert_eq!(style.margin_bottom, exact_kernel::Dimension::Points(10.));
        assert_eq!(style.line_height, exact_kernel::LineHeight::Length(30.));
        assert!(table.resolve("unknown", 20., 16., 2.).is_none());
        assert!(StyleTable::decode(r#"{"bad":[[65535,1]]}"#).is_err());
    }

    #[test]
    fn producer_record_serializes_named_fields_and_optional_ranges() {
        let named = |word: &str| vec![Value::str(word), Value::Number(1.), Value::Number(3.)];
        let mut block = named("heading");
        block.push(Value::str("checkbox:on"));
        let mut atom = named("mention");
        atom.push(Value::str("😀"));
        let record = Value::record(vec![
            Value::str("a😀z"),
            Value::list(vec![Value::record(named("mention"))]),
            Value::list(vec![Value::record(block)]),
            Value::list(vec![Value::record(vec![
                Value::Number(0.),
                Value::Number(1.),
                Value::some(Value::record(vec![Value::Number(0.), Value::Number(4.)])),
            ])]),
            Value::list(vec![Value::record(atom)]),
        ]);
        let text = from_value(&record).unwrap();
        let parsed: Json = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["value"], "a😀z");
        assert_eq!(
            parsed["spans"][0],
            json!({"name":"mention","start":1,"end":3})
        );
        assert_eq!(parsed["blocks"][0]["marker"], "checkbox:on");
        assert_eq!(parsed["hidden"][0]["reveal"], json!({"start":0,"end":4}));
        let decoded = decode(&text).unwrap();
        assert_eq!(decoded.spans[0].range, Range { start: 1, end: 3 });
        assert_eq!(decoded.blocks[0].marker, Some(Marker::CheckboxOn));
        assert_eq!(decoded.atoms[0].label, "😀");
        assert_eq!(decode(&encode(&decoded)).unwrap(), decoded);
    }

    #[test]
    fn malformed_records_fail_as_a_whole_and_offsets_are_finite_unsigned_integers() {
        let mut record = json!({"value":"abc","spans":[{"name":"unknown","start":-4.2,"end":3.9}],"blocks":[{"name":"h","start":0,"end":3,"marker":"new-word"}],"hidden":[{"start":0,"end":1}],"atoms":[]});
        let parsed = decode(&record.to_string()).unwrap();
        assert_eq!(parsed.spans[0].range, Range { start: 0, end: 3 });
        assert_eq!(parsed.blocks[0].marker, None);
        assert_eq!(parsed.hidden[0].reveal, None);
        record["spans"][0]["start"] = json!("bad");
        assert!(decode(&record.to_string()).is_err());
        for malformed in [
            "{}",
            "null",
            "[]",
            "{",
            r#"{"value":"a","spans":[],"blocks":[],"hidden":[]}"#,
        ] {
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
                Value::list(vec![]),
                Value::list(vec![]),
                Value::list(vec![]),
            ]),
        ] {
            assert!(from_value(&malformed).is_err(), "{malformed:?}");
        }
    }
}
