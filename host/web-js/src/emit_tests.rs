//! What the emitted program says about a plan's resources.
use exact_runner::{DataError, DataSource, Value};

#[test]
fn collection_edges_keep_the_compilers_refusal_for_a_plan_without_virtualization() {
    for event in ["reachstart", "reachend"] {
        let source = format!("component App\n  state disabled = false\n  action more\n    let n = 1\n  view\n    list virtualized=true height=200 {event}=more\n      for x in [1, 2] key=x\n        text `${{x}}`\n");
        let mut plan = contract::compile(&source).unwrap();
        crate::emit::emit(&plan, false, false).unwrap();
        let disabled = plan.slots[0].init;
        let binding = plan
            .bindings
            .iter_mut()
            .find(|b| {
                b.kind == exact_plan::BindingKind::Prop
                    && b.id == exact_kernel::PropId::Virtualized as u16
            })
            .unwrap();
        binding.expr = disabled;
        let error = crate::emit::emit(&plan, false, false)
            .err()
            .expect("non-collection edge");
        assert_eq!(error, contract::collection_edge_refusal(event));
    }
}

#[test]
fn an_app_file_iframe_keeps_its_source_for_the_file_loader() {
    let plan = contract::compile(
        "component App\n  view\n    iframe src=\"app:/data/doc.pdf\" sandbox=\"allow-scripts\"\n",
    )
    .unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(
        js.contains("\"data-app-src\":\"app:/data/doc.pdf\""),
        "{js}"
    );
    assert!(!js.contains("about:blank"), "{js}");
}

