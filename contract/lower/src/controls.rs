//! Form controls by their HTML names (LLP 1069.001, LLP 1069.002): which
//! control an element is, the tag it lowers with, HTML's content model for
//! `select` and `option`, and the bare words and derived rows a control
//! carries.

use crate::tags::Tag;
use crate::{err, LowerError, Lowerer};
use contract_syntax::{Expr, Span};
use exact_kernel::{NodeType, PropId, StyleId};
use exact_plan::{BindingKind, BindingsRow, Value};

/// The form control an element is (LLP 1069.001 D1), refusing a bound
/// `type`: the node type is chosen when the view compiles, from the literal.
/// A `select` is one; it takes no `type`.
pub(crate) fn control(
    tag: &str,
    attrs: &[contract_syntax::Attr],
) -> Result<Option<&'static str>, LowerError> {
    if tag == "select" {
        if let Some(a) = attrs
            .iter()
            .find(|a| matches!(a.name.as_str(), "type" | "checked"))
        {
            return err(
                "lower-attr-tag",
                format!(
                    "`select` takes no `{}`: its options are its choices",
                    a.name
                ),
                a.span,
            );
        }
        return Ok(Some("select"));
    }
    if tag != "input" {
        if let Some(a) = attrs.iter().find(|a| a.name == "checked") {
            return err(
                "lower-attr-tag",
                format!("`checked` belongs to `input type=\"checkbox\"`, not `{tag}`"),
                a.span,
            );
        }
        return Ok(None);
    }
    if let Some(a) = attrs.iter().find(|a| a.name == "type") {
        if !matches!(a.value, Expr::Str(..)) {
            return err(
                "lower-input-type",
                "`input`'s `type` is a literal (`type=\"text\"`, `\"password\"`, `\"checkbox\"`, …): it picks the kind of node when the view compiles",
                a.span,
            );
        }
    }
    let control = contract_syntax::input_control(tag, attrs);
    if control != Some("checkbox") {
        if let Some(a) = attrs.iter().find(|a| a.name == "checked") {
            return err(
                "lower-attr-tag",
                "`checked` belongs to `input type=\"checkbox\"`; a text field's is `value`",
                a.span,
            );
        }
    }
    if control == Some("file") {
        file_input(attrs)?;
    } else if let Some(a) = attrs
        .iter()
        .find(|a| matches!(a.name.as_str(), "accept" | "multiple" | "capture"))
    {
        return err(
            "lower-attr-tag",
            format!("`{}` belongs to `input type=\"file\"`", a.name),
            a.span,
        );
    }
    Ok(control)
}

/// The sentence of `rules/DEFERRED.md` that bounds the picker (LLP 1069.002
/// D1), cited by each refusal.
pub const PICKER_ADMISSION: &str = "rules/DEFERRED.md admits an image/video picker, widened to the types the app's `file_handlers` declare: \"Still no picker for any file, and no camera.\"";

/// `input type="file"` (LLP 1069.002 D1): `accept` is a literal list of
/// `image/*`, `video/*`, `image/<subtype>`, `video/<subtype>`, or a MIME
/// type or extension the manifest's `file_handlers` declares (checked at
/// bake, where the manifest is read: `contract::picker`); `capture` is
/// refused, and so are `*/*` and an empty list.
fn file_input(attrs: &[contract_syntax::Attr]) -> Result<(), LowerError> {
    if let Some(a) = attrs.iter().find(|a| a.name == "capture") {
        return err(
            "lower-picker-capture",
            format!("`capture` opens a camera, which is not admitted: {PICKER_ADMISSION}"),
            a.span,
        );
    }
    let Some(a) = attrs.iter().find(|a| a.name == "accept") else {
        return err(
            "lower-picker-accept",
            format!("`input type=\"file\"` needs a literal `accept` (`accept=\"image/*\"`): {PICKER_ADMISSION}"),
            attrs.iter().find(|a| a.name == "type").map_or_else(Default::default, |a| a.span),
        );
    };
    let Expr::Str(list, _) = &a.value else {
        return err(
            "lower-picker-accept",
            format!("`accept` is a literal, so the bake can bound it: {PICKER_ADMISSION}"),
            a.span,
        );
    };
    let tokens = accept_tokens(list);
    if tokens.is_empty() {
        return err(
            "lower-picker-accept",
            format!("`accept` names no type: {PICKER_ADMISSION}"),
            a.span,
        );
    }
    for t in tokens {
        let wild = t
            .split_once('/')
            .is_some_and(|(k, s)| s == "*" && k != "image" && k != "video");
        if t == "*" || t == "*/*" || wild || !(t.contains('/') || t.starts_with('.')) {
            return err(
                "lower-picker-accept",
                format!("`accept` may not name `{t}`: {PICKER_ADMISSION}"),
                a.span,
            );
        }
    }
    Ok(())
}

