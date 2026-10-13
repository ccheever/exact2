use super::*;

const CUSTOM_PROFILE: &str = "list appearance=\"auto\" testId=\"list\" flex=1
  section
    column testId=\"profile\" flex-direction=\"column\" width=\"100%\" padding-top=12 padding-bottom=12
      row testId=\"metric\" width=\"100%\" align-items=\"center\" gap=8 height=21
        image \"icon.svg\" width=19 height=19
        text \"Metric\" width=66 font-size=15
        view testId=\"gauge\" flex=1 min-width=0 height=14
        text \"42\" testId=\"value\" width=18 text-align=\"right\" font-size=15";

#[test]
fn full_width_custom_rows_include_the_system_insets_at_every_list_width() {
    for style in ["inset-grouped", "grouped", "plain"] {
        for width in [320.0, 402.0, 768.0] {
            let body = CUSTOM_PROFILE.replace(
                "appearance=\"auto\"",
                &format!("appearance=\"auto\" listStyle=\"{style}\""),
            );
            let r = boot_width(&body, width);
            let k = r.kernel();
            let profile = k.node(id(&r, "profile")).unwrap();
            let group = k.node(profile.parent.unwrap()).unwrap();
            let value = k.node(id(&r, "value")).unwrap();
            let gauge = k.node(id(&r, "gauge")).unwrap();
            assert_eq!(profile.style.box_sizing, BoxSizing::BorderBox);
            assert_eq!(profile.frame.x, group.frame.x, "{style} at {width}");
            assert_eq!(profile.frame.width, group.frame.width, "{style} at {width}");
            assert!(
                (value.frame.x + value.frame.width
                    - (profile.frame.x + profile.frame.width - 16.0))
                    .abs()
                    < 0.01,
                "trailing value remains inside the cell at {width}: {:?}",
                value.frame
            );
            assert!(gauge.frame.width > 0.0);
            assert!(gauge.frame.x + gauge.frame.width < value.frame.x);
            assert_eq!(group.children(), [profile.id], "no extra cell wrapper");
            assert_eq!(
                profile.children(),
                [id(&r, "metric")],
                "the authored children stay direct"
            );
        }
    }
}