struct Answers;
impl DataSource for Answers {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "preview" => Ok(Value::Number(1.0)),
            "full" => Ok(Value::Number(2.0)),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[test]
fn a_state_family_is_applied_as_css_when_it_changes() {
    let plan = contract::compile(
        "component App\n  state family = \"Arial\"\n  action pick(name: string)\n    family = name\n  view\n    column\n      button \"Menlo\" press=pick(\"Menlo\")\n      text \"The quick brown fox\" font-family=family testId=\"sample\"\n",
    )
    .unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(js.contains("fontFamily"), "{js}");
    assert!(js.contains("\"font-family\""), "{js}");
}

#[test]
fn a_constant_template_family_emits_as_a_literal_stack() {
    let plan = contract::compile(
        "component App\n  view\n    text \"x\" font-family=`Arial, ui-monospace`\n",
    )
    .unwrap();
    crate::emit::emit(&plan, false, false).unwrap();
}

#[test]
fn bubbling_focus_uses_the_dom_binder_without_a_wrapper_tab_stop() {
    let plan = contract::compile(
        "component App\n  state target = \"\"\n  action heard(e: FocusEvent)\n    target = e.target\n  view\n    column focusin=heard focusout=heard testId=\"wrapper\"\n      input id=\"child\"\n",
    )
    .unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(js.contains("onFocusWithin"), "{js}");
    assert!(js.contains("\"focusin\""), "{js}");
    assert!(js.contains("\"focusout\""), "{js}");
    assert!(!js.contains("\"tabindex\""), "{js}");
}

/// Review B4: an `else` row's build-time answer is the bake's for every
/// launch (the runner never asks it again), so the JS target receives it
/// settled (`res`'s last argument), while the resource it stands in for
/// keeps its build-time answer as a first frame to ask again at launch.
#[test]
fn an_else_row_is_settled_and_its_owner_is_a_bake() {
    let plan = contract::compile(
        "component App\n  resource full = full() as shape number else preview()\n  view\n    text `${full}`\n",
    )
    .unwrap();
    let plan = contract::bake(plan, Answers).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    let rows: Vec<&str> = js
        .split("const r_")
        .skip(1)
        .map(|s| s.split(';').next().unwrap())
        .collect();
    let owner = rows
        .iter()
        .find(|r| r.contains("\"full\""))
        .expect("full's row");
    let other = rows
        .iter()
        .find(|r| !r.contains("\"full\""))
        .expect("the else row");
    assert!(other.ends_with(",1)"), "the else row is settled: {other}");
    assert!(
        !owner.ends_with(",1)"),
        "the owner is a bake to ask again: {owner}"
    );
}

/// The runner answers its own sources: the program marks their resources, so
/// the overlay never lays a write over one (overlay.js).
#[test]
fn a_runner_owned_resource_is_marked() {
    let plan = contract::compile(
        "shape Time\n  epochAtZero: number\ncomponent App\n  resource time = exactTime() as shape Time\n  resource full = full() as shape number\n  view\n    text `${full}/${time.epochAtZero}`\n",
    )
    .unwrap();
    let plan = contract::bake(plan, Answers).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    let rows: Vec<&str> = js.split("const r_").skip(1).collect();
    let time = rows
        .iter()
        .find(|r| r.contains("\"exactTime\""))
        .expect("time's row");
    let full = rows
        .iter()
        .find(|r| r.contains("\"full\""))
        .expect("full's row");
    assert!(time.contains(".r.owned=true;"), "{time}");
    assert!(!full.contains(".r.owned"), "{full}");
}

/// LLP 1104 D6: `outline="none"` is the field's own `outline-style`, in its
/// class, which outranks the base sheet's `:focus-visible` restore
/// (`#exact-root#exact-root .cN` over `#exact-root input:…`); a bound one is
/// written where it changes, and a field without it declares none.
#[test]
fn outline_none_is_a_fields_own_declaration() {
    let plan = contract::compile(
        "component App\n  state t = \"\"\n  state quiet = true\n  action w(v: string)\n    t = v\n  view\n    column\n      textarea value=t input=w outline=\"none\"\n      input value=t input=w outline=(quiet ? \"none\" : none)\n      input value=t input=w\n",
    )
    .unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    assert!(
        out.css.starts_with("#exact-root#exact-root{"),
        "{}",
        out.css
    );
    assert_eq!(
        out.css.matches("outline-style:none").count(),
        1,
        "{}",
        out.css
    );
    let class = out
        .css
        .split(".c")
        .find(|c| c.contains("outline-style:none"))
        .unwrap();
    let class = class.split('{').next().unwrap();
    assert!(
        out.js.contains(&format!("\"textarea\",{class},")),
        "the textarea's class: {}",
        out.js
    );
    assert_eq!(
        out.js.matches("\"outline-style\"").count(),
        1,
        "the bound one: {}",
        out.js
    );
}

/// LLP 1116.009 D3: a sheet's unsaid background is a row of its class, the page's
/// own role with the table's fallback, so the route it covers is not seen through
/// it; a page is not given one, and a row the author wrote is the only one.
#[test]
fn a_modal_route_is_painted_by_its_class() {
    let app = |page: &str, sheet: &str| {
        let plan = contract::compile(&format!(
            "routes nav\n  home \"/\"\n    add \"/add\"\ncomponent App\n  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\"\n      for e in stack(nav) key=e.id\n        if e.name == \"home\"\n          column navigationKey=`${{e.id}}` {page}\n            text \"Home\"\n        if e.name == \"add\"\n          column navigationKey=`${{e.id}}` navigationPresentation=\"modal\" {sheet}\n            text \"New\"\n"
        ))
        .unwrap();
        crate::emit::emit(&plan, false, false).unwrap().css
    };
    let css = app("", "");
    assert_eq!(
        css.matches("background-color:var(--exact-background,")
            .count(),
        1,
        "{css}"
    );
    let css = app(
        "background-color=\"-exact-secondary-background\"",
        "background-color=\"#336699\"",
    );
    assert_eq!(css.matches("--exact-background,").count(), 0, "{css}");
    assert_eq!(
        css.matches("--exact-secondary-background,").count(),
        1,
        "{css}"
    );
}

/// LLP 1116.009 D4: the shell centres a sheet's title unless the author flows the
/// bar, so a header whose author wrote `justify-content` is named (from the
/// plan's own `written` bit, which the template's kernel carries), and one that
/// did not, or whose row the compiler implied, is not.
#[test]
fn a_header_with_its_own_justify_content_is_named() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      header justify-content=\"space-between\"\n        text \"One\" role=\"heading\" aria-level=2\n      header display=\"flex\" align-items=\"center\"\n        text \"Two\" role=\"heading\" aria-level=2\n",
    )
    .unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let headers: Vec<&str> = out.js.split("\"header\",").skip(1).collect();
    let named = |h: &str| {
        h.split('}')
            .next()
            .unwrap()
            .contains("\"data-exact-said\":\"justify-content\"")
    };
    assert_eq!(headers.len(), 2, "{}", out.js);
    assert_eq!(headers.iter().filter(|h| named(h)).count(), 1, "{}", out.js);
    assert!(named(headers[0]), "the first header wrote it: {}", out.js);
}

