//! The kernel’s CSS for computed presence declarations.

use exact_kernel::StyleId;
use exact_plan::{Opcode, Plan};
use exact_runner::vm::instructions;

/// Source literals parsed by the kernel for a bound presence declaration.
/// A held value may write no CSS (`none`); an unset value clears its row.
pub struct PresenceValues {
    pub held: std::collections::BTreeMap<String, Option<String>>,
    pub unsets: Vec<String>,
}

fn named_values(
    plan: &Plan,
    code: &[u8],
    row: StyleId,
    parse: impl Fn(&str) -> Option<Option<String>>,
) -> PresenceValues {
    let mut out = PresenceValues {
        held: Default::default(),
        unsets: Vec::new(),
    };
    let strings = instructions(code)
        .filter_map(|i| i.ok().filter(|i| i.op == Opcode::Str))
        .map(|i| plan.str(exact_plan::StrId(i.args[0] as u32)));
    for value in strings {
        if exact_kernel::StyleValue::text_unsets(value, row) {
            out.unsets.push(value.to_string());
        } else if let Some(css) = parse(value) {
            out.held.insert(value.to_string(), css);
        }
    }
    out
}

pub fn layout_transitions(plan: &Plan, code: &[u8]) -> PresenceValues {
    named_values(plan, code, StyleId::LayoutTransition, layout_transition)
}

pub fn exit_animations(plan: &Plan, code: &[u8], press: bool) -> PresenceValues {
    let keyframes = exact_runner::bridge::keyframes(plan);
    named_values(plan, code, StyleId::ExitAnimation, |value| {
        let mut style = exact_kernel::StyleProps::default();
        style
            .set_dynamic(
                StyleId::ExitAnimation,
                &exact_kernel::StyleValue::Text(value.into()),
            )
            .ok()?;
        // Resolve the press rule's scale frames; CSS handles names without a rule.
        let _ = keyframes.resolve(&mut style.rare.exit_animation);
        let animations = &style.rare.exit_animation;
        Some((!animations.0.is_empty()).then(|| exact_web::css::animations_css(animations, press)))
    })
}

/// The kernel's CSS for a bound layout transition, or no declaration.
pub fn layout_transition(value: &str) -> Option<Option<String>> {
    let mut style = exact_kernel::StyleProps::default();
    style
        .set_dynamic(
            StyleId::LayoutTransition,
            &exact_kernel::StyleValue::Text(value.into()),
        )
        .ok()?;
    let (text, _) = exact_web::css::css_text(&style, &[]);
    Some(
        text.split(';')
            .find_map(|d| d.strip_prefix("--exact-layout-transition:"))
            .map(str::to_string),
    )
}

/// A value not named in the binding's code clears the row with a diagnostic.
/// The target carries the kernel's declarations, without another CSS parser.
fn declaration_map(values: &PresenceValues, view: &str, say: &str, name: &str) -> String {
    let held = values
        .held
        .iter()
        .map(|(value, css)| {
            format!(
                "[{}]:{}",
                serde_json::to_string(value).unwrap(),
                serde_json::to_string(css).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let unsets = serde_json::to_string(&values.unsets).unwrap();
    format!("v=>{{if(v==null||/^\\s*(unset)?\\s*$/i.test(v)||{unsets}.includes(v))return null;const T={{{held}}};if(typeof v===\"string\"&&Object.hasOwn(T,v))return T[v];{say}(`view ${{{view}}}: invalid {name} value ${{JSON.stringify(v)}}; unset (the JS target writes only the values the code names)`);return null}}")
}

pub fn layout_transition_map(values: &PresenceValues, view: &str, say: &str) -> String {
    declaration_map(values, view, say, "layout-transition")
}

pub fn exit_animation_map(values: &PresenceValues, view: &str, say: &str) -> String {
    declaration_map(values, view, say, "exit-animation")
}
