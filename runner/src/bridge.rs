//! Values to kernel rows.
//!
//! @ref LLP 1004 D2 (kernel ordinals get meaning from `exact-kernel`)
//!
//! A binding names a kernel prop or style row by ordinal and yields a
//! [`Value`]. This is the one place a value becomes a kernel write: props
//! through `PropValue` by the prop's declared kind, style rows through the
//! kernel's own generated `StyleProps::set_dynamic`. Nothing is redeclared.

use exact_kernel::{PropId, PropValue, StyleId, StyleProps, StyleValue, StyleValueError};
use exact_plan::Value;
use std::collections::HashMap;

/// Session-scoped CSS family lists, indexed after the plan's compiled stacks.
#[derive(Debug, Default, Clone)]
pub struct RuntimeFonts {
    names: Vec<String>,
    ids: HashMap<String, u16>,
}

impl RuntimeFonts {
    /// Intern a CSS family list, returning its kernel stack id.
    pub fn intern(&mut self, name: &str, compiled: usize) -> Option<u16> {
        if let Some(id) = self.ids.get(name) {
            return Some(*id);
        }
        if runtime_family_members(name).is_empty() {
            return None;
        }
        let id = u16::try_from(compiled.checked_add(self.names.len())?).ok()?;
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        Some(id)
    }

    /// Runtime lists in id order.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// One CSS family name; quoting makes a generic keyword an installed name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFamilyMember {
    /// The family name without CSS quotes.
    pub name: String,
    /// Whether the author quoted this member.
    pub quoted: bool,
}

/// Members of a runtime CSS family list. A malformed list has no members
/// and leaves the binding unset, as an invalid computed CSS value does.
pub fn runtime_family_members(list: &str) -> Vec<RuntimeFamilyMember> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (i, c) in list.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c == ',' {
            out.push(&list[start..i]);
            start = i + 1;
        }
    }
    out.push(&list[start..]);
    if quote.is_some() || escaped || out.len() > 64 {
        return Vec::new();
    }
    out.into_iter()
        .map(|part| {
            let part = part.trim();
            let quoted = part.len() >= 2
                && ((part.starts_with('"') && part.ends_with('"'))
                    || (part.starts_with('\'') && part.ends_with('\'')));
            let name = if quoted {
                &part[1..part.len() - 1]
            } else if part.contains(['"', '\'']) {
                return None;
            } else {
                part
            };
            // Unquoted, a member is CSS identifiers, by the rule a compiled
            // literal meets: a browser refuses the whole declaration otherwise.
            if name.is_empty() || (!quoted && !exact_plan::is_css_family_name(name)) {
                return None;
            }
            Some(RuntimeFamilyMember {
                name: name.to_string(),
                quoted,
            })
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

/// Why a binding's value could not become a kernel row.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    UnknownProp(u16),
    UnknownStyle(u16),
    PropKind { prop: PropId, value: Value },
    Style(StyleValueError),
    StyleKind { style: StyleId, value: Value },
    FontStack { index: u32, stacks: usize },
}

/// A prop value by the prop's declared kind.
pub fn prop_value(id: u16, value: &Value) -> Result<(PropId, PropValue), BridgeError> {
    let prop = PropId::from_wire(id).ok_or(BridgeError::UnknownProp(id))?;
    let mismatch = || BridgeError::PropKind {
        prop,
        value: value.clone(),
    };
    let out = match (prop.kind(), value) {
        (exact_kernel::PropKind::Str, v @ exact_plan::str_value!()) => {
            PropValue::Str(v.text().into())
        }
        (exact_kernel::PropKind::Str, Value::Number(n)) => {
            PropValue::Str(crate::stdlib::format_number(*n).into())
        }
        (exact_kernel::PropKind::Bool, Value::Bool(b)) => PropValue::Bool(*b),
        (exact_kernel::PropKind::Int, Value::Number(n))
            if n.is_finite()
                && n.fract() == 0.0
                && *n >= i64::MIN as f64
                && *n < -(i64::MIN as f64) =>
        {
            PropValue::Int(*n as i64)
        }
        (exact_kernel::PropKind::Float, Value::Number(n)) => PropValue::Float(*n),
        _ => return Err(mismatch()),
    };
    Ok((prop, out))
}

/// Whether `value` is a CSS-wide keyword that leaves row `id` unset
/// ([`StyleValue::unsets`]): the row is cleared, as a class choice's
/// `none` is, and nothing is refused. On `font-family`, `revert` and
/// `revert-layer` are `unset` too, as for an author style the user agent
/// has no rule for; `initial` is the row's initial family ([`set_style`]).
pub fn unsets(id: u16, value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        StyleId::from_bit(id as u32).is_some_and(|style| {
            StyleValue::text_unsets(text, style)
                || (style == StyleId::FontFamily
                    && ["revert", "revert-layer"]
                        .iter()
                        .any(|keyword| text.trim().eq_ignore_ascii_case(keyword)))
        })
    })
}

