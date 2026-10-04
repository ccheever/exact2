//! Form controls by their HTML names (LLP 1069.001, LLP 1069.002): which
//! control an element is, the tag it lowers with, HTML's content model for
//! `select` and `option`, and the bare words and derived rows a control
//! carries.

use crate::tags::Tag;
use crate::{err, LowerError, Lowerer};
use contract_syntax::{Expr, Span};
use exact_kernel::{NodeType, PropId, StyleId};
use exact_plan::{BindingKind, BindingsRow, Value};

/// HTML's `input type` is ASCII-case-insensitive. Keep the prop every host
/// reads in its canonical lowercase spelling as well as classifying it that
/// way, including the literal arms of the admitted text-field choice.
pub(crate) fn canonical_type_attrs(
    tag: &str,
    attrs: &[contract_syntax::Attr],
) -> Option<Vec<contract_syntax::Attr>> {
    if tag != "input" {
        return None;
    }
    let type_attr = attrs.iter().find(|a| a.name == "type")?;
    let value = canonical_type_expr(&type_attr.value)?;
    (value != type_attr.value).then(|| {
        attrs
            .iter()
            .map(|a| {
                if a.name == "type" {
                    contract_syntax::Attr {
                        value: value.clone(),
                        ..a.clone()
                    }
                } else {
                    a.clone()
                }
            })
            .collect()
    })
}

fn canonical_type_expr(value: &Expr) -> Option<Expr> {
    match value {
        Expr::Str(kind, span) => Some(Expr::Str(kind.to_ascii_lowercase(), *span)),
        Expr::Ternary(cond, yes, no, span) => Some(Expr::Ternary(
            cond.clone(),
            Box::new(canonical_type_expr(yes)?),
            Box::new(canonical_type_expr(no)?),
            *span,
        )),
        _ => None,
    }
}

