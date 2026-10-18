//! A node's `:hover` and `:focus-visible` looks for the Swift presenter
//! (LLP 1121 D8, stage 8 B2): carried in its style dictionary, which the
//! presenter compares, and composes over the base on its own tracking.

use crate::style::{num, style_json_rows};
use exact_kernel::{ColorValue, Env, NodeRef, StyleId, StyleMask};

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

/// Whether the host may paint `property` over `node`'s own: a press's
/// paint, or a look naming it (set or cleared).
pub(crate) fn paints_over(node: &NodeRef<'_>, property: exact_motion::Property) -> bool {
    use exact_motion::Property as P;
    use StyleId::*;
    let (rows, row): (&[StyleId], _) = match property {
        P::Opacity => (&[PressOpacity, PressOpacityTo], Opacity),
        P::BackgroundColor => (&[PressBackgroundColor], BackgroundColor),
        P::Color => (&[PressColor], TextColor),
        _ => return false,
    };
    rows.iter().any(|r| node.style.mask.has(*r))
        || node
            .looks()
            .iter()
            .any(|(_, l)| l.mask.union(l.written).has(row))
}

/// `json`, a node's style dictionary, with its looks as `"looks"`, one
/// object keyed by state (`hover`, `focus_visible`), each its rows as the
/// style's own are written (colour pairs, roles; `currentcolor` as the node's
/// `current` colour), its opacity, which a style never carries, a number,
/// and a row it clears null (an opacity 1), so the base's goes; unchanged
/// for a node with none.
pub(crate) fn with_looks(
    json: String,
    node: &NodeRef<'_>,
    env: &Env,
    current: ColorValue,
) -> String {
    use StyleId::*;
    let mut looks = serde_json::Map::new();
    for (state, look) in node.looks() {
        let mut look = look.env_resolved(env).into_owned();
        // A side it clears is CSS's initial `currentcolor`, the node's colour.
        for id in [
            BorderColorTop,
            BorderColorRight,
            BorderColorBottom,
            BorderColorLeft,
        ] {
            if look.written.has(id) && !look.mask.has(id) {
                look.mask.set(id);
            }
        }
        let mask = look.mask;
        let own = |id, c: Option<ColorValue>| {
            if mask.has(id) {
                Some(c.unwrap_or(current))
            } else {
                c
            }
        };
        look.background_color = own(BackgroundColor, look.background_color);
        look.border_color_top = own(BorderColorTop, look.border_color_top);
        look.border_color_right = own(BorderColorRight, look.border_color_right);
        look.border_color_bottom = own(BorderColorBottom, look.border_color_bottom);
        look.border_color_left = own(BorderColorLeft, look.border_color_left);
        let (rows, _) = style_json_rows(&look, Some(env), StyleMask::EMPTY);
        let Ok(serde_json::Value::Object(mut rows)) = serde_json::from_str(&rows) else {
            continue;
        };
        rows.remove("written");
        for id in look.mask.union(look.written).iter() {
            let name = id.name();
            if id == Opacity {
                let opacity = if look.mask.has(Opacity) {
                    look.opacity
                } else {
                    1.0
                };
                rows.insert(
                    name.into(),
                    serde_json::from_str(&num(opacity)).unwrap_or_default(),
                );
            } else {
                rows.entry(name).or_insert(serde_json::Value::Null);
            }
        }
        if rows.is_empty() {
            continue;
        }
        let name = match *state {
            exact_kernel::LOOK_HOVER => "hover",
            _ => "focus_visible",
        };
        looks.insert(name.into(), rows.into());
    }
    if looks.is_empty() {
        return json;
    }
    let head = format!("{{\"looks\":{}", serde_json::Value::Object(looks));
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

    /// A look's `currentcolor` is the node's colour, and a row it clears
    /// crosses as null (an opacity as 1), so the base's goes.
    #[test]
    fn a_looks_current_colour_and_its_clears_cross() {
        let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
        let mut base = StyleProps::default();
        base.set_dynamic(StyleId::TextColor, &StyleValue::Text("#ff0000".into()))
            .unwrap();
        let mut hover = StyleProps::default();
        let current = StyleValue::Text("currentcolor".into());
        hover
            .set_dynamic(StyleId::BackgroundColor, &current)
            .unwrap();
        hover
            .set_dynamic(StyleId::BoxShadow, &StyleValue::Text("none".into()))
            .unwrap();
        hover.written.set(StyleId::Opacity);
        hover.written.set(StyleId::BorderColorTop);
        let ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(base),
            },
            Op::SetLooks {
                id: 1,
                looks: vec![(LOOK_HOVER, hover)],
            },
        ];
        kernel.apply(0, 1, &ops).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&style_json_for(&kernel.node(1).unwrap(), &Env::default()).0)
                .unwrap();
        let hover = &json["looks"]["hover"];
        assert_eq!(
            hover["background_color"],
            serde_json::json!([255, 0, 0, 255]),
            "{hover}"
        );
        assert!(
            hover.get("box_shadow").is_some_and(|v| v.is_null()),
            "{hover}"
        );
        assert_eq!(hover["opacity"], 1, "{hover}");
        assert_eq!(
            hover["border_color_top"],
            serde_json::json!([255, 0, 0, 255]),
            "a cleared side is currentcolor: {hover}"
        );
    }
}
