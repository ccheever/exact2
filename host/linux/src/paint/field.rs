//! A text field's own paint: its container (LLP 1104 D7), then its value or
//! placeholder, its selection and its caret (`caret.rs`).
//!
//! Where the shell hosts the platform's own field (LLP 1115.001 D8:
//! Android's `EditText`; [`crate::fields`]) the text, the placeholder, the
//! caret and the selection are that field's, and nothing of them is painted
//! here: the walk paints the container and its focus ring, names the field
//! to the backend in its content box, where it stacks in tree order under
//! the clips around it, and keeps where that box is in the field's own
//! (`Painter::field_content`), which the shell's field is placed by.
use super::*;
use crate::text::decorations::FieldDecorations;
use crate::text::field::{line_left, Cursor, Geometry};
use crate::text::{Paragraph, Spec, TextEngine};
use std::borrow::Cow;

pub(crate) struct PaintedField {
    pub(crate) geometry: Geometry,
    pub(crate) value: String,
    spec: Spec,
    content: Rect4,
    multiline: bool,
    masked: bool,
    origin: (f32, f32),
    pub(super) transform: Transform,
    decorations: Option<Rc<FieldDecorations>>,
}

impl PaintedField {
    pub(crate) fn decoration_state(&self) -> Option<serde_json::Value> {
        self.decorations
            .as_ref()
            .map(|decorations| decorations.state(&self.value))
    }

    fn new(
        paragraph: &Paragraph,
        value: &str,
        spec: Spec,
        content: Rect4,
        multiline: bool,
        masked: bool,
        transform: Transform,
        decorations: Option<Rc<FieldDecorations>>,
    ) -> Self {
        let left = |width| line_left(spec.align, spec.direction, content.2, width);
        let empty_x = left(0.);
        let first_x = left(spec.text_indent)
            + if spec.direction == exact_kernel::Direction::Rtl {
                0.
            } else {
                spec.text_indent
            };
        let origin = (
            content.0
                + if multiline {
                    0.
                } else if value.is_empty() {
                    first_x
                } else {
                    left(paragraph.width)
                },
            content.1
                + if multiline {
                    0.
                } else {
                    (content.3 - paragraph.height) / 2.
                },
        );
        Self {
            geometry: Geometry::new(
                paragraph,
                value,
                masked,
                if multiline { empty_x } else { 0. },
                if multiline { first_x } else { 0. },
            ),
            value: value.to_owned(),
            spec,
            content,
            multiline,
            masked,
            origin,
            transform,
            decorations,
        }
    }

    pub(crate) fn for_value(&self, text: &mut TextEngine, value: &str) -> Self {
        let mut spec = self.spec.clone();
        let mut run = spec.strut.clone();
        run.text = if self.masked {
            "•".repeat(value.chars().count())
        } else {
            value.to_owned()
        };
        spec.runs = vec![run];
        if let Some(decorations) = &self.decorations {
            spec = decorations.project(spec, value).spec;
        }
        let paragraph = text.paragraph(&spec, self.multiline.then_some(self.content.2));
        Self::new(
            &paragraph,
            value,
            spec,
            self.content,
            self.multiline,
            self.masked,
            self.transform,
            self.decorations.clone(),
        )
    }

    pub(crate) fn hit(&self, x: f32, y: f32) -> Option<Cursor> {
        let mut point = tiny_skia::Point::from_xy(x, y);
        self.transform.invert()?.map_point(&mut point);
        Some(
            self.geometry
                .hit(point.x - self.origin.0, point.y - self.origin.1),
        )
    }

    pub(super) fn place(&self, r: Rect4) -> Rect4 {
        (r.0 + self.origin.0, r.1 + self.origin.1, r.2, r.3)
    }
}

impl PaintedBox {
    /// A viewport point in the island where the field was painted. Its
    /// affine transform is inverted by `PaintedField::hit` afterwards.
    pub(crate) fn field_point(&self, x: f32, y: f32) -> (f32, f32) {
        self.projective
            .map_or((x, y), |(inv, ..)| crate::placement::map(&inv, x, y))
    }
}