/// LLP 1116 D8: a literal `value` on a `progress` is its `<progress>`'s
/// attribute, as a bound one is its property; without it the browser draws
/// an indeterminate bar.
#[test]
fn a_literal_progress_value_is_its_attribute() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      progress value=0.4 max=1 aria-label=\"Done\"\n",
    )
    .unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    let at = js.find("\"progress\"").expect("a <progress>");
    let made = js[at..].split(';').next().unwrap();
    assert!(made.contains("\"value\":\"0.4\""), "{made}");
    assert!(made.contains("\"max\":\"1\""), "{made}");
}

/// LLP 1116.004 D5.2: a radio group of radios is native buttons whose shell
/// look is the segmented control, and its page loads the input piece, which
/// gives it the arrow keys and one Tab stop, even when its radios are bare.
#[test]
fn a_radio_group_is_native_radios_and_loads_the_input_piece() {
    let group = |attrs: &str| {
        let plan = contract::compile(&format!(
            "component App\n  state v = 1\n  action pick(n: number)\n    v = n\n  view\n    row role=\"radiogroup\" aria-label=\"Size\"\n      button role=\"radio\" aria-checked=(v == 1) press=pick(1) {attrs}\n        text \"S\"\n      button role=\"radio\" aria-checked=(v == 2) press=pick(2) {attrs}\n        text \"M\"\n"
        ))
        .unwrap();
        crate::emit::emit(&plan, false, false).unwrap().js
    };
    let js = group("");
    let radio = js
        .split("const e")
        .find(|made| made.contains("\"role\":\"radio\""))
        .expect("a radio");
    assert!(radio.contains("\"button\""), "{radio}");
    assert!(
        radio.contains("\"data-button-style\":\"bordered\""),
        "a native radio: {radio}"
    );
    assert!(js.contains("pressFeedback()"), "{js}");
    let bare = group("appearance=\"none\"");
    assert!(!bare.contains("data-button-style"), "{bare}");
    assert!(
        bare.contains("pressFeedback()"),
        "the group alone loads it: {bare}"
    );
}

/// LLP 1115.000 D4.5: a page with a `tablist` or a `menu` loads the input
/// piece, which gives each its arrow keys, even when nothing in it declares a
/// press look; a page with neither does not.
#[test]
fn a_tablist_and_a_menu_load_the_input_piece() {
    let page = |role: &str| {
        let plan = contract::compile(&format!(
            "component App\n  state v = 1\n  action pick(n: number)\n    v = n\n  view\n    row{role}\n      button appearance=\"none\" press=pick(1)\n        text \"One\"\n      button appearance=\"none\" press=pick(2)\n        text \"Two\"\n"
        ))
        .unwrap();
        crate::emit::emit(&plan, false, false).unwrap().js
    };
    for role in ["tablist", "menu"] {
        let js = page(&format!(" role=\"{role}\""));
        assert!(js.contains("pressFeedback()"), "{role}: {js}");
    }
    let plain = page("");
    assert!(!plain.contains("pressFeedback()"), "{plain}");
}
#[test]
fn aria_disabled_is_an_attribute_for_static_and_bound_values() {
    let plan = contract::compile(
        "component App\n  state unavailable = false\n  view\n    column\n      button \"Update\" aria-disabled=\"true\"\n      button \"Other\" aria-disabled=unavailable\n",
    )
    .unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(js.contains("\"aria-disabled\":\"true\""), "{js}");
    assert!(
        js.matches("\"aria-disabled\"").count() >= 2,
        "the bound value uses the same DOM attribute: {js}"
    );
}