/// [`set_style`] for a binding of `plan`: a `-exact-platform-color()` only as one
/// of the plan's own string literals (LLP 1095 D3), so data, a template or
/// a concatenation never chooses which platform colour a host looks up. A
/// literal chosen by state (a ternary's arm, a class) is still the plan's.
pub fn set_plan_style(
    patch: &mut StyleProps,
    id: u16,
    value: &Value,
    plan: &exact_plan::Plan,
) -> Result<StyleId, BridgeError> {
    if let v @ exact_plan::str_value!() = value {
        let text = v.text();
        if text.contains("-exact-platform-color(") && !plan.strings.iter().any(|s| s == text) {
            let style = StyleId::from_bit(id as u32).ok_or(BridgeError::UnknownStyle(id))?;
            return Err(BridgeError::StyleKind {
                style,
                value: value.clone(),
            });
        }
    }
    // Profiles are parsed in the owning plan, even before a candidate is accepted.
    // Ordinary styles allocate no declaration table.
    let _profiles = matches!(value, exact_plan::str_value!())
        .then(|| {
            value
                .text()
                .as_bytes()
                .windows(8)
                .any(|w| w.eq_ignore_ascii_case(b"color(--"))
        })
        .unwrap_or(false)
        .then(|| {
            exact_kernel::style::profiled::declarations(
                plan.profiles
                    .iter()
                    .map(|row| (plan.str(row.name), plan.str(row.src), plan.str(row.intent))),
            )
        });
    // @ref LLP 1095 D4 — the plan's theme, for a colour that names an entry.
    let _theme = (!plan.theme.is_empty() && matches!(value, exact_plan::str_value!())).then(|| {
        exact_kernel::style::theme::declarations(plan.theme.iter().flat_map(|row| {
            let value = plan.str(row.value);
            std::iter::once((plan.str(row.name), value))
                .chain((!plan.str(row.role).is_empty()).then(|| (plan.str(row.role), value)))
        }))
    });
    set_style(patch, id, value, plan.stacks.len())
}

/// @ref LLP 1095 D4 — the roles `plan`'s theme replaces, set where the
/// kernel resolves a role: its own defaults (unsaid text is `CanvasText`)
/// and what crosses to a host, as well as a row naming the role. A plan with
/// none clears them.
pub fn set_role_overrides(plan: &exact_plan::Plan) {
    let overrides = plan
        .theme
        .iter()
        .filter_map(|row| {
            let id = exact_kernel::style::roles::role(plan.str(row.role))?;
            let value = plan.str(row.value);
            let colour = exact_kernel::ColorValue::parse_light_dark(value).or_else(|| {
                exact_kernel::Color::parse(value).map(exact_kernel::ColorValue::Fixed)
            })?;
            Some((id, colour))
        })
        .collect();
    exact_kernel::style::roles::set_overrides(overrides);
}

