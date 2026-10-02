//! Native modules (LLP 1024 D1): a hyphenated tag is a custom element. It
//! lowers to a `NativeView` whose module name is the tag and whose leftover
//! attributes are one JSON object, replaced whole on change.
//!
//! @ref LLP 1024 D1 (HTML's potential-custom-element-name, lowercase only,
//! the SVG/MathML reserved names refused; a fixed `display: block` row; the
//! known attribute table binds first and its renamed spellings stay refused;
//! an SVG element's own props, `svg::svg_only_prop`, stay the module's; a
//! known name the leaf box never uses, a text row or a form control's, is
//! refused by name rather than bound to nothing, `refused`)

use crate::tags::{self, Tag};
use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, File, Node, Span};
use contract_types::{Scope, Ty};
use exact_kernel::{NodeType, PropId, StyleId};
use exact_plan::asm::Asm;
use exact_plan::{BindingKind, BindingsRow};

/// HTML's reserved hyphenated names (SVG and MathML elements).
const RESERVED: [&str; 8] = [
    "annotation-xml",
    "color-profile",
    "font-face",
    "font-face-src",
    "font-face-uri",
    "font-face-format",
    "font-face-name",
    "missing-glyph",
];

/// Whether `name` is a native module tag: `[a-z][a-z0-9_]*(-[a-z][a-z0-9_]*)+`
/// and not reserved. The same string is the source spelling, the DOM tag
/// and the module table's key; the hosts check it again on plan bytes.
pub fn is_module_tag(name: &str) -> bool {
    let word = |w: &str| {
        let mut chars = w.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    name.contains('-') && name.split('-').all(word) && !RESERVED.contains(&name)
}

/// The tag a module name lowers to: a leaf `NativeView` that declares
/// `display: block`. A custom element has no UA rule and would be inline in
/// the browser; the row makes the web and the native box agree, and an
/// authored `display` still wins.
pub(crate) fn tag(name: &str) -> Option<Tag> {
    is_module_tag(name).then_some(Tag {
        node_type: NodeType::NativeView,
        fixed_styles: &[(StyleId::Display, "block")],
        fixed_props: &[],
        positional: None,
    })
}

/// Whether an attribute on a module tag is a leftover: a module prop, not a
/// row of the table. A renamed spelling (`fontSize`) is still refused with
/// its CSS name, and `class` is the style mechanism.
pub(crate) fn leftover(tag: &str, a: &Attr) -> bool {
    is_module_tag(tag)
        && a.name != "class"
        && (tags::attr(&a.name).is_none() || crate::svg::svg_only_prop(&a.name))
        && tags::renamed(&a.name).is_none()
}

/// A known attribute a module tag cannot use, refused by name. The table
/// binds before the module (LLP 1024 D1), so `appearance="dark"` on
/// `ghostty-terminal` would set a style row the leaf box never draws and
/// the module would never see the prop: rows of a text leaf (the schema's
/// text mask, `color`, `text-decoration-line`) and of a form control
/// (LLP 1069.001 D6's `appearance`, `accent-color`, `caret-color`, and the
/// control props) are refused on a module tag, naming what the word means.
/// Layout, box and paint rows, the handlers, `testId`, `id` and ARIA stay
/// the box's: the author sizes and places a module like any leaf.
pub(crate) fn refused(tag: &str, a: &Attr) -> Option<LowerError> {
    if !is_module_tag(tag) || leftover(tag, a) {
        return None;
    }
    let what = match tags::attr(&a.name)? {
        tags::AttrTarget::Styles(rows) => {
            let text = |r: &StyleId| {
                r.affects_text() || matches!(r, StyleId::TextColor | StyleId::TextDecorationLine)
            };
            let control = |r: &StyleId| {
                matches!(
                    r,
                    StyleId::Appearance | StyleId::AccentColor | StyleId::CaretColor
                )
            };
            if rows.iter().any(control) {
                "a form control's row"
            } else if rows.iter().any(text) {
                "a text row"
            } else {
                return None;
            }
        }
        _ if CONTROL_PROPS.contains(&a.name.as_str()) => "a form control's prop",
        _ => return None,
    };
    Some(LowerError {
        id: "lower-native-attr",
        message: format!(
            "`{}` on `{tag}` is {what}, which a native module's box never uses; a known attribute binds to the box, not the module (LLP 1024 D1), so give the module prop another name",
            a.name
        ),
        span: a.span,
    })
}

/// The form-control props (LLP 1069.001, LLP 1069.002) a module's box has
/// no use for. `checked` is refused on every tag but `input` before this
/// runs (`controls::control`); it is listed so the set reads whole.
const CONTROL_PROPS: [&str; 17] = [
    "value",
    "placeholder",
    "autofocus",
    "type",
    "min",
    "max",
    "step",
    "accept",
    "multiple",
    "readonly",
    "inputmode",
    "autocapitalize",
    "autocorrect",
    "spellcheck",
    "markup",
    "emojiPicker",
    "checked",
];

/// Every module tag the file's views name, with where: the driver checks
/// them against the app's roster (`bake-unknown-module`).
pub fn module_tags(file: &File) -> Vec<(String, Span)> {
    fn walk(nodes: &[Node], out: &mut Vec<(String, Span)>) {
        for n in nodes {
            match n {
                Node::Element {
                    tag,
                    children,
                    span,
                    ..
                } => {
                    if is_module_tag(tag) {
                        out.push((tag.clone(), *span));
                    }
                    walk(children, out);
                }
                Node::Use { children, .. } => walk(children, out),
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, out);
                    walk(otherwise, out);
                }
                Node::Each { body, .. } => walk(body, out),
                Node::Match { some, none, .. } => {
                    walk(&some.1, out);
                    walk(none, out);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut out);
    }
    out
}

impl Lowerer<'_> {
    /// A module node's two props: its name, and its leftover attributes as
    /// one object — keys sorted, the last binding of a key winning, each
    /// value a string, number or bool (or an option of one, absent when
    /// none), evaluated with the node's bindings.
    pub(crate) fn native_bindings(
        &mut self,
        tag: &str,
        leftovers: &[&Attr],
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        bindings.push(BindingsRow {
            kind: BindingKind::Prop,
            id: PropId::NativeViewModuleName as u16,
            expr: self.b.constant(&exact_plan::Value::str(tag)),
        });
        let mut keyed: std::collections::BTreeMap<&str, &Attr> = Default::default();
        for a in leftovers {
            keyed.insert(a.name.as_str(), a);
        }
        let mut asm = Asm::new();
        let mut depth = locals;
        for (key, a) in &keyed {
            asm.str(self.b.str(key));
            let ty = crate::expr::compile(self, &mut asm, &a.value, scope, &mut depth)?;
            let scalar = |t: &Ty| matches!(t, Ty::String | Ty::Number | Ty::Bool | Ty::Unknown);
            let admitted = match &ty {
                Ty::Option(inner) => scalar(inner),
                t => scalar(t),
            };
            if !admitted {
                return err(
                    "lower-native-prop",
                    format!(
                        "`{key}` on `{tag}` is a module prop: a string, number or bool, or an option of one"
                    ),
                    a.span,
                );
            }
        }
        asm.native_props(keyed.len() as u32);
        bindings.push(BindingsRow {
            kind: BindingKind::Prop,
            id: PropId::NativeViewProps as u16,
            expr: self.b.code(asm),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::is_module_tag;

    /// SVG elements' own props (LLP 1055.000's filter primitives among them)
    /// stay a module's props; rows still bind first.
    #[test]
    fn svg_element_props_stay_a_modules_on_its_tag() {
        let attr = |name: &str| contract_syntax::Attr {
            name: name.into(),
            value: contract_syntax::Expr::Ident("v".into(), Default::default()),
            span: Default::default(),
        };
        for name in [
            "mode", "seed", "values", "in", "viewBox", "offset", "orient",
        ] {
            assert!(super::leftover("exact-fixture", &attr(name)), "{name}");
        }
        for name in ["width", "fill", "x", "filter"] {
            assert!(!super::leftover("exact-fixture", &attr(name)), "{name}");
        }
    }

    /// A row the leaf box never uses is refused on a module tag, by name;
    /// the box's own rows, the events and the module's props are not.
    #[test]
    fn a_text_or_control_row_on_a_module_tag_is_refused_by_name() {
        let attr = |name: &str| contract_syntax::Attr {
            name: name.into(),
            value: contract_syntax::Expr::Ident("v".into(), Default::default()),
            span: Default::default(),
        };
        for name in [
            "appearance",
            "accent-color",
            "caret-color",
            "font-size",
            "font-family",
            "color",
            "line-height",
            "text-align",
            "white-space",
            "text-decoration-line",
            "value",
            "placeholder",
            "autofocus",
            "type",
            "readonly",
            "inputmode",
            "spellcheck",
            "markup",
        ] {
            let e = super::refused("ghostty-terminal", &attr(name))
                .unwrap_or_else(|| panic!("{name} was not refused"));
            assert_eq!(e.id, "lower-native-attr");
            assert!(
                e.message
                    .starts_with(&format!("`{name}` on `ghostty-terminal` is ")),
                "{}",
                e.message
            );
        }
        for name in [
            "width",
            "height",
            "flex",
            "min-height",
            "padding",
            "margin-top",
            "background-color",
            "opacity",
            "border-radius",
            "display",
            "position",
            "z-index",
            "transition",
            "testId",
            "id",
            "aria-label",
            "role",
            "inert",
            "load",
            "press",
            "change",
            "message",
            "scheme",
            "mode",
            "seed",
            "photo",
            "class",
            "fontSize",
        ] {
            assert!(
                super::refused("ghostty-terminal", &attr(name)).is_none(),
                "{name}"
            );
        }
        assert!(super::refused("text", &attr("appearance")).is_none());
        assert!(super::refused("input", &attr("value")).is_none());
    }

    #[test]
    fn the_admission_is_lowercase_pcen_without_the_reserved_names() {
        for ok in [
            "ghostty-terminal",
            "photo-editor",
            "exact-fixture",
            "a-b-c",
            "x1-y_2",
        ] {
            assert!(is_module_tag(ok), "{ok}");
        }
        for no in [
            "text",
            "Ghostty-Terminal",
            "photo-Editor",
            "-lead",
            "trail-",
            "a--b",
            "1a-b",
            "font-face",
            "annotation-xml",
            "a-b.c",
        ] {
            assert!(!is_module_tag(no), "{no}");
        }
    }
}
