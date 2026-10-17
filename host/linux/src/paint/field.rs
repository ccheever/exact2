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
        let mut ink = if placeholder {
            // An authored `::placeholder` colour, its alpha its own (LLP 1104 D3).
            control::authored_placeholder(node.style, self.dark)
                .unwrap_or_else(|| look::role("-exact-placeholder", self.dark))
        } else {
            presented_color(walk, node)
                .unwrap_or_else(|| rgba(node.text_color().resolve(self.dark)))
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
        // The focused field's selection (x2apps codeedit #2).
        let field = caret::FieldText {
            node: node.id,
            style: &computed,
            value,
            masked: node.props.str(PropId::Type) == Some("password"),
            origin: (content.0, oy),
            width: content.2,
            multiline,
        };
        // A single line is shaped unconstrained, so `text-align` places
        // it here; a textarea's paragraph is laid out at the field's
        // width and aligns its own lines.
        let ox = content.0
            + if multiline {
                0.0
            } else {
                caret::line_left(&computed, content.2, paragraph.width)
            };
        let selection = walk.scene.selection.filter(|_| focused);
        if let Some(s) = selection {
            self.field_highlight(&field, s, ts);
        }
        {
            let mut engine = self.text.borrow_mut();
            self.backend.text(
                &mut engine,
                &paragraph,
                &[RunPaint {
                    color: ink,
                    source: node.id,
                }],
                (ox, oy),
                ts,
            );
        }
        if focused {
            let caret = rgba(
                computed
                    .caret_color
                    .unwrap_or(node.text_color())
                    .resolve(self.dark),
            );
            let at_end = || exact_runner::FieldSelection::at_end(value);
            self.field_caret(&field, caret, selection.unwrap_or_else(at_end), ts);
            // LLP 1104 D6: `outline: none` (the row's one value) draws no ring.
            if ringed {
                let accent = control::ring(node, self.dark);
                self.field_ring(field_shape.as_ref().unwrap_or(outer), accent, ts);
            }
        }
    }
}