impl Painter {
    /// Paint `node`, a text field, in `rect`: `surface` is its box as
    /// presented, `content` its content box and `outer` its border shape.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn text_input(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        rect: Rect4,
        surface: Rect4,
        content: Rect4,
        outer: &Shape,
        ts: Transform,
    ) {
        let hosted = crate::fields::hosts(node);
        // A row does not keep a caret, nor a focus ring, which comes and
        // goes with no change of the tree: a hosted field has one too.
        self.row_refuse();
        let native = node.style.appearance != exact_kernel::Appearance::None;
        let disabled = node.props.bool(PropId::Disabled) == Some(true);
        let focused = walk.scene.focus == Some(node.id);
        let field_shape = native.then(|| self.text_field_chrome(node, surface, focused, ts));
        // Material's field shows its focus by its indicator, not a ring.
        let ringed = focused
            && !node.style.mask.has(exact_kernel::StyleId::OutlineStyle)
            && (!native || look::neutral());
        let content = if let Some(c) = node.field_content_rect() {
            (rect.0 + c.x, rect.1 + c.y, c.width, c.height)
        } else {
            content
        };
        // Typed text its bound value has not replaced (LLP 1069.001 D4).
        let value = control::choice(node, walk.scene.chosen.get(&node.id))
            .unwrap_or_else(|| node.props.str(PropId::Value).unwrap_or(""));
        // Material's field floats its placeholder as its label (`material.rs`).
        let labelled = native
            && !look::neutral()
            && self.field_label(node, surface, focused, value.is_empty(), ts);
        if hosted {
            let within = [content.0 - rect.0, content.1 - rect.1, content.2, content.3];
            self.field_content.insert(node.id, within);
            if content.2 > 0.0 && content.3 > 0.0 {
                let field = NativeKind::Field;
                self.backend
                    .native(node.id, field, &Shape::rect(content), ts, "", false);
            }
            if ringed {
                let accent = control::ring(node, self.dark);
                self.field_ring(field_shape.as_ref().unwrap_or(outer), accent, ts);
            }
            return;
        }
        let placeholder = value.is_empty();
        // A password is masked, one bullet a character, as the web and
        // Apple's secure fields draw it.
        let masked;
        let shown = if labelled && placeholder {
            ""
        } else if placeholder {
            node.props.str(PropId::Placeholder).unwrap_or("")
        } else if node.props.str(PropId::Type) == Some("password") {
            masked = "\u{2022}".repeat(value.chars().count());
            &masked
        } else {
            value
        };
        let mut computed = node.computed_style(StyleMask::INHERITED);
        // A field's value is never collapsed, as the kernel measures it.
        if !computed.white_space.model().preserves() {
            computed.white_space = exact_kernel::WhiteSpace::PreWrap;
        }
        let spec = text_spec(&computed, shown);
        let multiline = node.props.str(PropId::SemanticTag) == Some("textarea");
        let decorations = (!placeholder && multiline)
            .then(|| {
                FieldDecorations::read(
                    node.props.str(PropId::Decorations)?,
                    node.props.str(PropId::DecorationStyles)?,
                    walk.scene.kernel.root_font_size(),
                    walk.scene.kernel.display_scale(),
                )
                .ok()
                .map(Rc::new)
            })
            .flatten();
        let (spec, inks, backgrounds) = match &decorations {
            Some(decorations) => {
                let projected = decorations.project(spec, shown);
                (
                    projected.spec,
                    Some(projected.inks),
                    Some(projected.backgrounds),
                )
            }
            None => (spec, None, None),
        };
        let paragraph = self.text.borrow_mut().paragraph_replacing(
            (node.id, 0),
            &spec,
            multiline.then_some(content.2),
        );
        walk.text.insert(node.key, paragraph.clone());
        let oy = content.1
            + if multiline {
                0.0
            } else {
                (content.3 - paragraph.height) / 2.0
            };
        // The field names its inks by role: the placeholder's, and the grey
        // of a disabled one (the neutral look's, LLP 1115.000 D17, and
        // Material's `onSurfaceVariant` and `onSurface` at 38 %).
        // The value's ink: a native field's own colour, moving or pressed,
        // else its start's, the design's FieldText: an inherited colour, or
        // an ancestor's press, never tints it (LLP 1115 D4, LLP 1121 D9). A
        // field of the author's own inherits as text does. The caret takes
        // it too.
        let value_ink = if native {
            super::inline::own_presented_color(walk, node, self.dark)
        } else {
            presented_color(walk, node, self.dark)
        }
        .unwrap_or_else(|| rgba(node.text_color().resolve(self.dark)));
        let mut ink = if placeholder {
            // An authored `::placeholder` colour, its alpha its own (LLP 1104 D3).
            control::authored_placeholder(node.style, self.dark)
                .unwrap_or_else(|| look::role("-exact-placeholder", self.dark))
        } else {
            value_ink
        };
        if native
            && disabled
            && !node.style.mask.has(if placeholder {
                exact_kernel::StyleId::PlaceholderColor
            } else {
                exact_kernel::StyleId::TextColor
            })
        {
            ink = look::role("GrayText", self.dark);
        }
        // A single line is shaped unconstrained, so `text-align` places
        // it here; a textarea's paragraph is laid out at the field's
        // width and aligns its own lines.
        let ox = content.0
            + if multiline {
                0.0
            } else {
                line_left(
                    computed.text_align,
                    computed.direction,
                    content.2,
                    paragraph.width,
                )
            };
        let base = RunPaint {
            color: ink,
            source: node.id,
        };
        let palette: Cow<'_, [RunPaint]> = match inks {
            Some(inks) => Cow::Owned(
                inks.iter()
                    .map(|color| RunPaint {
                        color: color.map_or(ink, |color| rgba(color.resolve(self.dark))),
                        source: node.id,
                    })
                    .collect(),
            ),
            None => Cow::Borrowed(std::slice::from_ref(&base)),
        };
        let field = PaintedField::new(
            &paragraph,
            value,
            spec,
            content,
            multiline,
            node.props.str(PropId::Type) == Some("password"),
            ts,
            decorations,
        );
        let selection = walk.scene.selection.filter(|_| focused);
        if let Some(backgrounds) = backgrounds {
            let backgrounds: Vec<_> = backgrounds
                .iter()
                .enumerate()
                .map(|(index, color)| {
                    color.map(|color| {
                        color.map_or(palette[index].color, |color| rgba(color.resolve(self.dark)))
                    })
                })
                .collect();
            for (r, color) in paragraph.run_backgrounds(&backgrounds) {
                self.backend
                    .fill(&Shape::rect((ox + r.0, oy + r.1, r.2, r.3)), color, ts);
            }
        }
        if let Some(s) = selection {
            self.field_highlight(&field, s);
        }
        if field.decorations.is_some() {
            self.text_decorations(
                walk.scene.kernel,
                &paragraph,
                &palette,
                (ox, oy),
                ts,
                walk.reveal,
            );
        }
        {
            let mut engine = self.text.borrow_mut();
            self.backend
                .text(&mut engine, &paragraph, &palette, (ox, oy), ts);
        }
        if focused {
            let caret = computed
                .caret_color
                .map_or(value_ink, |c| rgba(c.resolve(self.dark)));
            let at_end = || exact_runner::FieldSelection::at_end(value);
            self.field_caret(
                &field,
                caret,
                selection.unwrap_or_else(at_end),
                walk.scene.caret_upstream,
            );
            // LLP 1104 D6: `outline: none` (the row's one value) draws no ring.
            if ringed {
                let accent = control::ring(node, self.dark);
                self.field_ring(field_shape.as_ref().unwrap_or(outer), accent, ts);
            }
        }
        walk.fields.insert(node.id, Rc::new(field));
    }
}
