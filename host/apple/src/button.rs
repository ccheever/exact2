//! Native button faces and authored rows cross the measurement and presentation seams together.
//! @ref LLP 1069.011.001 D1–D15.
use exact_kernel::{ButtonFaceStyle, PressFace};

/// `style` is the `buttonStyle` the author wrote. A written name crosses
/// with its row's platform columns (`bordered`'s for a name not in the
/// table). With none written the face says so, `"style":null`, and carries
/// no column: what an unsaid button draws is each platform's own default,
/// which the host holds (`ButtonFace`, LLP 1115.000 D15).
pub(crate) fn face_json(
    face: Option<&PressFace>,
    rows: Option<&ButtonFaceStyle>,
    style: Option<&str>,
) -> String {
    face_json_named(face, rows, style, None)
}

/// `face_json`, and the icon token's name as its author wrote it
/// (`icon.reply`, `icon_path::icon_name`) as `symbolIcon` where the face's
/// symbol is an svg-only icon's path: the agent's name for it (stage 9 A2).
pub(crate) fn face_json_named(
    face: Option<&PressFace>,
    rows: Option<&ButtonFaceStyle>,
    style: Option<&str>,
    icon: Option<&str>,
) -> String {
    let quote = exact_runner::agent::quote;
    let button = face.is_some();
    let face = face.cloned().unwrap_or_default();
    let row = style.and_then(exact_kernel::generated::button_style);
    let drawn =
        style.and_then(|_| row.or_else(|| exact_kernel::generated::button_style("bordered")));
    let mut json = format!("{{\"button\":{button},\"title\":");
    match &face.title {
        Some(t) => quote(t, &mut json),
        None => json.push_str("null"),
    }
    json.push_str(",\"symbol\":");
    // An SF Symbol's own name, or a role's Apple name (LLP 1035.004.000);
    // a role with none is "", as `symbolName` is: a symbol that draws no
    // image, not no symbol (grok's code review).
    match face.symbol.as_deref().map(exact_kernel::apple_symbol_name) {
        Some(apple) => quote(apple, &mut json),
        None => json.push_str("null"),
    }
    // @ref LLP 1121 D10 — an svg-only icon's path beside its "" name (stage
    // 9 A2): the face draws it as a template image its control tints.
    crate::icon_path::push_json(face.symbol.as_deref(), &mut json);
    if let Some(name) = icon.filter(|_| json.contains("\"symbolPath\"")) {
        json.push_str(",\"symbolIcon\":");
        quote(name, &mut json);
    }
    json.push_str(&format!(
        ",\"raster\":{},\"leading\":{},\"fits\":{},\"label\":",
        face.raster, face.leading, face.fits
    ));
    match &face.label {
        Some(l) => quote(l, &mut json),
        None => json.push_str("null"),
    }
    json.push_str(",\"image\":");
    if let Some(image) = &face.image {
        json.push_str(&format!("{{\"view\":{},\"source\":", image.view));
        quote(&image.source, &mut json);
        json.push_str(",\"size\":");
        if let Some((width, height)) = image.size {
            json.push_str(&format!("[{width},{height}]"));
        } else {
            json.push_str("null");
        }
        json.push('}');
    } else {
        json.push_str("null");
    }
    json.push_str(",\"style\":");
    match style {
        Some(style) => quote(style, &mut json),
        None => json.push_str("null"),
    }
    if let Some(d) = drawn {
        json.push_str(",\"ios\":");
        quote(d.ios, &mut json);
        json.push_str(",\"iosBefore26\":");
        quote(d.ios_before_26, &mut json);
        json.push_str(",\"macos\":");
        quote(d.macos, &mut json);
    }
    json.push_str(&format!(",\"known\":{}", style.is_none() || row.is_some()));
    json.push_str(",\"subtitle\":");
    match &face.subtitle {
        Some(t) => quote(t, &mut json),
        None => json.push_str("null"),
    }
    let placement = match face.placement {
        exact_kernel::ButtonImagePlacement::Leading => "leading",
        exact_kernel::ButtonImagePlacement::Trailing => "trailing",
        exact_kernel::ButtonImagePlacement::Top => "top",
        exact_kernel::ButtonImagePlacement::Bottom => "bottom",
    };
    json.push_str(",\"placement\":");
    quote(placement, &mut json);
    if let Some(rows) = rows {
        json.push_str(",\"rows\":{");
        for (index, (name, value)) in [
            ("title", Some(&rows.title)),
            ("subtitle", rows.subtitle.as_ref()),
            ("symbol", Some(&rows.symbol)),
            ("button", Some(&rows.button)),
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 {
                json.push(',');
            }
            quote(name, &mut json);
            json.push(':');
            json.push_str(&value.map_or_else(
                || "{}".into(),
                |part| {
                    let mut part = part.clone();
                    // A clear declaration has no value mask. Text still resolves
                    // through the button, but genuine absence keeps platform ink.
                    if matches!(name, "title" | "subtitle")
                        && (part.written.has(exact_kernel::StyleId::TextColor)
                            || rows.button.written.has(exact_kernel::StyleId::TextColor))
                    {
                        part.mask.set(exact_kernel::StyleId::TextColor);
                    }
                    crate::style::style_json_resolved(&part).0
                },
            ));
        }
        json.push_str(",\"imageGap\":");
        json.push_str(
            &rows
                .image_gap
                .map_or_else(|| "null".into(), |v| v.to_string()),
        );
        json.push('}');
    }
    json.push('}');

    json
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_raster_size_report_refreshes_the_native_button_face() {
        let mut plan = contract::compile("component App\n  view\n    button appearance=\"auto\"\n      image \"symbol:sf/photo\"\n      text \"Photo\"\n").unwrap();
        // The host protocol can represent a raster while portable authoring
        // admission waits for every graphical host to paint one.
        let source = plan
            .strings
            .iter()
            .position(|s| s.starts_with("symbol:"))
            .unwrap();
        plan.strings.set(source, "photo.png");
        let (mut host, _) = crate::Host::boot(
            &plan.encode(),
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.0,
            800.0,
        )
        .unwrap();
        let button = host.runner().kernel().roots()[0];
        let image = host
            .runner()
            .kernel()
            .press_face(button)
            .unwrap()
            .image
            .unwrap();
        let batch: serde_json::Value =
            serde_json::from_str(&host.set_intrinsic(image.view, Some((32.0, 20.0)))).unwrap();
        assert_eq!(batch["controls"], true);
        let face = host.runner().kernel().press_face(button).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&face_json(Some(&face), None, None)).unwrap();
        assert_eq!(json["image"]["view"], image.view);
        assert_eq!(json["image"]["source"], "photo.png");
        assert_eq!(json["image"]["size"], serde_json::json!([32, 20]));
    }

    #[test]
    fn bridge_rows_keep_authorship_and_child_overrides_and_semantic_fields() {
        let plan = contract::compile(r#"component Buttons
  view
    column color="red" font-size=44
      button appearance="auto" buttonStyle="tinted" flex-direction="column" gap=11 padding=9 border-radius=18 -exact-control-size="large"
        image "symbol:sf/lock.fill" font-size=28 -exact-tint-color="green"
        text "Lock" font-weight=600
        text "Your vehicle"
"#).unwrap();
        let (host, _) = crate::Host::boot(
            &plan.encode(),
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.0,
            800.0,
        )
        .unwrap();
        let kernel = host.runner().kernel();
        let id = kernel.node(kernel.roots()[0]).unwrap().children()[0];
        let face = kernel.press_face(id).unwrap();
        let rows = kernel.button_face_style(id).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&face_json(Some(&face), Some(&rows), Some("tinted"))).unwrap();
        assert_eq!(json["subtitle"], "Your vehicle");
        assert_eq!(json["symbol"], "lock.fill");
        assert_eq!(json["placement"], "top");
        assert_eq!(json["rows"]["imageGap"], 11);
        assert_eq!(json["rows"]["title"]["font_weight"], 600);
        assert!(json["rows"]["title"].get("font_size").is_none());
        assert!(json["rows"]["title"].get("text_color").is_none());
        assert_eq!(json["rows"]["symbol"]["font_size"], 28);
        assert_eq!(json["rows"]["button"]["control_size"], "large");
        assert_eq!(json["rows"]["button"]["padding_left"], 9);
        assert_eq!(json["rows"]["button"]["border_radius_top_left"], 18);
    }
    #[test]
    fn clear_colour_intent_reaches_native_faces_without_colouring_unsaid_faces() {
        let plan = contract::compile(
            r#"component Buttons
  view
    column color="red"
      button testId="unsaid" "Unsaid"
      button testId="clear-button" color="inherit" "Clear"
      button testId="clear-title"
        text "Clear" color="inherit"
      button testId="transparent" color="transparent" "Transparent"
"#,
        )
        .unwrap();
        let (host, _) = crate::Host::boot(
            &plan.encode(),
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.,
            800.,
        )
        .unwrap();
        let kernel = host.runner().kernel();
        for name in ["unsaid", "clear-button", "clear-title", "transparent"] {
            let id = kernel
                .node_by_key(kernel.find_by_test_id(name)[0])
                .unwrap()
                .id;
            let rows = kernel.button_face_style(id).unwrap();
            let json: serde_json::Value = serde_json::from_str(&face_json(
                kernel.press_face(id).as_ref(),
                Some(&rows),
                None,
            ))
            .unwrap();
            if name == "unsaid" {
                assert!(json["rows"]["title"].get("text_color").is_none(), "{json}");
            } else {
                let expected = if name == "transparent" {
                    [0, 0, 0, 0]
                } else {
                    [255, 0, 0, 255]
                };
                assert_eq!(
                    json["rows"]["title"]["text_color"],
                    serde_json::json!(expected),
                    "{name}: {json}"
                );
            }
        }
    }
    /// LLP 1115.000 D12.1: a face row's size is the kernel's number, which
    /// the host draws as it is. An `em` under the platform's control font
    /// carries that font's Dynamic Type scale; an absolute size, and an `em`
    /// of one, is the author's; `rem` follows the root. Nothing asks the host
    /// to scale any of them again.
    #[test]
    fn a_face_rows_size_is_the_kernels_number_and_is_scaled_nowhere_else() {
        let plan = contract::compile(
            r#"component Buttons
  view
    column font-size=99
      button appearance="auto" testId="platform" font-size="1.5em"
        image "symbol:sf/lock" font-size="2em"
        text "Platform" font-size="2em"
        text "Subtitle" font-size="1em"
      button appearance="auto" testId="absolute" font-size=13
        image "symbol:sf/lock" font-size="2em"
        text "Absolute" font-size="1em"
        text "Subtitle" font-size="1em"
      button appearance="auto" testId="root" font-size="1rem"
        text "Root" font-size="1em"
"#,
        )
        .unwrap();
        let (mut host, _) = crate::Host::boot(
            &plan.encode(),
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.,
            800.,
        )
        .unwrap();
        let mut env = host.runner().kernel().env();
        extern "C" fn font(_: *mut std::ffi::c_void, _: u8) -> crate::control_text::CControlFont {
            crate::control_text::CControlFont {
                family: std::ptr::null(),
                family_len: 0,
                family_id: 42,
                size: 34.,
                weight: 400,
                italic: 0,
            }
        }
        // A platform font already scaled for accessibility.
        let platform = crate::control_text::text_styles(font, std::ptr::null_mut());
        env.control_text_styles = Some(platform);
        host.runner_mut().kernel_mut().set_env(env).unwrap();
        host.resize(400., 800.);
        let kernel = host.runner().kernel();
        for (name, title_size) in [("platform", 102.), ("absolute", 13.), ("root", 16.)] {
            let id = kernel
                .node_by_key(kernel.find_by_test_id(name)[0])
                .unwrap()
                .id;
            let rows = kernel.button_face_style(id).unwrap();
            let json: serde_json::Value = serde_json::from_str(&face_json(
                kernel.press_face(id).as_ref(),
                Some(&rows),
                None,
            ))
            .unwrap();
            assert_eq!(json["rows"]["title"]["font_size"], title_size);
            for part in if name == "root" {
                &["title"][..]
            } else {
                &["title", "subtitle", "symbol"][..]
            } {
                assert!(
                    json["rows"][part].get("font_size_resolved").is_none(),
                    "{name} {part}"
                );
            }
        }
    }
    /// LLP 1121 D10, stage 9 A2: an svg-only icon's face carries its path
    /// beside its "" name; an `sf` icon's and a role's carry none.
    #[test]
    fn an_svg_only_icons_face_carries_its_path() {
        let json = |symbol: &str| -> serde_json::Value {
            let face = PressFace {
                symbol: Some(symbol.into()),
                fits: true,
                ..Default::default()
            };
            serde_json::from_str(&face_json(Some(&face), None, None)).unwrap()
        };
        let svg = json("icon||0 0 24 24|M2 2 L22 2 L12 20 Z|icons/tri.svg");
        assert_eq!(svg["symbol"], "");
        assert_eq!(svg["symbolPath"], "M 2 2 L 22 2 L 12 20 Z");
        assert_eq!(svg["symbolViewBox"], "0 0 24 24");
        assert!(svg.get("symbolFillRule").is_none(), "nonzero: {svg}");
        // Stage 14 C2: an even-odd icon's face says its rule.
        let even = json("icon||0 0 24 24|evenodd:M2 2 L22 2 L12 20 Z|icons/tri.svg");
        assert_eq!(even["symbolPath"], "M 2 2 L 22 2 L 12 20 Z");
        assert_eq!(even["symbolFillRule"], "evenodd");
        let sf = json("icon|heart|0 0 24 24|M2 2 L22 2 L12 20 Z|icons/tri.svg");
        assert_eq!(sf["symbol"], "heart");
        assert!(sf.get("symbolPath").is_none(), "{sf}");
        assert!(json("sf/lock").get("symbolPath").is_none());
        // The agent's name for it, where the plan gives one; none beside an `sf`.
        let named = |symbol: &str| -> serde_json::Value {
            let face = PressFace {
                symbol: Some(symbol.into()),
                fits: true,
                ..Default::default()
            };
            serde_json::from_str(&face_json_named(Some(&face), None, None, Some("icon.tri")))
                .unwrap()
        };
        assert_eq!(
            named("icon||0 0 24 24|M2 2 L22 2 L12 20 Z|icons/tri.svg")["symbolIcon"],
            "icon.tri"
        );
        assert!(
            named("icon|heart|0 0 24 24|M2 2 L22 2 L12 20 Z|icons/tri.svg")
                .get("symbolIcon")
                .is_none()
        );
    }
    /// LLP 1115.000 D15: the face says whether its author wrote a
    /// `buttonStyle`. A written name crosses with its row's columns; with
    /// none written the style is null and no column crosses, since what an
    /// unsaid button draws is the platform's default, which the host holds.
    /// A name outside the table still draws `bordered` and says it is unknown.
    #[test]
    fn the_face_says_whether_its_author_wrote_a_style() {
        let face = PressFace {
            title: Some("Add".into()),
            fits: true,
            ..Default::default()
        };
        let json = |style| -> serde_json::Value {
            serde_json::from_str(&face_json(Some(&face), None, style)).unwrap()
        };
        let said = json(Some("bordered"));
        assert_eq!(said["style"], "bordered");
        assert_eq!(said["ios"], "bordered");
        assert_eq!(said["macos"], "push");
        assert_eq!(said["known"], true);
        let unsaid = json(None);
        assert!(unsaid["style"].is_null(), "{unsaid}");
        for column in ["ios", "iosBefore26", "macos"] {
            assert!(unsaid.get(column).is_none(), "{column}: {unsaid}");
        }
        assert_eq!(unsaid["known"], true, "nothing written is not a wrong name");
        let wrong = json(Some("chunky"));
        assert_eq!(wrong["style"], "chunky");
        assert_eq!(wrong["ios"], "bordered");
        assert_eq!(wrong["known"], false);
    }
    struct NoData;
    impl exact_runner::DataSource for NoData {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(name.into()))
        }
    }
}
