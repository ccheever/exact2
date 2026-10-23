//! Dynamic counterparts of the web template's text-style fills.

use crate::style;
use exact_kernel::StyleId;
use exact_plan::{BindingKind, NodesRow, Plan};
use exact_web::host::template::Parts;

type Writes = Vec<(String, String, Option<String>)>;

fn binding(plan: &Plan, row: &NodesRow, id: StyleId) -> Option<bool> {
    row.bindings
        .iter()
        .map(|b| plan.binding(b))
        .find(|b| b.kind == BindingKind::Style && b.id == id as u16)
        .map(|b| style::literal(plan, plan.code(b.expr)).is_none())
}

fn said(name: &str) -> String {
    format!("--exact-said-{name}")
}

pub(super) fn style_map(plan: &Plan, map: &str) -> String {
    let every = crate::modes::every_token();
    let mut styles = crate::measures::size_styles(plan, every);
    for (name, id, prop) in crate::measures::type_properties(plan, every) {
        if id != StyleId::FontSize {
            continue;
        }
        if let Some((_, style)) = styles.iter().find(|(v, _)| *v == format!("var({prop})")) {
            styles.push((name, style.clone()));
        }
    }
    if styles.is_empty() {
        return map.to_string();
    }
    let pairs: Vec<_> = styles
        .iter()
        .map(|(v, s)| {
            format!(
                "{}:{}",
                serde_json::to_string(v).unwrap(),
                serde_json::to_string(s).unwrap()
            )
        })
        .collect();
    format!(
        "(B=>v=>({map})(typeof v===\"string\"&&Object.hasOwn(B,v)?B[v]:v))({{{}}})",
        pairs.join(",")
    )
}

pub(super) fn font_size(
    plan: &Plan,
    row: &NodesRow,
    parts: &Parts,
    share: &mut impl FnMut(String) -> String,
) -> Writes {
    let shorthand = parts.props.contains_key("data-fontshorthand");
    let mut writes = vec![(
        "font-size".into(),
        "px".into(),
        Some(share(style::TEXT_STYLE_MAP.to_string())),
    )];
    for ((id, name), map) in exact_web::css::text_style::ROWS
        .into_iter()
        .zip(&*style::TEXT_STYLE_ROWS)
    {
        let dynamic = binding(plan, row, id);
        if dynamic == Some(false) {
            continue;
        }
        if !shorthand
            && (parts.props.contains_key("data-button-style")
                || id == StyleId::LineHeight && parts.props.contains_key("data-native"))
        {
            continue;
        }
        let map = share(style_map(plan, map));
        let shell_leading = id == StyleId::LineHeight
            && (parts.tag == "select" && !parts.props.get("multiple").is_some_and(|v| v == "true")
                || parts.props.contains_key("data-exact-field"));
        let fill = if shorthand {
            map.clone()
        } else {
            let marker = if shell_leading {
                exact_web::css::text_style::SHELL_LEADING.to_string()
            } else {
                said(name)
            };
            format!("v=>{{const t=({map})(v);return t==null?null:`var({marker},${{t}})`}}")
        };
        let (property, fill) = if dynamic == Some(true) {
            // No named style exposes the shell's own row, if this control has one.
            let absent = if shell_leading {
                format!("var({})", exact_web::css::text_style::CONTROL_LEADING)
            } else {
                // Custom-property `initial` is invalid, so the inherited row's var() inherits.
                "initial".into()
            };
            (
                style::style_fallback(name),
                format!("v=>({fill})(v)??{absent:?}"),
            )
        } else {
            (name.into(), fill)
        };
        writes.push((property, String::new(), Some(fill)));
        if shorthand && dynamic.is_none() {
            writes.push((
                said(name),
                String::new(),
                Some(format!("v=>({map})(v)==null?null:\" \"")),
            ));
        }
    }
    writes
}

pub(super) fn longhand(
    plan: &Plan,
    row: &NodesRow,
    parts: &Parts,
    id: StyleId,
    normal: &str,
    start: Option<&str>,
    writes: &mut Writes,
) {
    let name = style::css_property(id);
    let shorthand = parts.props.contains_key("data-fontshorthand");
    let control_start = parts.props.contains_key("data-button-style")
        || id == StyleId::LineHeight && parts.props.contains_key("data-native");
    let dynamic_size = binding(plan, row, StyleId::FontSize) == Some(true)
        && parts.tag != "img"
        && (shorthand || !control_start);
    if dynamic_size {
        for (property, _, map) in writes
            .iter_mut()
            .filter(|(property, _, _)| *property == name)
        {
            let fallback = format!("var({})", style::style_fallback(property));
            let inner = map.take().unwrap_or_else(|| "v=>v".into());
            *map = Some(format!("v=>({inner})(v)??{fallback:?}"));
        }
    }
    let absent = if dynamic_size && shorthand {
        format!(
            "({})==null?null:\" \"",
            start.expect("a bound shorthand has its size expression")
        )
    } else {
        "null".into()
    };
    // A clear cancels this node's start, while `none` exposes it again.
    writes.push((
        said(&name),
        String::new(),
        Some(format!(
            "v=>v==null?{absent}:/^\\s*(unset|inherit)\\s*$/i.test(v)||!{normal}({name:?},String(v))?\"inherit\":\" \""
        )),
    ));
}
