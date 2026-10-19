//! Inline-only source decorations for the shared Linux/Windows field presenter.
//! Blocks, hidden ranges and atoms retain their source and ordinary editing.
use super::Spec;
use exact_kernel::text::decorations::Decorations;
use exact_kernel::{ColorValue, StyleId};
use exact_runner::decorations::StyleTable;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct FieldDecorations {
    record: Decorations,
    styles: StyleTable,
    root: f32,
    scale: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{StyleProps, WhiteSpace};
    use serde_json::json;

    fn record(value: &str, spans: serde_json::Value) -> String {
        json!({"value":value,"spans":spans,"blocks":[],"hidden":[],"atoms":[]}).to_string()
    }
    fn spec(value: &str) -> Spec {
        crate::paint::text_spec(
            &StyleProps {
                font_size: 16.,
                white_space: WhiteSpace::PreWrap,
                ..Default::default()
            },
            value,
        )
    }

    #[test]
    fn spans_preserve_unicode_source_and_merge_overlapping_declarations_in_order() {
        let source = "a😀bold z";
        let record = record(
            source,
            json!([
                {"name":"strong","start":1,"end":7},
                {"name":"blue","start":3,"end":7},
            ]),
        );
        let styles = json!({
            "strong":[[StyleId::FontWeight as u16,700],[StyleId::TextColor as u16,"#ff0000"]],
            "blue":[[StyleId::TextColor as u16,"#0000ff"],[StyleId::FontStyle as u16,"italic"],[StyleId::BackgroundColor as u16,"#ffff00"]]
        });
        let decorated = FieldDecorations::read(&record, &styles.to_string(), 16., 1.)
            .unwrap()
            .project(spec(source), source);
        let runs = &decorated.spec.runs;
        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<String>(),
            source
        );
        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(),
            ["a", "😀", "bold", " z"]
        );
        assert_eq!(runs[1].weight, 700);
        assert_eq!(runs[2].weight, 700);
        assert!(runs[2].italic);
        assert_eq!(
            crate::paint::rgba(decorated.inks[1].unwrap().resolve(false)),
            [255, 0, 0, 255]
        );
        assert_eq!(
            crate::paint::rgba(decorated.inks[2].unwrap().resolve(false)),
            [0, 0, 255, 255]
        );
        assert_eq!(
            crate::paint::rgba(decorated.backgrounds[2].unwrap().unwrap().resolve(false)),
            [255, 255, 0, 255]
        );
    }

    #[test]
    fn stale_spans_reconcile_and_unsupported_metadata_keeps_ordinary_source() {
        let record = json!({"value":"@a end","spans":[{"name":"size","start":0,"end":2}],
            "blocks":[{"name":"size","start":0,"end":6,"marker":"rule"}],
            "hidden":[{"start":0,"end":2,"reveal":null}],
            "atoms":[{"name":"size","start":0,"end":2,"label":"Alice"}]})
        .to_string();
        let styles = json!({"size":[[StyleId::FontSize as u16,"1.5rem"],[StyleId::TextDecorationLine as u16,"underline"]]}).to_string();
        let source = "X @a end";
        let projected = FieldDecorations::read(&record, &styles, 20., 1.)
            .unwrap()
            .project(spec(source), source);
        assert_eq!(
            projected
                .spec
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            source
        );
        let span = projected
            .spec
            .runs
            .iter()
            .find(|run| run.text == "X @a")
            .unwrap();
        assert_eq!(span.size, 30.);
        assert_eq!(span.mark, super::super::markup::UNDERLINE);
        assert!(projected
            .spec
            .runs
            .iter()
            .all(|run| run.indent == 0. && !run.hang));
    }

    #[test]
    fn many_disjoint_spans_keep_source_and_precedence_without_rescanning_the_document() {
        let source = "ab".repeat(10_000);
        let spans: Vec<_> = (0..10_000)
            .map(|index| json!({"name":"strong","start":index*2,"end":index*2+1}))
            .collect();
        let styles = json!({"strong":[[StyleId::FontWeight as u16,700]]}).to_string();
        let projected = FieldDecorations::read(&record(&source, json!(spans)), &styles, 16., 1.)
            .unwrap()
            .project(spec(&source), &source);
        assert_eq!(projected.spec.runs.len(), 20_000);
        assert_eq!(
            projected
                .spec
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            source
        );
        assert!(projected
            .spec
            .runs
            .iter()
            .enumerate()
            .all(|(index, run)| run.weight == if index % 2 == 0 { 700 } else { 400 }));
    }
}