#[test]
fn authored_padding_and_classes_stay_inside_a_full_width_row() {
    let body = CUSTOM_PROFILE.replace("padding-top=12 padding-bottom=12", "padding=24");
    let r = boot(&body);
    let k = r.kernel();
    let profile = k.node(id(&r, "profile")).unwrap();
    let group = k.node(profile.parent.unwrap()).unwrap();
    let value = k.node(id(&r, "value")).unwrap();
    assert_eq!(profile.frame.width, group.frame.width);
    assert_eq!(profile.style.padding_left, Dimension::Points(24.0));
    assert_eq!(profile.style.padding_right, Dimension::Points(24.0));
    assert!(
        (value.frame.x + value.frame.width - (profile.frame.x + profile.frame.width - 24.0)).abs()
            < 0.01
    );

    let src = format!(
        "style Insets\n  padding=28\n{}",
        app(&CUSTOM_PROFILE.replace("padding-top=12 padding-bottom=12", "class=Insets"))
    );
    let plan = contract::bake(contract::compile(&src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = id(&r, "root");
    r.kernel_mut()
        .compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    let profile = r.kernel().node(id(&r, "profile")).unwrap();
    assert_eq!(
        profile.style.padding_left,
        Dimension::Points(28.0),
        "the class replaces the sheet"
    );
    assert_eq!(profile.style.padding_right, Dimension::Points(28.0));
    let value = r.kernel().node(id(&r, "value")).unwrap();
    assert!(
        (value.frame.x + value.frame.width - (profile.frame.x + profile.frame.width - 28.0)).abs()
            < 0.01
    );
}

#[test]
fn ordinary_boxes_and_authored_box_sizing_keep_css_semantics() {
    let r = boot("view testId=\"box\" width=\"100%\" padding=12 height=20");
    let node = r.kernel().node(id(&r, "box")).unwrap();
    assert_eq!(node.style.box_sizing, BoxSizing::ContentBox);
    assert_eq!(
        node.frame.width, 426.0,
        "a plain box still adds its padding"
    );

    let body = CUSTOM_PROFILE.replace(
        "testId=\"profile\"",
        "testId=\"profile\" box-sizing=\"content-box\"",
    );
    let r = boot(&body);
    let k = r.kernel();
    let profile = k.node(id(&r, "profile")).unwrap();
    let group = k.node(profile.parent.unwrap()).unwrap();
    assert_eq!(profile.style.box_sizing, BoxSizing::ContentBox);
    assert_eq!(
        profile.frame.width,
        group.frame.width + 32.0,
        "an explicit authored sizing replaces the sheet"
    );
}

#[test]
fn full_width_native_buttons_keep_their_own_chrome_inside_the_cell() {
    let r = boot(
        "list appearance=\"auto\" flex=1\n  section\n    button appearance=\"auto\" width=\"100%\" press=go testId=\"native\"\n      text \"Native\"",
    );
    let k = r.kernel();
    let button = k.node(id(&r, "native")).unwrap();
    let group = k.node(button.parent.unwrap()).unwrap();
    assert_eq!(button.style.box_sizing, BoxSizing::BorderBox);
    assert_eq!(button.frame.x, group.frame.x);
    assert_eq!(button.frame.width, group.frame.width);
    assert_eq!(button.frame.height, 52.0);
    assert_eq!(button.props.bool(PropId::GroupedRowSeparator), Some(true));

    let src = format!(
        "style Native\n  appearance=\"auto\"\n{}",
        app(
            "list appearance=\"auto\" flex=1\n  section\n    button class=Native width=\"100%\" press=go testId=\"native\"\n      text \"Native\""
        )
    );
    contract::compile(&src).expect("class-made native buttons keep only allowed UA rows");
}

#[test]
fn system_separators_skip_hidden_and_last_rows_without_taking_layout_height() {
    let mut r = boot(
        "list appearance=\"auto\" testId=\"list\" flex=1\n  section\n    button press=go testId=\"a\"\n      text \"First\"\n    row display=\"none\" testId=\"hidden\"\n      text \"Hidden\"\n    when dark\n      row testId=\"b\"\n        text \"Last\"",
    );
    let a = id(&r, "a");
    assert!(
        !r.kernel().grouped_row_separator(a),
        "last visible row has no separator"
    );
    assert!(!r.kernel().grouped_row_separator(id(&r, "hidden")));
    assert_eq!(r.kernel().node(a).unwrap().frame.height, 52.0);
    r.dispatch(a, exact_runner::Event::Press).unwrap();
    let root = id(&r, "root");
    r.kernel_mut()
        .compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    let b = id(&r, "b");
    assert!(r.kernel().grouped_row_separator(a));
    assert!(!r.kernel().grouped_row_separator(b));
    let af = r.kernel().node(a).unwrap().frame;
    let bf = r.kernel().node(b).unwrap().frame;
    assert_eq!(
        bf.y,
        af.y + af.height,
        "paint separators never overlap layout"
    );
    r.dispatch(a, exact_runner::Event::Press).unwrap();
    assert!(
        !r.kernel().grouped_row_separator(a),
        "removing the final row updates the previous one"
    );

    let cardless = boot(
        "list appearance=\"auto\" flex=1\n  section background-color=\"transparent\"\n    row testId=\"a\"\n      text \"First\"\n    row\n      text \"Last\"",
    );
    assert!(!cardless.kernel().grouped_row_separator(id(&cardless, "a")));
    assert_eq!(
        cardless
            .kernel()
            .node(id(&cardless, "a"))
            .unwrap()
            .props
            .bool(PropId::GroupedRowSeparator),
        Some(false)
    );
}

#[test]
fn authored_borders_keep_their_css_geometry_beside_system_separator_paint() {
    let r = boot(
        "list appearance=\"auto\" flex=1\n  section\n    row testId=\"bordered\" width=\"100%\" height=60 border-bottom-width=3 border-bottom-style=\"solid\"\n      text \"Bordered\"\n    row\n      text \"Last\"",
    );
    let k = r.kernel();
    let row = k.node(id(&r, "bordered")).unwrap();
    let group = k.node(row.parent.unwrap()).unwrap();
    assert_eq!(row.frame.width, group.frame.width);
    assert_eq!(
        row.frame.height, 60.0,
        "the border stays inside the authored height"
    );
    assert_eq!(row.style.border_width_bottom, 3.0);
    assert!(k.grouped_row_separator(row.id));
}

#[test]
fn the_system_separator_marker_is_not_an_authored_attribute() {
    let error = contract::compile(&app("view groupedRowSeparator=true")).unwrap_err();
    assert!(error.message.contains("groupedRowSeparator"));
}