#[test]
fn a_dialog_close_handler_uses_the_dom_event() {
    let plan = contract::compile(
        "component App\n  state count = 0\n  action closed\n    count = count + 1\n  view\n    dialog id=\"form\" close=closed\n      text \"Form\"\n",
    ).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(js.contains("\"close\""), "{js}");
}

#[test]
fn a_scrollend_handler_is_the_dom_event_on_a_scroll() {
    let plan = contract::compile(
        "component App\n  state ends = 0\n  action ended\n    ends = ends + 1\n  view\n    scroll scrollend=ended height=100\n      text \"a\"\n",
    ).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(js.contains("\"scrollend\""), "{js}");
}

#[test]
fn a_button_that_styles_its_own_disabled_state_carries_the_mark() {
    let plan = contract::compile(
        "component App\n  state busy = false\n  action go\n    busy = true\n  view\n    column\n      button press=go disabled=busy background-color=(busy ? \"#eee\" : \"#06f\") testId=\"own\"\n        text \"Save\"\n      button press=go disabled=busy testId=\"plain\"\n        text \"Save\"\n",
    ).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert_eq!(js.matches("disabledstyled").count(), 1, "{js}");
}

#[test]
fn static_authored_clears_keep_inline_css_over_platform_rules() {
    let plan = contract::compile(
        r##"component App
  view
    main navigationKey="/" color="#123456"
      header color="inherit" testId="bar"
        text "Title" role="heading" aria-level=1
        button "Action" color="unset" testId="action"
      input placeholder="Hint" testId="placeholder"
        ::placeholder
          color="unset"
"##,
    )
    .unwrap();
    let sites = crate::emit::Sites::new(&plan).unwrap();
    let parts = crate::style::project(&plan, &sites, &mut Vec::new()).unwrap();
    for name in ["bar", "action"] {
        let part = parts
            .iter()
            .flatten()
            .find(|part| part.props.get("data-testid").is_some_and(|id| id == name))
            .unwrap();
        assert!(part.css.contains("color:unset;"), "{}", part.css);
    }
    let placeholder = parts
        .iter()
        .flatten()
        .find(|part| {
            part.props
                .get("data-testid")
                .is_some_and(|id| id == "placeholder")
        })
        .unwrap();
    assert!(
        placeholder
            .css
            .contains("--exact-placeholder-color:initial;"),
        "{}",
        placeholder.css
    );
}

#[test]
fn finite_strings_keep_member_bytes_when_a_hidden_capture_allows_nonfinite_numbers() {
    let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
    let number = b.primitive(exact_plan::TypeKind::Number);
    let words = b.string_enum(&["pending".into(), "done".into(), "n🥳".into()]);
    let shape = b.record("Capture", &[("n", number), ("state", words)]);
    let plan = b.finish().unwrap();
    let normal = crate::emit::type_code_numeric(&plan, shape, false);
    let hidden = crate::emit::type_code_numeric(&plan, shape, true);
    assert!(normal.starts_with("{n"), "{normal}");
    assert!(hidden.starts_with("{N"), "{hidden}");
    assert_eq!(&normal[2..], &hidden[2..]);
    assert!(hidden.contains("pending"));
    assert!(hidden.contains("n🥳"));
}