pub(crate) struct Projection {
    pub spec: Spec,
    pub inks: Vec<Option<ColorValue>>,
    pub backgrounds: Vec<Option<Option<ColorValue>>>,
}

impl FieldDecorations {
    pub(crate) fn state(&self, value: &str) -> serde_json::Value {
        let resolved = self.record.resolve(value);
        let spans: Vec<_> = resolved.spans.iter().filter(|span|self.styles.names().any(|name|name==span.name)).map(|span| {
            serde_json::json!({"name":span.name,"start":span.range.start,"end":span.range.end})
        }).collect();
        serde_json::json!({"source":value,"recordValue":self.record.value,"provisional":resolved.provisional,"spans":spans,
            "ignored":["blocks","hidden","atoms"]})
    }

    pub(crate) fn read(record: &str, styles: &str, root: f32, scale: f32) -> Result<Self, String> {
        Ok(Self {
            record: exact_runner::decorations::decode(record)?,
            styles: StyleTable::decode(styles)?,
            root,
            scale,
        })
    }

    pub(crate) fn project(&self, mut spec: Spec, value: &str) -> Projection {
        let resolved = self.record.resolve(value);
        let length = value.encode_utf16().count() as u32;
        let styles: BTreeMap<_, _> = self
            .styles
            .names()
            .filter_map(|name| {
                Some((
                    name,
                    self.styles
                        .resolve(name, self.root, spec.strut.size, self.scale)?,
                ))
            })
            .collect();
        let mut events: BTreeMap<u32, Vec<(usize, bool)>> = BTreeMap::new();
        events.entry(0).or_default();
        events.entry(length).or_default();
        for (index, span) in resolved.spans.iter().enumerate() {
            if styles.contains_key(span.name.as_str()) {
                events
                    .entry(span.range.start)
                    .or_default()
                    .push((index, true));
                events
                    .entry(span.range.end)
                    .or_default()
                    .push((index, false));
            }
        }
        let events: Vec<_> = events.into_iter().collect();
        let mut units = 0;
        let mut offsets = vec![(0, 0)];
        for (byte, character) in value.char_indices() {
            units += character.len_utf16() as u32;
            offsets.push((units, byte + character.len_utf8()));
        }
        let byte = |offset: u32| {
            offsets[offsets
                .partition_point(|(units, _)| *units < offset)
                .min(offsets.len() - 1)]
            .1
        };
        let mut runs = Vec::new();
        let mut inks = Vec::new();
        let mut backgrounds = Vec::new();
        let mut active = BTreeMap::new();
        for pair in events.windows(2) {
            for &(index, starts) in &pair[0].1 {
                if starts {
                    active.insert(index, styles[resolved.spans[index].name.as_str()].clone());
                } else {
                    active.remove(&index);
                }
            }
            let start = byte(pair[0].0);
            let end = byte(pair[1].0);
            if start == end {
                continue;
            }
            let mut run = spec.strut.clone();
            run.text = value[start..end].into();
            let (mut ink, mut background) = (None, None);
            for style in active.values() {
                let has = |id| style.mask.has(id);
                if has(StyleId::FontSize) {
                    run.size = style.font_size
                }
                if has(StyleId::FontWeight) {
                    run.weight = style.font_weight
                }
                if has(StyleId::FontFamily) {
                    run.family = style.font_family
                }
                if has(StyleId::FontStyle) {
                    run.italic = style.font_style != exact_kernel::FontStyle::Normal
                }
                if has(StyleId::FontVariantNumeric) {
                    run.font_variant_numeric = style.font_variant_numeric
                }
                if has(StyleId::TextColor) {
                    ink = Some(style.text_color)
                }
                if has(StyleId::BackgroundColor) {
                    background = Some(style.background_color)
                }
                if has(StyleId::TextDecorationLine) {
                    use exact_kernel::TextDecorationLine as Line;
                    run.mark = match style.text_decoration_line {
                        Line::None => 0,
                        Line::Underline => super::markup::UNDERLINE,
                        Line::LineThrough => super::markup::STRIKE,
                        Line::UnderlineLineThrough => {
                            super::markup::UNDERLINE | super::markup::STRIKE
                        }
                    };
                }
            }
            runs.push(run);
            inks.push(ink);
            backgrounds.push(background);
        }
        if runs.is_empty() {
            let mut run = spec.strut.clone();
            run.text = value.into();
            runs.push(run);
            inks.push(None);
            backgrounds.push(None);
        }
        spec.runs = runs;
        Projection {
            spec,
            inks,
            backgrounds,
        }
    }
}