/// `accept`'s comma-separated tokens, trimmed and lowercased, as HTML reads them.
pub fn accept_tokens(list: &str) -> Vec<String> {
    list.split(',')
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Whether an `accept` token is a media type every app may pick (LLP
/// 1069.002 D1): `image/*`, `video/*`, or one image or video subtype.
pub fn media_accept(token: &str) -> bool {
    token.split_once('/').is_some_and(|(kind, sub)| {
        matches!(kind, "image" | "video")
            && !sub.is_empty()
            && sub
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"*.+-_".contains(&b))
    })
}

/// The tag an `input` control lowers with (a `select` keeps its own).
pub(crate) fn tag(kind: &str, t: Tag) -> Tag {
    match kind {
        // A file input (LLP 1069.002 D1): the kernel's `Control`, a measured
        // leaf the host presents as its own control (the browser's "Choose
        // File"); no margins, as Chrome's UA sheet gives `input[type=file]`
        // none.
        "file" => Tag {
            node_type: NodeType::Control,
            fixed_styles: &[],
            fixed_props: &[],
            positional: None,
        },
        // A checkbox: the margins Chrome's UA sheet gives
        // `input[type=checkbox]` (`3px 3px 3px 4px`) and ARIA's role;
        // `switch` or `role="switch"` replaces the role (LLP 1069.001 D1, D3).
        "checkbox" => Tag {
            node_type: NodeType::Control,
            fixed_styles: &[
                (StyleId::MarginTop, "3"),
                (StyleId::MarginRight, "3"),
                (StyleId::MarginBottom, "3"),
                (StyleId::MarginLeft, "4"),
            ],
            fixed_props: &[(PropId::AccessibilityRole, "checkbox")],
            positional: None,
        },
        // A range: Chrome's UA margin (`2px`) and ARIA's role.
        "range" => Tag {
            node_type: NodeType::Control,
            fixed_styles: &[
                (StyleId::MarginTop, "2"),
                (StyleId::MarginRight, "2"),
                (StyleId::MarginBottom, "2"),
                (StyleId::MarginLeft, "2"),
            ],
            fixed_props: &[(PropId::AccessibilityRole, "slider")],
            positional: None,
        },
        _ => t,
    }
}

/// A range's `value`, `min`, `max` and `step` as HTML's strings: a number
/// literal is written as one, a bound number through `toString` (LLP
/// 1069.001 D4: the props are strings on the wire, typed per control).
/// `None` when nothing needs rewriting.
pub(crate) fn range_attrs(
    control: Option<&str>,
    attrs: &[contract_syntax::Attr],
) -> Option<Vec<contract_syntax::Attr>> {
    let numeric = |a: &contract_syntax::Attr| {
        matches!(a.name.as_str(), "value" | "min" | "max" | "step")
            && !matches!(a.value, Expr::Str(..))
    };
    if control != Some("range") || !attrs.iter().any(numeric) {
        return None;
    }
    Some(
        attrs
            .iter()
            .map(|a| {
                if !numeric(a) {
                    return a.clone();
                }
                let value = match &a.value {
                    Expr::Number(n, span) => Expr::Str(exact_num::Shortest(*n).to_string(), *span),
                    e => Expr::Call("toString".into(), vec![e.clone()], e.span()),
                };
                contract_syntax::Attr { value, ..a.clone() }
            })
            .collect(),
    )
}

/// `option` belongs in a `select`, and a `select` holds only `option`s
/// (directly, or through `each`, `when` and `match`, which are not
/// elements): HTML's content model, which every host's menu reads.
pub(crate) fn check_nesting(tag: &str, parent: Option<&str>, span: Span) -> Result<(), LowerError> {
    if tag == "option" && parent != Some("select") {
        return err(
            "lower-option-parent",
            "`option` belongs in a `select`",
            span,
        );
    }
    if parent == Some("select") && tag != "option" {
        return err(
            "lower-option-parent",
            format!("a `select` holds `option`s, not `{tag}`"),
            span,
        );
    }
    Ok(())
}

impl Lowerer<'_> {
    /// A control's bare word — HTML's boolean `switch` on a checkbox (LLP
    /// 1069.001 D1), `multiple` on a file input (LLP 1069.002 D1) — as its
    /// prop; `false` when `word` is not one.
    pub(crate) fn control_word(
        &mut self,
        tag: &str,
        word: &Expr,
        control: Option<&str>,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<bool, LowerError> {
        let (name, owner, id) = if contract_syntax::is_input_switch(tag, word) {
            // The checkbox is drawn as a switch, and ARIA hears one either way.
            ("switch", "checkbox", PropId::AccessibilityRole)
        } else if contract_syntax::is_input_multiple(tag, word) {
            ("multiple", "file", PropId::Multiple)
        } else {
            return Ok(false);
        };
        if control != Some(owner) {
            return err(
                "lower-attr-tag",
                format!("`{name}` belongs to `input type=\"{owner}\"`"),
                word.span(),
            );
        }
        let expr = if owner == "file" {
            self.b.constant(&Value::Bool(true))
        } else {
            self.fixed(false, "switch")
        };
        bindings.push(BindingsRow {
            kind: BindingKind::Prop,
            id: id as u16,
            expr,
        });
        Ok(true)
    }
}

/// A control's derived rows: a checkbox's checked state is its
/// accessibility state on every host (LLP 1069.001 D8). Whether a row was
/// added.
pub(crate) fn derived_rows(bindings: &mut Vec<BindingsRow>) -> bool {
    let checked = PropId::Checked as u16;
    let Some(expr) = bindings
        .iter()
        .rev()
        .find(|b| b.kind == BindingKind::Prop && b.id == checked)
        .map(|b| b.expr)
    else {
        return false;
    };
    bindings.push(BindingsRow {
        kind: BindingKind::Prop,
        id: PropId::AccessibilityChecked as u16,
        expr,
    });
    true
}