/// [`set_plan_style`] for one of `plan`'s style bindings, with what the
/// binding says of the row: whether its author wrote it on this node
/// (@ref LLP 1115.000 D2). The kernel keeps that beside the row.
pub fn set_binding_style(
    patch: &mut StyleProps,
    binding: &exact_plan::BindingsRow,
    value: &Value,
    plan: &exact_plan::Plan,
) -> Result<StyleId, BridgeError> {
    // @ref LLP 1115.002 D6 — a metric's name is the compiler's, in a sheet
    // it writes: a row its author wrote never reads one, whatever built
    // the value (the compiler refuses the literal).
    if binding.written
        && value
            .as_str()
            .is_some_and(|text| exact_kernel::style::metrics::named(text).is_some())
    {
        let style =
            StyleId::from_bit(binding.id as u32).ok_or(BridgeError::UnknownStyle(binding.id))?;
        return Err(BridgeError::StyleKind {
            style,
            value: value.clone(),
        });
    }
    let style = set_plan_style(patch, binding.id, value, plan)?;
    set_written(patch, binding, style);
    Ok(style)
}

/// A binding whose runtime font stack id has been interned by this session.
pub fn set_runtime_font_style(
    patch: &mut StyleProps,
    binding: &exact_plan::BindingsRow,
    id: u16,
    stacks: usize,
) -> Result<StyleId, BridgeError> {
    let style = set_style(patch, binding.id, &Value::Number(f64::from(id)), stacks)?;
    set_written(patch, binding, style);
    Ok(style)
}

fn set_written(patch: &mut StyleProps, binding: &exact_plan::BindingsRow, style: StyleId) {
    // Both ways: a later binding for the same row is the winner, and an
    // implied winner takes an earlier written bit off (a compiled plan
    // has one binding a row; a built one may not).
    if binding.written {
        patch.written.set(style);
    } else {
        patch.written.clear(style);
    }
}

/// Set style row `id` on `patch` from `value`.
pub fn set_style(
    patch: &mut StyleProps,
    id: u16,
    value: &Value,
    stacks: usize,
) -> Result<StyleId, BridgeError> {
    let style = StyleId::from_bit(id as u32).ok_or(BridgeError::UnknownStyle(id))?;
    if style == StyleId::FontFamily {
        // CSS's `initial` is the row's initial value, not the parent's.
        if value
            .as_str()
            .is_some_and(|text| text.trim().eq_ignore_ascii_case("initial"))
        {
            let initial = StyleProps::default().font_family;
            return set_style(patch, id, &Value::Number(f64::from(initial)), stacks);
        }
        let index = match value {
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => *n as u32,
            _ => {
                return Err(BridgeError::StyleKind {
                    style,
                    value: value.clone(),
                })
            }
        };
        if index as usize >= stacks {
            return Err(BridgeError::FontStack { index, stacks });
        }
    }
    let style_value = match value {
        Value::Number(n) => StyleValue::Number(*n),
        v @ exact_plan::str_value!() => match v.text() {
            "auto" if style.codec() == exact_kernel::StyleCodec::Dimension => StyleValue::Auto,
            // A lone percentage is one; other text ending in `%` is a CSS
            // value (`transform-origin: 0 100%`), as the compiler reads it.
            t => match t
                .strip_suffix('%')
                .and_then(|p| exact_num::parse_f64(p).ok())
            {
                Some(p) => StyleValue::Percent(p),
                _ => StyleValue::Text(t.into()),
            },
        },
        _ => {
            return Err(BridgeError::StyleKind {
                style,
                value: value.clone(),
            })
        }
    };
    patch
        .set_dynamic(style, &style_value)
        .map_err(BridgeError::Style)?;
    Ok(style)
}

/// The plan's `@keyframes` by name, each parsed once per plan (LLP 1055 D5),
/// the first time an `animation` names it: a plan's rules are many and a
/// first frame starts few of them.
#[derive(Debug, Default)]
pub struct KeyframesTable(
    Vec<(
        String,
        String,
        std::cell::OnceCell<Option<exact_motion::Keyframes>>,
    )>,
);