/// The form control an element is (LLP 1069.001 D1), refusing a bound
/// `type` that could name a control: the node type is chosen when the view
/// compiles, from the literal. A choice between text fields' types stays a
/// text field and is bound. A `select` is one; it takes no `type`.
pub(crate) fn control(
    tag: &str,
    attrs: &[contract_syntax::Attr],
) -> Result<Option<&'static str>, LowerError> {
    if tag != "textarea" {
        if let Some(a) = attrs.iter().find(|a| a.name == "rows") {
            return err("lower-attr-tag", "HTML rows belongs to textarea", a.span);
        }
    }
    if !matches!(tag, "input" | "textarea") {
        if let Some(a) = attrs.iter().find(|a| a.name == "maxlength") {
            return err(
                "lower-attr-tag",
                "HTML maxlength belongs to input or textarea",
                a.span,
            );
        }
    }
    // @ref LLP 1069.011 D1, D2 — a native button, and its style nowhere else.
    if tag == "button" && native_button(attrs)? {
        return Ok(Some("button"));
    }
    if let Some(a) = attrs.iter().find(|a| a.name == "buttonStyle") {
        return err(
            "lower-button-style",
            "`buttonStyle` styles a native button: `button appearance=\"auto\"`",
            a.span,
        );
    }
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
        if let Some(kind) = contract_syntax::unsupported_input_type(&a.value) {
            return err(
                "lower-input-type",
                format!(
                    "`input type=\"{kind}\"` is a non-text control Exact does not support; use `button` for an action"
                ),
                a.span,
            );
        }
        if !matches!(a.value, Expr::Str(..)) && !contract_syntax::text_input_type(&a.value) {
            return err(
                "lower-input-type",
                "`input`'s `type` is a literal (`type=\"text\"`, `\"password\"`, `\"checkbox\"`, …), or a choice between text fields' (`type=shown ? \"text\" : \"password\"`): it picks the kind of node when the view compiles",
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
        "button" => native_tag(),
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
        // A date, time or local date and time: no UA margin in Chrome, and
        // no ARIA role (HTML-AAM maps none).
        "date" | "time" | "datetime-local" => Tag {
            node_type: NodeType::Control,
            fixed_styles: &[],
            fixed_props: &[],
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

/// @ref LLP 1069.011 D1 — whether a `button` is a native one: its effective
/// `appearance` (its class's rows, then its own, over the fixed `none`) is the
/// literal `auto`. A bound one is refused: being native decides the node.
fn native_button(attrs: &[contract_syntax::Attr]) -> Result<bool, LowerError> {
    match attrs.iter().rev().find(|a| a.name == "appearance") {
        None => Ok(false),
        Some(a) => match &a.value {
            Expr::Str(v, _) => Ok(v == "auto"),
            _ => err(
                "lower-button-appearance",
                "a `button`'s `appearance` is a literal: `\"auto\"` makes it the platform's own button, `\"none\"` (the default) the author's box. To switch between them, write `when` with two buttons",
                a.span,
            ),
        },
    }
}

/// The native button lowering's tag (LLP 1069.011 D3): a `Control` of type
/// `button`, a block-level flex item whose box is `border-box` with the
/// platform's chrome inside it (D6).
pub(crate) fn native_tag() -> Tag {
    Tag {
        node_type: NodeType::Control,
        fixed_styles: &[
            (StyleId::Appearance, "auto"),
            (StyleId::Display, "flex"),
            (StyleId::BoxSizing, "border-box"),
        ],
        fixed_props: &[
            (PropId::AccessibilityRole, "button"),
            (PropId::Type, "button"),
        ],
        positional: None,
    }
}

/// What makes a place one a native button may not be (LLP 1069.011.000
/// D9), for the children of the element being lowered; `None` when it adds
/// nothing. `menu_row` is whether that element is a popover's direct child —
/// a menu row, which may be a native button but whose own children are not
/// rows (a host makes a menu item of a popover's direct children only).
pub(crate) fn button_context(
    tag: &str,
    control: Option<&str>,
    menu_row: bool,
) -> Option<&'static str> {
    if tag == "canvas" {
        return Some("a `canvas`");
    }
    if control == Some("button") {
        return Some("a native button");
    }
    // A projection makes one item of a custom button, whatever it holds (a
    // toolbar's, a tab bar's): a native button inside one would be dropped.
    if tag == "button" {
        return Some("a custom `button`");
    }
    if menu_row {
        return Some("a menu row (a native button can be a popover's direct child, not below one)");
    }
    None
}

/// The style rows a native button's box may carry (LLP 1069.011 D6): where it
/// sits and how big it is, whether it shows (`display` is checked to be its
/// own `flex` or `none`), its opacity and transforms, its clip, its accent.
/// Everything else is the platform's to draw or hit-test.
const NATIVE_ROWS: &[StyleId] = &[
    StyleId::Width,
    StyleId::Height,
    StyleId::MinWidth,
    StyleId::MinHeight,
    StyleId::MaxWidth,
    StyleId::MaxHeight,
    StyleId::MarginTop,
    StyleId::MarginRight,
    StyleId::MarginBottom,
    StyleId::MarginLeft,
    StyleId::AlignSelf,
    StyleId::FlexGrow,
    StyleId::FlexShrink,
    StyleId::FlexBasis,
    StyleId::PositionType,
    StyleId::Top,
    StyleId::Right,
    StyleId::Bottom,
    StyleId::Left,
    StyleId::AspectRatio,
    StyleId::Display,
    StyleId::ZIndex,
    StyleId::GridColumn,
    StyleId::GridRow,
    StyleId::Opacity,
    StyleId::Visibility,
    StyleId::Translate,
    StyleId::TranslatePercent,
    StyleId::TranslateZ,
    StyleId::Scale,
    StyleId::Rotate,
    StyleId::RotateAxis,
    StyleId::Transform,
    StyleId::TransformOrigin,
    StyleId::ClipPath,
    StyleId::AccentColor,
    StyleId::Appearance,
    StyleId::Transition,
    StyleId::Animation,
    StyleId::ExitAnimation,
];

/// The motion a native button may carry: its opacity and transforms.
fn native_motion(p: exact_motion::Property) -> bool {
    use exact_motion::Property as P;
    matches!(p, P::Opacity | P::Translate | P::Scale | P::Rotate)
}

/// Whether `attrs` has `name` as the literal `value`.
fn literal(attrs: &[contract_syntax::Attr], name: &str, value: &str) -> bool {
    attrs
        .iter()
        .any(|a| a.name == name && matches!(&a.value, Expr::Str(v, _) if v == value))
}

/// Whether a value can be empty: a blank literal, `none`, or an arm of a
/// choice that is one.
fn may_be_empty(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => s.trim().is_empty(),
        Expr::None(_) => true,
        Expr::Ternary(_, a, b, _) => may_be_empty(a) || may_be_empty(b),
        Expr::Typed(x, _, _) => may_be_empty(x),
        _ => false,
    }
}

/// Whether a value is always empty: every arm blank or `none`.
fn always_empty(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => s.trim().is_empty(),
        Expr::None(_) => true,
        Expr::Ternary(_, a, b, _) => always_empty(a) && always_empty(b),
        Expr::Typed(x, _, _) => always_empty(x),
        _ => false,
    }
}

