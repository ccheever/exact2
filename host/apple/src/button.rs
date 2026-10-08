//! Native button faces and authored rows cross the measurement and presentation seams together.
//! @ref LLP 1069.011.001 D1–D15.
use exact_kernel::{ButtonFaceStyle, PressFace};

pub(crate) fn face_json(
    face: Option<&PressFace>,
    rows: Option<&ButtonFaceStyle>,
    style: &str,
) -> String {
    let quote = exact_runner::agent::quote;
    let button = face.is_some();
    let face = face.cloned().unwrap_or_default();
    let row = exact_kernel::generated::button_style(style);
    let drawn = row.or_else(|| exact_kernel::generated::button_style("bordered"));
    let mut json = format!("{{\"button\":{button},\"title\":");
    match &face.title {
        Some(t) => quote(t, &mut json),
        None => json.push_str("null"),
    }
    json.push_str(",\"symbol\":");
    // An SF Symbol's own name, or a role's Apple name (LLP 1035.004.000);
    // a role with none is "", as `symbolName` is: a symbol that draws no
    // image, not no symbol (grok's code review).
    match face.symbol.as_deref().map(|r| {
        r.strip_prefix("sf/")
            .or_else(|| exact_kernel::generated::symbol(r).map(|s| s.0))
            .unwrap_or("")
    }) {
        Some(apple) => quote(apple, &mut json),
        None => json.push_str("null"),
    }
    json.push_str(&format!(
        ",\"raster\":{},\"leading\":{},\"fits\":{},\"label\":",
        face.raster, face.leading, face.fits
    ));
    match &face.label {
        Some(l) => quote(l, &mut json),
        None => json.push_str("null"),
    }
    json.push_str(",\"style\":");
    quote(style, &mut json);
    if let Some(d) = drawn {
        json.push_str(",\"ios\":");
        quote(d.ios, &mut json);
        json.push_str(",\"iosBefore26\":");
        quote(d.ios_before_26, &mut json);
        json.push_str(",\"macos\":");
        quote(d.macos, &mut json);
    }
    json.push_str(&format!(",\"known\":{}", row.is_some()));
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
            json.push_str(&value.map_or_else(|| "{}".into(), resolved_rows));
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

/// The face record already resolved lengths against the live kernel environment.
/// Neither sizing nor drawing may re-resolve them against a second environment.
fn resolved_rows(style: &exact_kernel::StyleProps) -> String {
    let mut json = crate::style::style_json_resolved(style).0;
    // An em size has already incorporated the platform font's Dynamic Type scale.
    if style.mask.has(exact_kernel::StyleId::FontSize)
        && matches!(
            style.relative.get(exact_kernel::StyleId::FontSize),
            Some((exact_kernel::style::relative::Unit::Em, _))
        )
    {
        json.pop();
        json.push_str(",\"font_size_resolved\":1}");
    }
    json
}

#[cfg(test)]
mod tests {
    use super::*;
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
            serde_json::from_str(&face_json(Some(&face), Some(&rows), "tinted")).unwrap();
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
    fn em_fonts_scale_only_platform_bases_and_absolute_bases_scale_once() {
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
        for (name, title_size, scaled) in [
            ("platform", 102., true),
            ("absolute", 13., false),
            ("root", 16., false),
        ] {
            let id = kernel
                .node_by_key(kernel.find_by_test_id(name)[0])
                .unwrap()
                .id;
            let rows = kernel.button_face_style(id).unwrap();
            let json: serde_json::Value = serde_json::from_str(&face_json(
                kernel.press_face(id).as_ref(),
                Some(&rows),
                "bordered",
            ))
            .unwrap();
            assert_eq!(json["rows"]["title"]["font_size"], title_size);
            for part in if name == "root" {
                &["title"][..]
            } else {
                &["title", "subtitle", "symbol"][..]
            } {
                assert_eq!(
                    json["rows"][part].get("font_size_resolved").is_some(),
                    scaled,
                    "{name} {part}"
                );
            }
        }
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
