//! LLP 1104 r8 D1/D2: fields default native; authored backgrounds, borders
//! and corners devolve them once, while explicit `auto` refuses those rows.

use exact_kernel::{Appearance, Kernel, NodeType, PropId, StyleId};
use exact_plan::{BindingKind, Plan};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(styles: &str, body: &str) -> String {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    format!("{styles}component App\n  state on = true\n  action flip\n    on = not on\n  view\n    column\n{body}")
}

fn compile(styles: &str, body: &str) -> Plan {
    contract::compile(&app(styles, body)).unwrap_or_else(|e| panic!("{body}: {e}"))
}

fn boot(styles: &str, body: &str) -> Runner<NoData> {
    Runner::boot(
        compile(styles, body),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// Rows on the last node, the field in these plan assertions.
fn rows(plan: &Plan, kind: BindingKind) -> Vec<u16> {
    plan.nodes
        .last()
        .unwrap()
        .bindings
        .iter()
        .map(|id| plan.binding(id))
        .filter(|b| b.kind == kind)
        .map(|b| b.id)
        .collect()
}

#[test]
fn default_native_fields_have_no_sheet_or_appearance_row() {
    for tag in [
        "input",
        "input type=\"text\"",
        "input type=\"email\"",
        "input type=\"password\"",
        "input type=\"search\"",
        "input type=\"tel\"",
        "input type=\"url\"",
        "input type=\"number\"",
        "input type=\"EMAIL\"",
        "input type=(on ? \"password\" : \"text\")",
        "textarea",
        "textarea markup=\"none\"",
    ] {
        let plan = compile("", tag);
        let styles = rows(&plan, BindingKind::Style);
        let expected = if tag.starts_with("textarea") {
            vec![StyleId::WhiteSpace as u16, StyleId::OverflowWrap as u16]
        } else {
            vec![]
        };
        assert_eq!(styles, expected, "{tag}: no compiled field sheet");
        assert!(
            !rows(&plan, BindingKind::Prop).contains(&253),
            "no fieldStyle mark"
        );
        let r = boot("", &format!("{tag} testId=\"field\""));
        let field = r.kernel().node(id(&r, "field")).unwrap();
        assert_eq!(field.node_type, NodeType::TextInput);
        assert_eq!(field.style.appearance, Appearance::Auto, "{tag}");
    }
    assert!(
        PropId::from_name("fieldStyle").is_none(),
        "prop 253 is retired"
    );
}

fn disabling() -> Vec<(String, String)> {
    let mut out = vec![
        ("background-color".into(), "\"transparent\"".into()),
        ("background-image".into(), "\"none\"".into()),
    ];
    for side in ["top", "right", "bottom", "left"] {
        for (part, value) in [
            ("width", "0"),
            ("style", "\"none\""),
            ("color", "\"transparent\""),
        ] {
            out.push((format!("border-{side}-{part}"), value.into()));
        }
    }
    for corner in ["top-left", "top-right", "bottom-right", "bottom-left"] {
        out.push((format!("border-{corner}-radius"), "0".into()));
    }
    out
}

#[test]
fn every_disabling_longhand_devolves_the_default_and_is_refused_under_auto() {
    for (name, value) in disabling() {
        for tag in ["input", "textarea"] {
            let body = format!("{tag} {name}={value} testId=\"field\"");
            let r = boot("", &body);
            assert_eq!(
                r.kernel().node(id(&r, "field")).unwrap().style.appearance,
                Appearance::None,
                "{body}"
            );
            let e =
                contract::compile(&app("", &format!("{body} appearance=\"auto\""))).unwrap_err();
            assert_eq!(e.id, "lower-appearance", "{body}: {}", e.message);
            assert!(e.message.contains(&format!("`{name}`")), "{}", e.message);
            assert!(e
                .message
                .contains("a native field draws its own background, border and corners"));
            assert!(e.message.contains("appearance=\"none\""));
            compile("", &format!("{body} appearance=\"none\""));
        }
    }
}

#[test]
fn shorthands_are_checked_as_expanded_longhands() {
    for (name, value, count) in [
        ("border", "\"0\"", 12),
        ("border-top", "\"0\"", 3),
        ("border-right", "\"0\"", 3),
        ("border-bottom", "\"0\"", 3),
        ("border-left", "\"0\"", 3),
        ("border-width", "0", 4),
        ("border-style", "\"none\"", 4),
        ("border-color", "\"transparent\"", 4),
        ("border-radius", "0", 4),
        ("border-width", "\"0px 1px 2px 3px\"", 4),
        ("border-radius", "\"0px 1px 2px 3px\"", 4),
    ] {
        let body = format!("input {name}={value} testId=\"field\"");
        let r = boot("", &body);
        assert_eq!(
            r.kernel().node(id(&r, "field")).unwrap().style.appearance,
            Appearance::None,
            "{body}"
        );
        let source = app("", &format!("{body} appearance=\"auto\""));
        let errors =
            contract::compile_path_source_all(std::path::Path::new("app.contract"), &source, false)
                .err()
                .expect("explicit auto must refuse the shorthand");
        assert_eq!(errors.len(), count, "{body}: {errors:?}");
        assert!(errors.iter().all(|e| e.id == "lower-appearance"));
        for e in errors {
            assert!(
                e.message.contains("background-color") || e.message.contains("border-"),
                "{}",
                e.message
            );
        }
    }
}

#[test]
fn classes_resolve_before_appearance_and_own_attributes_win() {
    let r = boot("style Native\n  appearance=\"auto\"\nstyle Bare\n  appearance=\"none\"\n", "input class=Native testId=\"native\"\ninput class=Native appearance=\"none\" background-color=\"red\" testId=\"bare\"\ninput class=Bare appearance=\"auto\" testId=\"own\"");
    for (name, expected) in [
        ("native", Appearance::Auto),
        ("bare", Appearance::None),
        ("own", Appearance::Auto),
    ] {
        assert_eq!(
            r.kernel().node(id(&r, name)).unwrap().style.appearance,
            expected
        );
    }
    let e = contract::compile(&app(
        "style Fill\n  background-color=\"red\"\n",
        "input class=Fill appearance=\"auto\"",
    ))
    .unwrap_err();
    assert_eq!(e.id, "lower-appearance");
    assert!(e.message.contains("background-color"));
}

#[test]
fn rows_on_any_arm_make_the_field_bare_even_when_cleared() {
    for (styles, row) in [
        (
            "style Fill\n  background-color=\"red\"\nstyle Plain\n  font-weight=600\n",
            "class=(on ? Fill : Plain)",
        ),
        ("", "background-color=(on ? \"red\" : none)"),
        ("", "background-image=(on ? \"none\" : none)"),
        ("", "border-top-width=(on ? 1 : none)"),
        (
            "style Fill\n  border=\"2px solid red\"\nstyle Plain\n  font-weight=600\n",
            "class=(on ? Fill : Plain)",
        ),
        ("", "border=(on ? \"2px solid red\" : none)"),
    ] {
        let mut r = boot(
            styles,
            &format!(
                "input {row} testId=\"field\"\nbutton press=flip testId=\"flip\"\n  text \"Flip\""
            ),
        );
        let field = id(&r, "field");
        assert_eq!(
            r.kernel().node(field).unwrap().style.appearance,
            Appearance::None
        );
        r.dispatch(id(&r, "flip"), Event::Press).unwrap();
        assert_eq!(
            r.kernel().node(field).unwrap().style.appearance,
            Appearance::None
        );
        assert!(r
            .kernel()
            .node(field)
            .unwrap()
            .style
            .background_color
            .is_none_or(|c| c.resolve(false).a() == 0));
        let e = contract::compile(&app(styles, &format!("input {row} appearance=\"auto\"")))
            .unwrap_err();
        assert_eq!(e.id, "lower-appearance");
    }
}

#[test]
fn a_conditional_class_clears_to_the_kernel_without_sheet_fallbacks() {
    let mut r = boot("style Roomy\n  padding=12\n  border-top-width=2\nstyle Plain\n  font-weight=600\n", "input class=(on ? Roomy : Plain) testId=\"field\"\nbutton press=flip testId=\"flip\"\n  text \"Flip\"");
    assert_eq!(
        r.kernel().node(id(&r, "field")).unwrap().style.padding_top,
        exact_kernel::Dimension::Points(12.0)
    );
    r.dispatch(id(&r, "flip"), Event::Press).unwrap();
    let field = r.kernel().node(id(&r, "field")).unwrap();
    assert_eq!(
        field.style.padding_top,
        exact_kernel::Dimension::Points(0.0)
    );
    assert_eq!(field.style.border_widths(), [0.0; 4]);
}

#[test]
fn admitted_non_disabling_rows_keep_the_field_native() {
    for row in [
        "background-clip=\"padding-box\"",
        "background-attachment=\"fixed\"",
        "padding=12",
        "color=\"red\"",
        "opacity=0.8",
        "box-shadow=\"0px 1px 2px black\"",
        "corner-shape=\"bevel\"",
    ] {
        for appearance in ["", " appearance=\"auto\""] {
            let r = boot("", &format!("input {row}{appearance} testId=\"field\""));
            assert_eq!(
                r.kernel().node(id(&r, "field")).unwrap().style.appearance,
                Appearance::Auto,
                "{row}"
            );
        }
    }
}

#[test]
fn excluded_text_inputs_and_markdown_editor_always_get_none() {
    for tag in [
        "input type=\"hidden\"",
        "input type=\"color\"",
        "input type=\"month\"",
        "input type=\"week\"",
        "input type=\"unknown\"",
        "textarea markup=\"markdown\"",
        "textarea markup=(on ? \"markdown\" : \"none\")",
    ] {
        for appearance in ["", " appearance=\"auto\""] {
            let body = format!("{tag}{appearance} testId=\"field\"");
            let plan = compile("", &body);
            assert!(
                rows(&plan, BindingKind::Style).contains(&(StyleId::Appearance as u16)),
                "{tag}"
            );
            let r = boot("", &body);
            let field = r.kernel().node(id(&r, "field")).unwrap();
            assert_eq!(field.node_type, NodeType::TextInput);
            assert_eq!(field.style.appearance, Appearance::None, "{body}");
        }
    }
    let r = boot("", "input type=\"checkbox\" testId=\"control\"");
    let control = r.kernel().node(id(&r, "control")).unwrap();
    assert_eq!(control.node_type, NodeType::Control);
    assert_eq!(control.style.appearance, Appearance::Auto);
}

#[test]
fn fields_and_buttons_share_the_literal_appearance_error() {
    for tag in ["input", "textarea", "button"] {
        for (styles, row) in [
            ("", "appearance=(on ? \"none\" : \"auto\")"),
            (
                "style A\n  appearance=\"auto\"\nstyle B\n  opacity=1\n",
                "class=(on ? A : B)",
            ),
            (
                "style A\n  appearance=\"auto\"\nstyle B\n  appearance=\"none\"\n",
                "class=(on ? A : B)",
            ),
        ] {
            let e = contract::compile(&app(styles, &format!("{tag} {row}"))).unwrap_err();
            assert_eq!(e.id, "lower-appearance", "{}", e.message);
            assert!(e.message.contains("write `if` with two"));
        }
        let body = if tag == "button" {
            format!("{tag} class=(on ? A : B)\n  text \"Go\"")
        } else {
            format!("{tag} class=(on ? A : B)")
        };
        compile(
            "style A\n  appearance=\"auto\"\nstyle B\n  appearance=\"auto\"\n",
            &body,
        );
    }
}

#[test]
fn buttons_default_to_the_platform_control() {
    let r = boot("", "button testId=\"button\"\n  text \"Go\"");
    let button = r.kernel().node(id(&r, "button")).unwrap();
    assert_eq!(button.node_type, NodeType::Control);
    assert_eq!(button.style.appearance, Appearance::Auto);
}

#[test]
fn disabled_fields_have_no_compiled_dimming() {
    let mut r = boot("style Faint\n  opacity=0.8\n", "input disabled=true testId=\"off\"\ntextarea disabled=on testId=\"bound\"\ninput disabled=on opacity=0.7 testId=\"own\"\ninput disabled=on class=Faint testId=\"classed\"\nbutton press=flip testId=\"flip\"\n  text \"Flip\"");
    for _ in 0..2 {
        for (name, expected) in [("off", 1.0), ("bound", 1.0), ("own", 0.7), ("classed", 0.8)] {
            assert_eq!(
                r.kernel().node(id(&r, name)).unwrap().style.opacity,
                expected
            );
        }
        r.dispatch(id(&r, "flip"), Event::Press).unwrap();
    }
}

#[test]
fn background_remains_a_refused_spelling_of_background_color() {
    let e = contract::compile(&app("", "input background=\"transparent\"")).unwrap_err();
    assert_eq!(e.id, "lower-unknown-attr");
    assert!(e.message.contains("background-color"));
}

/// LLP 1104 D6: `outline="none"` (or `outline-style`) takes a text field's
/// ring off. The row is set from the field's own attribute or a class, and
/// an unset arm clears it; a field without it has no row, the platform's
/// ring. Only `none` is admitted, and only on a text field or a textarea.
#[test]
fn outline_none_marks_a_text_field_and_nothing_else() {
    let mut r = boot(
        "style Quiet\n  outline=\"none\"\n",
        "input outline=\"none\" testId=\"input\"\ntextarea outline-style=\"none\" appearance=\"none\" testId=\"textarea\"\ntextarea markup=\"markdown\" outline=\"none\" testId=\"markdown\"\ninput class=Quiet background-color=\"red\" testId=\"class\"\ninput outline=(on ? \"none\" : none) testId=\"arm\"\ninput testId=\"plain\"\nbutton press=flip testId=\"flip\"\n  text \"Flip\"",
    );
    let has = |r: &Runner<NoData>, name: &str| {
        let node = r.kernel().node(id(r, name)).unwrap();
        node.style.mask.has(StyleId::OutlineStyle)
    };
    for name in ["input", "textarea", "markdown", "class", "arm"] {
        assert!(has(&r, name), "{name} takes the ring off");
    }
    assert!(
        !has(&r, "plain"),
        "a field without it keeps the platform's ring"
    );
    r.dispatch(id(&r, "flip"), Event::Press).unwrap();
    assert!(!has(&r, "arm"), "an unset arm gives the ring back");
    for (styles, body, refusal) in [
        (
            "",
            "button press=flip outline=\"none\"\n  text \"Go\"",
            "lower-attr-tag",
        ),
        (
            "",
            "input type=\"checkbox\" outline=\"none\"",
            "lower-attr-tag",
        ),
        ("", "column tabindex=0 outline=\"none\"", "lower-attr-tag"),
        (
            "style Quiet\n  outline=\"none\"\n",
            "column class=Quiet",
            "lower-attr-tag",
        ),
        ("", "input outline=\"solid\"", "lower-attr-value"),
        ("", "textarea outline=\"2px solid red\"", "lower-attr-value"),
    ] {
        let e = contract::compile(&app(styles, body)).unwrap_err();
        assert_eq!(e.id, refusal, "{body}: {}", e.message);
    }
}

/// LLP 1116.007 D4: `appearance="textfield"`, CSS's way to drop a number
/// input's spin buttons, is the platform's field (still native to the
/// kernel, so its chrome and look are the field's) without the stepper; it
/// refuses a drawn background as `auto` does, and is a text field's word
/// only.
#[test]
fn textfield_is_the_platforms_field_without_a_number_fields_stepper() {
    for tag in [
        "input type=\"number\"",
        "input",
        "input type=\"email\"",
        "textarea",
    ] {
        let r = boot(
            "",
            &format!("{tag} appearance=\"textfield\" testId=\"field\""),
        );
        let field = r.kernel().node(id(&r, "field")).unwrap();
        assert_eq!(field.node_type, NodeType::TextInput, "{tag}");
        assert_eq!(field.style.appearance, Appearance::Textfield, "{tag}");
    }
    let e = contract::compile(&app(
        "",
        "input type=\"number\" appearance=\"textfield\" background-color=\"red\"",
    ))
    .unwrap_err();
    assert_eq!(e.id, "lower-appearance", "{}", e.message);
    assert!(e.message.contains("`background-color`"), "{}", e.message);
    let e = contract::compile(&app("", "input appearance=(on ? \"textfield\" : \"auto\")"))
        .unwrap_err();
    assert_eq!(e.id, "lower-appearance", "{}", e.message);
    for body in [
        "input type=\"checkbox\" appearance=\"textfield\"",
        "input type=\"range\" appearance=\"textfield\"",
        "column appearance=\"textfield\"",
        "button press=flip appearance=\"textfield\"\n  text \"Go\"",
    ] {
        let e = contract::compile(&app("", body)).unwrap_err();
        assert_eq!(e.id, "lower-appearance", "{body}: {}", e.message);
    }
}

/// LLP 1104 D3 (amended 2026-10-12): CSS's `::placeholder { color }`, a part
/// block in a `style` or on the field itself, where its value may be
/// computed. The field's own wins over its class's, an unset arm gives the
/// platform's colour back, and a field without it has no row. Only `color`,
/// only on a text field; the names an author might guess point at the block.
#[test]
fn a_placeholder_part_colours_a_text_field_and_nothing_else() {
    use exact_kernel::{Color, ColorValue};
    let mut r = boot(
        "style Themed\n  padding=4\n  ::placeholder\n    color=\"#6b7280\"\n",
        "input placeholder=\"Search\" class=Themed testId=\"class\"\ninput placeholder=\"Name\" class=Themed testId=\"own\"\n  ::placeholder\n    color=\"red\"\ntextarea placeholder=\"Notes\" testId=\"arm\"\n  ::placeholder\n    color=(on ? \"#00800080\" : none)\ninput placeholder=\"Plain\" testId=\"plain\"\nbutton press=flip testId=\"flip\"\n  text \"Flip\"",
    );
    let colour = |r: &Runner<NoData>, name: &str| {
        let node = r.kernel().node(id(r, name)).unwrap();
        assert!(
            !node.style.mask.has(StyleId::TextColor),
            "{name}: the field's own colour is untouched"
        );
        node.style
            .mask
            .has(StyleId::PlaceholderColor)
            .then_some(node.style.placeholder_color)
    };
    let fixed = |r, g, b, a| Some(ColorValue::Fixed(Color::rgba(r, g, b, a)));
    assert_eq!(colour(&r, "class"), fixed(0x6b, 0x72, 0x80, 0xff));
    assert_eq!(
        colour(&r, "own"),
        fixed(0xff, 0, 0, 0xff),
        "the field's own part wins"
    );
    assert_eq!(
        colour(&r, "arm"),
        fixed(0, 0x80, 0, 0x80),
        "its own alpha, kept"
    );
    assert_eq!(colour(&r, "plain"), None, "no part: the platform's colour");
    r.dispatch(id(&r, "flip"), Event::Press).unwrap();
    assert_eq!(
        colour(&r, "arm"),
        None,
        "an unset arm gives the platform's colour back"
    );
    for (styles, body, refusal, says) in [
        (
            "",
            "input placeholder-color=\"red\"",
            "lower-unknown-attr",
            "::placeholder",
        ),
        (
            "",
            "input -exact-placeholder-color=\"red\"",
            "lower-unknown-attr",
            "::placeholder",
        ),
        (
            "",
            "input\n  ::placeholder\n    font-style=\"italic\"",
            "lower-unknown-attr",
            "only `color`",
        ),
        (
            "style S\n  ::placeholder\n    opacity=0.5\n",
            "input class=S",
            "lower-unknown-attr",
            "only `color`",
        ),
        (
            "",
            "text \"Hi\"\n  ::placeholder\n    color=\"red\"",
            "lower-attr-tag",
            "text field",
        ),
        (
            "",
            "input type=\"checkbox\"\n  ::placeholder\n    color=\"red\"",
            "lower-attr-tag",
            "text field",
        ),
        (
            "",
            "input\n  ::before\n    color=\"red\"",
            "syntax-part",
            "no generated box",
        ),
        (
            "",
            "input\n  ::selection\n    color=\"red\"",
            "syntax-part",
            "::placeholder",
        ),
        (
            "style S\n  ::after\n    color=\"red\"\n",
            "input class=S",
            "syntax-part",
            "no generated box",
        ),
        (
            "",
            "input\n  ::placeholder\n    color=\"red\"\n  ::placeholder\n    color=\"blue\"",
            "syntax-duplicate-attr",
            "::placeholder",
        ),
    ] {
        let e = contract::compile(&app(styles, body)).unwrap_err();
        assert_eq!(e.id, refusal, "{body}: {}", e.message);
        assert!(e.message.contains(says), "{body}: {}", e.message);
    }
}