/// Every literal a value can be: itself, or each arm of a choice.
fn literals(e: &Expr) -> Option<Vec<&str>> {
    match e {
        Expr::Str(s, _) => Some(vec![s.as_str()]),
        Expr::Ternary(_, a, b, _) => {
            let mut out = literals(a)?;
            out.extend(literals(b)?);
            Some(out)
        }
        _ => None,
    }
}

impl Lowerer<'_> {
    /// A native button's attributes, face and place (LLP 1069.011 D2, D5,
    /// D6, D11), after class merging.
    pub(crate) fn check_native_button(
        &self,
        attrs: &[contract_syntax::Attr],
        children: &[contract_syntax::Node],
        span: Span,
    ) -> Result<(), LowerError> {
        let alternative = "write a custom `button` (without `appearance=\"auto\"`) there";
        if let Some(place) = self.button_context {
            return err(
                "lower-button-context",
                format!("a native button cannot be inside {place} in this version: {alternative}"),
                span,
            );
        }
        for a in attrs {
            let name = a.name.as_str();
            let refuse = |id: &'static str, why: String| err(id, why, a.span);
            match name {
                // A menu's invoker is a custom button in this version (LLP 1069.011.000
                // D5); a row that only closes its popover or dialog — a confirmation's
                // action or cancel — is not an invoker.
                "popovertarget" if literal(attrs, "popovertargetaction", "hide") => {}
                "commandfor" if literal(attrs, "command", "close") => {}
                "popovertarget" | "commandfor" | "href" | "action" | "swipeContent"
                | "swipeLeading" | "swipeTrailing" | "swipeIndicator" | "popover" => {
                    return refuse(
                        "lower-button-context",
                        format!("a native button takes no `{name}` in this version: {alternative}"),
                    )
                }
                "type" if !matches!(&a.value, Expr::Str(v, _) if v == "button") => {
                    return refuse(
                        "lower-button-context",
                        "a native button's `type` is `\"button\"`: it is what makes it one".into(),
                    )
                }
                // Its fixed role yields to a tab's or a menu item's (LLP 1069.011.000 D4, D5).
                "role"
                    if !literals(&a.value).is_some_and(|roles| {
                        roles.iter().all(|r| {
                            matches!(
                                *r,
                                "button" | "tab" | "menuitem" | "menuitemcheckbox" | "menuitemradio"
                            )
                        })
                    }) =>
                {
                    return refuse(
                        "lower-button-context",
                        format!("a native button's role is `button`, `tab`, `menuitem`, `menuitemcheckbox` or `menuitemradio`, written as a literal or a choice: {alternative}"),
                    )
                }
                "backgroundMaterial" | "glassGroup" => {
                    return refuse(
                        "lower-button-style-attr",
                        format!("a native button draws its own glass: no `{name}` on it (put a `glassGroup` on its parent)"),
                    )
                }
                "display"
                    if !literals(&a.value)
                        .is_some_and(|ds| ds.iter().all(|d| matches!(*d, "flex" | "none"))) =>
                {
                    return refuse(
                        "lower-button-style-attr",
                        "a native button's `display` is its own `\"flex\"` or `\"none\"` (or a choice between them): the platform lays out its face".into(),
                    )
                }
                "buttonStyle" => match literals(&a.value) {
                    Some(names) => {
                        if let Some(bad) = names
                            .iter()
                            .find(|n| exact_kernel::generated::button_style(n).is_none())
                        {
                            return refuse(
                                "lower-button-style",
                                format!(
                                    "`buttonStyle=\"{bad}\"` is not a button style; styles: {}",
                                    exact_kernel::generated::BUTTON_STYLES.join(", ")
                                ),
                            );
                        }
                    }
                    None => {
                        return refuse(
                            "lower-button-style",
                            "`buttonStyle` is a style's name, or a choice between names, so each can be checked".into(),
                        )
                    }
                },
                _ => {}
            }
            match crate::tags::attr_valued(name, &a.value) {
                Some(crate::tags::AttrTarget::Styles(rows)) => {
                    if rows.iter().any(|r| !NATIVE_ROWS.contains(r)) {
                        return refuse(
                            "lower-button-style-attr",
                            format!("a native button draws its own `{name}`: the platform draws the button; give it a size, a place, `opacity`, a transform or `accent-color`"),
                        );
                    }
                    if rows.contains(&StyleId::Transition) {
                        self.check_native_transition(a)?;
                    }
                    if rows.contains(&StyleId::Animation) || rows.contains(&StyleId::ExitAnimation)
                    {
                        self.check_native_animation(a)?;
                    }
                }
                Some(crate::tags::AttrTarget::Handler(h))
                    if !matches!(h, "press" | "focus" | "blur" | "key" | "hover") =>
                {
                    return refuse(
                        "lower-button-context",
                        format!("a native button takes `press`, `focus`, `blur`, `key` and `hover` handlers, not `{name}`: {alternative}"),
                    );
                }
                _ => {}
            }
        }
        let labelled = attrs
            .iter()
            .any(|a| a.name == "aria-label" && !may_be_empty(&a.value));
        let faces = face_counts(children)?;
        if faces.contains(&(0, 0)) {
            return err(
                "lower-button-content",
                "a native button shows a title (`text`), a symbol (`image \"symbol:…\"`), or both: this one can show neither",
                span,
            );
        }
        if faces.iter().any(|f| f.0 > 1 || f.1 > 1) {
            return err(
                "lower-button-content",
                "a native button shows at most one `text` and one symbol `image` at a time",
                span,
            );
        }
        if !labelled && faces.contains(&(0, 1)) {
            return err(
                "lower-button-content",
                "a native button that shows only a symbol needs an `aria-label` that is never empty",
                span,
            );
        }
        Ok(())
    }

    /// A native button's `transition`: only its opacity and transforms.
    fn check_native_transition(&self, a: &contract_syntax::Attr) -> Result<(), LowerError> {
        let Some(texts) = literals(&a.value) else {
            return err(
                "lower-button-style-attr",
                "a native button's `transition` is a literal, so its properties can be checked",
                a.span,
            );
        };
        for text in texts {
            let Ok(ts) = exact_motion::Transitions::parse(text) else {
                continue; // the row's own check names the error
            };
            for t in &ts.0 {
                let ok = matches!(t.property, exact_motion::TransitionProperty::Property(p) if native_motion(p));
                if !ok {
                    return err(
                        "lower-button-style-attr",
                        "a native button transitions only its `opacity` and transforms (`translate`, `scale`, `rotate`): the platform draws the rest",
                        a.span,
                    );
                }
            }
        }
        Ok(())
    }

    /// A native button's `animation` or `exit-animation`: keyframes, looked
    /// up by name, that touch only its opacity and transforms.
    fn check_native_animation(&self, a: &contract_syntax::Attr) -> Result<(), LowerError> {
        let Some(texts) = literals(&a.value) else {
            return err(
                "lower-button-style-attr",
                "a native button's animation is a literal, so its keyframes can be checked",
                a.span,
            );
        };
        for text in texts {
            let Ok(anims) = exact_motion::Animations::parse(text) else {
                continue;
            };
            for anim in &anims.0 {
                let ok = anim.name == "none"
                    || self
                        .keyframes
                        .get(&anim.name)
                        .is_some_and(|ps| ps.iter().all(|p| native_motion(*p)));
                if !ok {
                    return err(
                        "lower-button-style-attr",
                        format!("a native button animates only its `opacity` and transforms; `{}` animates more (or is not declared here)", anim.name),
                        a.span,
                    );
                }
            }
        }
        Ok(())
    }
}

