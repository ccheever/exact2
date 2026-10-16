//! Buttons native by default, with one static admission rule (LLP 1104 D1, D2).
//! Reuse the explicit native check; a default refusal becomes a bare box.

use crate::{err, tags::AttrTarget, ButtonSite, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Node, Span};
use exact_kernel::StyleId;

fn rows(a: &Attr) -> Vec<StyleId> {
    match crate::tags::attr_valued(&a.name, &a.value) {
        Some(AttrTarget::Styles(ids)) => ids.to_vec(),
        Some(AttrTarget::Shorthand) => crate::shorthands::rows(&a.name).to_vec(),
        Some(AttrTarget::Flex) => vec![StyleId::FlexGrow, StyleId::FlexShrink, StyleId::FlexBasis],
        _ => Vec::new(),
    }
}

fn layout(id: StyleId) -> bool {
    use StyleId::*;
    matches!(
        id,
        Width
            | Height
            | MinWidth
            | MinHeight
            | MaxWidth
            | MaxHeight
            | MarginTop
            | MarginRight
            | MarginBottom
            | MarginLeft
            | AlignSelf
            | FlexGrow
            | FlexShrink
            | FlexBasis
            | PositionType
            | Top
            | Right
            | Bottom
            | Left
            | AspectRatio
            | ZIndex
            | GridColumn
            | GridRow
    )
}

/// The control's tint, which the platform applies to its own button (LLP 1104
/// D3's table: `tintColor`), so it preserves no authored appearance.
fn tint(id: StyleId) -> bool {
    id == StyleId::AccentColor
}

impl Lowerer<'_> {
    /// Decide from authored class-then-own rows, before any grouped-list UA
    /// sheet. A sheet is not author CSS and cannot disable native appearance.
    pub(crate) fn button_appearance(
        &mut self,
        tag: &str,
        class: &[Attr],
        own: &[Attr],
        children: &[Node],
        context: Option<&str>,
        span: Span,
    ) -> Result<Option<ButtonSite>, LowerError> {
        if tag != "button" {
            return Ok(None);
        }
        let attrs: Vec<_> = class.iter().chain(own).cloned().collect();
        let explicit = match attrs.iter().rev().find(|a| a.name == "appearance") {
            None => None,
            Some(a) => match &a.value {
                Expr::Str(v, _) if v == "auto" || v == "none" => Some(v.as_str()),
                _ => return err("lower-appearance", "a `button`'s `appearance` is a literal: `\"auto\"` (the platform's own button) or `\"none\"` (the bare box). To switch between them, write `if` with two buttons", a.span),
            },
        };
        if explicit == Some("none") {
            return Ok(Some(ButtonSite {
                native: false,
                bare_reason: None,
                migrate: false,
            }));
        }
        let face = crate::grouped::unsheet(children);
        let children = face.as_deref().unwrap_or(children);
        let composed = self.compose_animation(&attrs)?;
        let checked = composed.as_deref().unwrap_or(&attrs);
        let disabled = attrs.iter().find_map(|a| {
            rows(a)
                .into_iter()
                .find(|id| crate::fields::disables(*id))
                .map(|id| {
                    let longhand = crate::vocab::attrs()
                        .into_iter()
                        .find_map(|(name, target)| {
                            matches!(target, AttrTarget::Styles(ids) if ids == [id])
                                .then_some(name.to_string())
                        })
                        .expect("a disabling row has a CSS longhand");
                    // The reason names the longhand; the edit, what the author wrote.
                    (longhand, a.name.clone())
                })
        });
        if explicit == Some("auto") {
            self.check_native_button(checked, children, span)?;
            return Ok(Some(ButtonSite {
                native: true,
                bare_reason: None,
                migrate: false,
            }));
        }
        let bare = if let Some(context) = context {
            Some((Bare::Place, context.to_owned()))
        } else if let Some((longhand, written)) = disabled {
            Some((Bare::Drops(written), longhand))
        } else {
            self.check_native_button(checked, children, span)
                .err()
                .map(|e| {
                    let kind = match e.id {
                        "lower-button-content" | "lower-button-style-attr" => Bare::Face,
                        "lower-button-style" => Bare::Style,
                        _ => Bare::Place,
                    };
                    (kind, e.message)
                })
        };
        if let Some((kind, reason)) = &bare {
            if let Some(style) = attrs.iter().find(|a| a.name == "buttonStyle") {
                return err(
                    "lower-button-style",
                    button_style_edit(kind, reason),
                    style.span,
                );
            }
        }
        let bare_reason = bare.map(|(_, reason)| reason);
        let native = bare_reason.is_none();
        let migrate = native
            && self.sites.is_some()
            && (attrs
                .iter()
                .any(|a| rows(a).iter().any(|id| !layout(*id) && !tint(*id)))
                || self.styled_button_face(children)?);
        Ok(Some(ButtonSite {
            native,
            bare_reason,
            migrate,
        }))
    }

    fn styled_button_face(&self, nodes: &[Node]) -> Result<bool, LowerError> {
        for node in nodes {
            let styled = match node {
                Node::Element { attrs, .. } => {
                    let class = self
                        .class_rows(attrs)?
                        .map(|(_, rows)| rows)
                        .unwrap_or_default();
                    class.iter().chain(attrs).any(|a| !rows(a).is_empty())
                }
                Node::When {
                    then, otherwise, ..
                } => self.styled_button_face(then)? || self.styled_button_face(otherwise)?,
                Node::Match { some, none, .. } => {
                    self.styled_button_face(&some.1)? || self.styled_button_face(none)?
                }
                _ => false,
            };
            if styled {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Why a default button is bare (LLP 1116.014 D1): the edit that fixes it
/// depends on which.
enum Bare {
    /// The face or an attribute on it: a native button's own.
    Face,
    /// An attribute of the button, as written, that CSS UI 4 §7.2.1 says
    /// drops the platform's appearance.
    Drops(String),
    /// Where it stands, or an attribute a native button lacks.
    Place,
    /// `buttonStyle` itself, which names its fix.
    Style,
}

/// A `buttonStyle` on a bare button: the edit that keeps the native button,
/// when there is one, then the edit that makes the custom face knowingly
/// custom, then why it is bare.
fn button_style_edit(kind: &Bare, reason: &str) -> String {
    let custom =
        "for a custom face, remove `buttonStyle` (or write `appearance=\"none\"` without it)";
    match kind {
        // The reason leads: for the face it names the edit that keeps what
        // the attribute was for (LLP 1116.018).
        Bare::Face => format!(
            "{reason}. A native button's face is `text`s and a symbol `image`, sized by `font-size`; {custom}"
        ),
        Bare::Drops(written) => format!(
            "to keep the native button, remove `{written}` (it drops the platform's appearance); {custom}"
        ),
        Bare::Place => format!("{custom}. This button is bare: {reason}"),
        Bare::Style => reason.to_owned(),
    }
}
