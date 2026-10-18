//! A node's `:hover` and `:focus-visible` looks for the Swift presenter
//! (LLP 1121 D8, stage 8 B2): carried in its style dictionary, which the
//! presenter compares, and composes over the base on its own tracking.

use crate::style::{num, style_json_rows};
use exact_kernel::{Env, NodeRef, StyleId, StyleMask};
use std::fmt::Write as _;

/// The pressed paint's rows (LLP 1061 D7), `:active`'s absolute opacity
/// (LLP 1121 D8) among them: a cleared one is no pressed look.
pub(crate) fn press_paint_rows() -> StyleMask {
    use StyleId::{PressBackgroundColor, PressColor, PressOpacity, PressOpacityTo};
    [
        PressOpacity,
        PressOpacityTo,
        PressBackgroundColor,
        PressColor,
    ]
    .into_iter()
    .fold(StyleMask::EMPTY, |m, id| m.union(StyleMask::of(id)))
}

/// `json`, a node's style dictionary, with its looks as `"looks"`, one
/// object keyed by state (`hover`, `focus_visible`), each its rows as the
/// style's own are written (colour pairs, roles) and its opacity, which a
/// style never carries, a number; unchanged for a node with none.
pub(crate) fn with_looks(json: String, node: &NodeRef<'_>, env: &Env) -> String {
    let mut out = String::from("{");
    for (state, look) in node.looks() {
        let (mut rows, _) = style_json_rows(&look.env_resolved(env), Some(env), StyleMask::EMPTY);
        if look.mask.has(StyleId::Opacity) {
            rows.pop();
            let comma = if rows == "{" { "" } else { "," };
            let _ = write!(rows, "{comma}\"opacity\":{}}}", num(look.opacity));
        }
        if rows == "{}" {
            continue;
        }
        let name = match *state {
            exact_kernel::LOOK_HOVER => "hover",
            _ => "focus_visible",
        };
        let comma = if out.len() == 1 { "" } else { "," };
        let _ = write!(out, "{comma}\"{name}\":{rows}");
    }
    if out.len() == 1 {
        return json;
    }
    let head = format!("{{\"looks\":{out}}}");
    match json.as_str() {
        "{}" => head + "}",
        _ => head + "," + &json[1..],
    }
}

#[cfg(test)]
mod tests {
    use crate::style::style_json_for;
    use exact_kernel::Env;
    use exact_kernel::{Kernel, MonospaceMeasurer, NodeType, Op, StyleId, StyleProps, StyleValue};
    use exact_kernel::{LOOK_FOCUS_VISIBLE, LOOK_HOVER};

    /// A node's looks cross in its style as one `looks` object by state,
    /// colours in their pair or role form, opacity a number; a node with
    /// none sends no key.
    #[test]
    fn a_nodes_looks_cross_in_its_style() {
        let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
        let mut focus = StyleProps::default();
        let fill = StyleValue::Text("light-dark(#ffffff, #000000)".into());
        focus.set_dynamic(StyleId::BackgroundColor, &fill).unwrap();
        focus
            .set_dynamic(StyleId::Opacity, &StyleValue::Number(0.5))
            .unwrap();
        let mut hover = StyleProps::default();
        let red = StyleValue::Text("-exact-system-red".into());
        hover.set_dynamic(StyleId::BorderColorTop, &red).unwrap();
        let view = NodeType::View;
        let ops = vec![
            Op::CreateView {
                id: 1,
                node_type: view,
            },
            Op::CreateView {
                id: 2,
                node_type: view,
            },
            Op::SetLooks {
                id: 1,
                looks: vec![(LOOK_HOVER, hover), (LOOK_FOCUS_VISIBLE, focus)],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
        ];
        kernel.apply(0, 1, &ops).unwrap();
        let json = |id: u32| -> serde_json::Value {
            serde_json::from_str(&style_json_for(&kernel.node(id).unwrap(), &Env::default()).0)
                .unwrap()
        };
        let looks = &json(1)["looks"];
        let pair = serde_json::json!([[255, 255, 255, 255], [0, 0, 0, 255]]);
        assert_eq!(looks["focus_visible"]["background_color"], pair, "{looks}");
        assert_eq!(looks["focus_visible"]["opacity"], 0.5);
        assert_eq!(
            looks["hover"]["border_color_top"]["sys"], "systemRedColor",
            "{looks}"
        );
        assert!(json(2).get("looks").is_none());
    }
}