/// Every face a native button's children can show, as (texts, symbol
/// images), each counted to two; refusing what a face cannot hold
/// (LLP 1069.011 D5): `each`, any other element, attributes but `testId`, a
/// source that is not a literal symbol role.
fn face_counts(nodes: &[contract_syntax::Node]) -> Result<Vec<(u8, u8)>, LowerError> {
    use contract_syntax::Node;
    let mut faces = vec![(0u8, 0u8)];
    let add = |faces: Vec<(u8, u8)>, more: Vec<(u8, u8)>| {
        let mut out: Vec<(u8, u8)> = Vec::new();
        for f in &faces {
            for m in &more {
                let sum = ((f.0 + m.0).min(2), (f.1 + m.1).min(2));
                if !out.contains(&sum) {
                    out.push(sum);
                }
            }
        }
        out
    };
    for node in nodes {
        let here: Vec<(u8, u8)> = match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                ..
            } => {
                if !matches!(tag.as_str(), "text" | "image") {
                    return err(
                        "lower-button-content",
                        format!("a native button shows a `text` and a symbol `image`, not a `{tag}`: the platform draws the button"),
                        *span,
                    );
                }
                if let Some(a) = attrs.iter().find(|a| a.name != "testId") {
                    return err(
                        "lower-button-content",
                        format!("a native button's `{tag}` takes no `{}`: the platform draws its title and image", a.name),
                        a.span,
                    );
                }
                if !children.is_empty() {
                    return err(
                        "lower-button-content",
                        format!("a native button's `{tag}` has no children"),
                        *span,
                    );
                }
                match tag.as_str() {
                    // A blank title shows nothing: it is no title.
                    "text" => {
                        match positional.first() {
                            Some(e) if always_empty(e) => return err(
                                "lower-button-content",
                                "a native button's `text` is its title: this one is always empty",
                                *span,
                            ),
                            Some(e) if may_be_empty(e) => vec![(1, 0), (0, 0)],
                            Some(_) => vec![(1, 0)],
                            None => {
                                return err(
                                    "lower-button-content",
                                    "a native button's `text` is its title: give it one",
                                    *span,
                                )
                            }
                        }
                    }
                    "image" => {
                        // A role from the table or an SF Symbol's name
                        // (LLP 1035.004.000), a choice between them, or a
                        // source computed at runtime that starts `symbol:`.
                        let symbol = |s: &str| {
                            s.strip_prefix("symbol:").is_some_and(|r| {
                                r.strip_prefix("sf/").map_or_else(
                                    || exact_kernel::generated::symbol(r).is_some(),
                                    |name| !name.is_empty(),
                                )
                            })
                        };
                        let ok = match positional.first() {
                            Some(Expr::Template(parts, _)) => matches!(
                                parts.first(),
                                Some(contract_syntax::TemplatePart::Text(t)) if t.starts_with("symbol:")
                            ),
                            Some(e) => {
                                literals(e).is_some_and(|srcs| srcs.iter().all(|s| symbol(s)))
                            }
                            None => false,
                        };
                        if !ok {
                            return err(
                                "lower-button-content",
                                "a native button's image is a symbol, `image \"symbol:…\"` (a role or `sf/` and an SF Symbol's name, a choice between them, or a template that starts `symbol:`)",
                                *span,
                            );
                        }
                        vec![(0, 1)]
                    }
                    _ => unreachable!("checked above"),
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                let mut out = face_counts(then)?;
                for f in face_counts(otherwise)? {
                    if !out.contains(&f) {
                        out.push(f);
                    }
                }
                out
            }
            Node::Match { some, none, .. } => {
                let mut out = face_counts(&some.1)?;
                for f in face_counts(none)? {
                    if !out.contains(&f) {
                        out.push(f);
                    }
                }
                out
            }
            Node::Each { span, .. } => {
                return err(
                    "lower-button-content",
                    "a native button's title and image are fixed in number: no `each` inside it",
                    *span,
                )
            }
            _ => vec![(0, 0)],
        };
        faces = add(faces, here);
    }
    Ok(faces)
}

