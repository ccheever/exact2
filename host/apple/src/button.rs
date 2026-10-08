//! Native button faces and authored rows cross the measurement and presentation seams together.
//! @ref LLP 1069.011.001 D1–D15.
use exact_kernel::{ButtonFaceStyle, Env, PressFace};

pub(crate) fn face_json(
    face: Option<&PressFace>,
    rows: Option<&ButtonFaceStyle>,
    style: &str,
    env: &Env,
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
            json.push_str(
                &value.map_or_else(|| "{}".into(), |v| crate::style::style_json(v, env).0),
            );
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
        let json: serde_json::Value = serde_json::from_str(&face_json(
            Some(&face),
            Some(&rows),
            "tinted",
            &kernel.env(),
        ))
        .unwrap();
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