/// How a runner resolves an `animation` list's names against the plan's
/// `@keyframes`, when CSS animations are linked (LLP 1047.001): a plan that
/// animates nothing never names a rule, and links none of the resolution.
pub type KeyframesLink = Option<fn(&KeyframesTable, &mut exact_motion::Animations) -> Vec<String>>;

/// The plan's `keyframes` table. The compiler validated every row; one that
/// no longer parses (a plan from another evaluator) names nothing.
pub fn keyframes(plan: &exact_plan::Plan) -> KeyframesTable {
    KeyframesTable(
        plan.keyframes
            .iter()
            .map(|row| {
                (
                    plan.str(row.name).to_string(),
                    plan.str(row.css).to_string(),
                    std::cell::OnceCell::new(),
                )
            })
            .collect(),
    )
}

impl KeyframesTable {
    /// Give an `animation` row its keyframes; a name no rule has starts no
    /// animation, as in CSS, and is returned as a journal line.
    pub fn resolve(&self, row: &mut exact_motion::Animations) -> Vec<String> {
        row.resolve(|name| {
            self.0
                .iter()
                .filter(|(n, ..)| n == name)
                .find_map(|(_, css, rule)| {
                    rule.get_or_init(|| exact_motion::Keyframes::parse(css).ok())
                        .as_ref()
                })
        })
        .into_iter()
        .map(|name| format!("animation-name `{name}` matches no keyframes: no animation"))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_css_family_members_keep_quoted_commas_and_order() {
        assert_eq!(
            runtime_family_members("Missing, 'A, B', ui-monospace"),
            [
                RuntimeFamilyMember {
                    name: "Missing".into(),
                    quoted: false
                },
                RuntimeFamilyMember {
                    name: "A, B".into(),
                    quoted: true
                },
                RuntimeFamilyMember {
                    name: "ui-monospace".into(),
                    quoted: false
                },
            ]
        );
        assert!(runtime_family_members("'unfinished, serif").is_empty());
        assert!(runtime_family_members("serif,,monospace").is_empty());
    }

    #[test]
    fn invalid_runtime_lists_do_not_intern() {
        let mut fonts = RuntimeFonts::default();
        for value in [
            "inherit",
            "initial",
            "unset",
            "revert",
            "revert-layer",
            "serif,,monospace",
            "'unfinished",
            // Not CSS identifiers: the browser refuses the declaration.
            "Arial;",
            "1Arial",
            "Arial 2",
            "Arial!",
            "Foo/Bar",
            "-1x",
            "Arial, 1x",
            "Arial, initial",
            "default",
            "serif Foo",
            "Monospace Foo",
        ] {
            assert_eq!(fonts.intern(value, 8), None, "{value}");
        }
        assert!(fonts.names().is_empty());
        assert_eq!(fonts.intern("'inherit', Arial", 8), Some(8));
        for value in [
            "'Arial;'",
            "Helvetica Neue",
            "Helvetica\tNeue",
            "-x",
            "--x",
            "_x",
            "a-b_c9",
            "\\31 x",
            "Foo default",
            "inherit Foo",
            "Foo serif",
            "emoji Foo",
            "ui-monospace Foo",
            "Ünï",
        ] {
            assert!(fonts.intern(value, 8).is_some(), "{value}");
        }
    }

    /// CSS-wide keywords: `initial` is the row's initial value, the
    /// kernel's family 0, not the parent's; `revert` on an author style,
    /// with no user-agent rule for the row, is `unset`, as `inherit` is.
    #[test]
    fn initial_is_the_kernel_family_and_revert_inherits() {
        let family = StyleId::FontFamily as u16;
        for keyword in ["initial", "INITIAL", " initial "] {
            let mut style = StyleProps::default();
            set_style(&mut style, family, &Value::Number(4.0), 8).unwrap();
            set_style(&mut style, family, &Value::str(keyword), 8).unwrap();
            assert!(style.mask.has(StyleId::FontFamily), "{keyword}");
            assert_eq!(style.font_family, 0, "{keyword}");
        }
        for keyword in ["inherit", "unset", "revert", "Revert-Layer"] {
            assert!(unsets(family, &Value::str(keyword)), "{keyword}");
        }
        assert!(!unsets(family, &Value::str("initial")));
    }

    #[test]
    fn a_platform_color_is_admitted_only_as_a_plan_literal() {
        let literal = "-exact-platform-color(ios bridgeTestColor, #010203)";
        let mut plan = exact_plan::Plan::default();
        plan.strings.push(literal.into());
        let color = StyleId::TextColor as u16;
        let mut style = StyleProps::default();
        assert!(set_plan_style(&mut style, color, &Value::str(literal), &plan).is_ok());
        for built in [
            "-exact-platform-color(ios bridgeTestOtherColor, #010203)",
            "linear-gradient(-exact-platform-color(ios bridgeTestColor, #010203), #fff)",
        ] {
            assert!(
                matches!(
                    set_plan_style(&mut style, color, &Value::str(built), &plan),
                    Err(BridgeError::StyleKind { .. })
                ),
                "{built}"
            );
        }
        assert!(set_plan_style(&mut style, color, &Value::str("#fff"), &plan).is_ok());
    }

    /// LLP 1115.002 D6: a metric's name is the compiler's. A row its author
    /// wrote never reads one, whatever built the value; a row of a sheet
    /// the compiler wrote holds the metric's value in this process's column
    /// (the web's, until a host says its platform).
    #[test]
    fn a_metric_is_read_only_on_a_row_the_compiler_wrote() {
        let plan = exact_plan::Plan::default();
        let row = |written| exact_plan::BindingsRow {
            kind: exact_plan::BindingKind::Style,
            id: StyleId::MinHeight as u16,
            expr: Default::default(),
            written,
        };
        let name = Value::str("-exact-grouped-row-min-height");
        let mut style = StyleProps::default();
        assert!(matches!(
            set_binding_style(&mut style, &row(true), &name, &plan),
            Err(BridgeError::StyleKind { .. })
        ));
        assert!(!style.mask.has(StyleId::MinHeight));
        set_binding_style(&mut style, &row(false), &name, &plan).unwrap();
        assert_eq!(style.min_height, exact_kernel::Dimension::Points(52.0));
        assert!(!style.written.has(StyleId::MinHeight), "the platform's");
    }

    /// LLP 1115.000 D2: where two bindings set one row the node has the
    /// winner's bit, an implied winner's too. A compiled plan has one
    /// binding a row, but validation admits two.
    #[test]
    fn a_later_binding_for_the_same_row_brings_its_own_written_bit() {
        let plan = exact_plan::Plan::default();
        let row = |written| exact_plan::BindingsRow {
            kind: exact_plan::BindingKind::Style,
            id: StyleId::TextColor as u16,
            expr: Default::default(),
            written,
        };
        let mut style = StyleProps::default();
        set_binding_style(&mut style, &row(true), &Value::str("#ff0000"), &plan).unwrap();
        assert!(style.written.has(StyleId::TextColor));
        set_binding_style(&mut style, &row(false), &Value::str("#0000ff"), &plan).unwrap();
        assert!(style.mask.has(StyleId::TextColor), "the row is still set");
        assert!(
            !style.written.has(StyleId::TextColor),
            "the implied winner takes the written bit off"
        );
        set_binding_style(&mut style, &row(true), &Value::str("#00ff00"), &plan).unwrap();
        assert!(style.written.has(StyleId::TextColor));
    }

    #[test]
    fn css_percentages_accept_browser_exponents() {
        let mut style = StyleProps::default();
        set_style(
            &mut style,
            StyleId::GridTemplateColumns as u16,
            &Value::str("1e2%"),
            0,
        )
        .unwrap();
        assert_eq!(style.rare.grid_template_columns.css(), "100%");
    }
}