/// A `button` or `link` with nothing to press: no children, no size and no
/// insets that size it (LLP 1017 P1c), refused before layout could find it.
pub(crate) fn check_zero_size(
    tag: &str,
    attrs: &[contract_syntax::Attr],
    children: &[contract_syntax::Node],
    span: Span,
) -> Result<(), LowerError> {
    const SIZES: &[&str] = &[
        "width",
        "height",
        "flex",
        "padding",
        "padding-top",
        "padding-right",
        "padding-bottom",
        "padding-left",
        "min-width",
        "min-height",
    ];
    // An absolutely positioned box is sized by its insets (CSS 2 §10.3.7,
    // §10.6.4): `inset=0`, or `top` with `bottom` or `left` with `right`
    // (ledger2 Rough 4: a modal's backdrop button was refused).
    let has = |name: &str| attrs.iter().any(|a| a.name == name);
    let positioned = attrs.iter().any(|a| {
        a.name == "position"
            && !matches!(&a.value, contract_syntax::Expr::Str(s, _)
                if matches!(s.as_str(), "static" | "relative" | "sticky"))
    });
    let inset =
        positioned && (has("inset") || has("top") && has("bottom") || has("left") && has("right"));
    if matches!(tag, "button" | "link")
        && children.is_empty()
        && !inset
        && !attrs.iter().any(|a| SIZES.contains(&a.name.as_str()))
    {
        return err(
            "lower-zero-size",
            format!("`{tag}` has no children and no size, so it has zero area and nothing to press: give it children or a size"),
            span,
        );
    }
    Ok(())
}
