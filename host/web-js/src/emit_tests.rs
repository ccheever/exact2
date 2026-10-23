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
            .to_mut()
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

/// LLP 1019 D9: `font-variation-settings` is the property, literal or bound.
#[test]
fn font_variation_settings_are_written_literal_and_bound() {
    let plan = contract::compile(
        "component App\n  state bold = false\n  view\n    column\n      text \"a\" font-variation-settings=`'GRAD' 150, 'wdth' 60`\n      text \"b\" font-variation-settings=(bold ? `\"wght\" 800` : \"normal\")\n",
    )
    .unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let all = format!("{}{}", out.css, out.js);
    assert!(
        all.contains(r#"font-variation-settings:"GRAD" 150, "wdth" 60;"#),
        "{all}"
    );
    assert!(out.js.contains("\"font-variation-settings\""), "{}", out.js);
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

/// An author's `user-select` is written in WebKit's prefixed property too,
/// the one Safari selects by, literal and bound; its `auto` and a clear
/// write nothing, so the page's sheet decides, as the native hosts read `auto`.
#[test]
fn user_select_is_written_for_webkit_too() {
    let plan = contract::compile(
        "component App\n  state on = false\n  view\n    column\n      text \"Copy me\" user-select=\"text\"\n      text \"Or me\" user-select=(on ? \"text\" : \"auto\")\n      text \"Not me\" user-select=\"auto\"\n      text \"Nor me\" user-select=\"unset\"\n",
    )
    .unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let all = format!("{}{}", out.js, out.css);
    assert!(
        all.contains("-webkit-user-select:text;user-select:text;"),
        "{all}"
    );
    for unsaid in ["user-select:auto", "user-select:unset"] {
        assert!(!all.contains(unsaid), "{unsaid}: {all}");
    }
    assert_eq!(
        out.js
            .matches("(auto|initial|unset)\\s*$/i.test(v)?null:v")
            .count(),
        2,
        "{}",
        out.js
    );
    for name in ["\"-webkit-user-select\"", "\"user-select\""] {
        assert!(out.js.contains(name), "{name}: {}", out.js);
    }
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
fn a_file_drags_enter_and_leave_bind_through_the_files_piece() {
    let plan = contract::compile(
        "component App\n  state over = false\n  action entered\n    over = true\n  action left\n    over = false\n  view\n    column dragenter=entered dragleave=left height=100\n      text \"a\"\n",
    ).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(
        js.contains("onFileDrag") && js.contains("\"dragenter\"") && js.contains("\"dragleave\""),
        "{js}"
    );
}

#[test]
fn a_button_that_styles_its_own_disabled_state_carries_the_mark() {
    let plan = contract::compile(
        "component App\n  state busy = false\n  action go\n    busy = true\n  view\n    column\n      button press=go disabled=busy background-color=(busy ? \"#eee\" : \"#06f\") testId=\"own\"\n        text \"Save\"\n      button press=go disabled=busy testId=\"plain\"\n        text \"Save\"\n",
    ).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert_eq!(js.matches("disabledstyled").count(), 1, "{js}");
}

/// A platform button's written padding is named on its axis, as its written
/// size is: the neutral look's floor steps aside there (element.rs,
/// neutral.css).
#[test]
fn a_native_button_names_its_written_padding() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      button padding-top=0 padding-bottom=0 testId=\"block\"\n        text \"One\"\n      button padding=8 testId=\"inset\"\n        text \"Two\"\n      button testId=\"unsaid\"\n        text \"Three\"\n",
    )
    .unwrap();
    let sites = crate::emit::Sites::new(&plan).unwrap();
    let parts = crate::style::project(&plan, &sites, &mut Vec::new()).unwrap();
    let said = |name: &str| {
        parts
            .iter()
            .flatten()
            .find(|part| part.props.get("data-testid").is_some_and(|id| id == name))
            .unwrap()
            .props
            .get("data-exact-said")
            .cloned()
    };
    assert_eq!(said("block").as_deref(), Some("padding-block"));
    assert_eq!(
        said("inset").as_deref(),
        Some("padding-inline padding-block")
    );
    assert_eq!(said("unsaid"), None);
}

#[test]
fn a_native_button_marks_bound_size_and_padding() {
    let plan = contract::compile(
        r#"component App
  state n = 0
  view
    button width=(n + 80) height=(n + 24) padding=n
      text "Go"
"#,
    )
    .unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    for name in [
        "width",
        "height",
        "padding-top",
        "padding-right",
        "padding-bottom",
        "padding-left",
    ] {
        assert!(
            js.contains(&format!("\"data-exact-{name}\"")),
            "{name}: {js}"
        );
    }
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

#[test]
fn wire_types_erase_finite_strings_but_surface_defaults_preserve_members() {
    let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
    let words = b.string_enum(&["a|b".into(), "n🥳".into()]);
    let maybe = b.option(words);
    let list = b.list(words);
    let nested = b.record(
        "Nested",
        &[("one", words), ("maybe", maybe), ("many", list)],
    );
    let record = b.record("State", &[("state", words), ("nested", nested)]);
    let plan = b.finish().unwrap();
    assert_eq!(crate::emit::type_code_wire(&plan, record), "{s{s?s[s}}");
    let ordinary = crate::emit::type_code_numeric(&plan, record, false);
    assert!(ordinary.contains("a|b"));
    assert!(ordinary.contains('e'));
    let defaults: serde_json::Value =
        serde_json::from_str(&crate::emit::surface_type_json(&plan, record)).unwrap();
    assert_eq!(defaults["state"], serde_json::json!(["e", ["a|b", "n🥳"]]));
    assert_eq!(defaults["nested"]["one"], defaults["state"]);
}

#[test]
fn virtual_surface_finite_defaults_run_unpublished_omitted_and_nested() {
    let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
    let words = b.string_enum(&["a|b".into(), "n🥳".into()]);
    let maybe = b.option(words);
    let list = b.list(words);
    let nested = b.record(
        "Nested",
        &[("one", words), ("maybe", maybe), ("many", list)],
    );
    let record = b.record("State", &[("state", words), ("nested", nested)]);
    let plan = b.finish().unwrap();
    let descriptor = crate::emit::surface_type_json(&plan, record);
    let script = format!(
        r#"
const src = await Bun.file(process.argv[1]).text();
const data = {{}}, Resources = [], commit = f => {{ f(); return true; }}, R = () => {{}}, Refusal = Error;
const surfaces = new Function('data','Resources','commit','R','Refusal',src.replace(/^import .*;$/gm,'').replace(/^export /gm,'') + ';return surfaces;')(data,Resources,commit,R,Refusal);
surfaces({{hud:['world',{descriptor}]}});
const read = () => data.reserved.exactSurface(null, [], 'hud');
const expected = ['a|b', ['a|b', null, []]];
function equal(a,b) {{ if(JSON.stringify(a)!==JSON.stringify(b)) throw Error(JSON.stringify(a)); }}
equal(read(), expected);
for (const json of ['{{}}','{{"nested":{{}}}}']) {{ exact.surfaceRecord('world',json); equal(read(),expected); }}
exact.surfaceRecord('world','{{"state":"n🥳"}}'); equal(read(),['n🥳',expected[1]]);
exact.surfaceRecord('world',null); equal(read(),expected);
"#
    );
    let out = std::process::Command::new("bun")
        .args(["--eval", &script])
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("facts.js"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn split_metadata_keeps_lists_and_handler_write_sets_distinct() {
    let plan = contract::compile(r#"routes nav
  home "/"
    item "/item/:id"

component App
  state wide = false
  action toggle
    wide = !wide
  action open
    nav = push(nav, path("item", "a"))
  view
    main navigationKey=`${top(nav).id}` navigationBack="back"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0
          list testId="strip" virtualized=true display="flex" height=120 overflow-x="scroll" overflow-y="hidden" estimated-item-width=180
            for n in ["a", "b"] key=n
              text n width=180
          list testId="rows" role=(wide ? "presentation" : "list")
            link href="/item/a" testId="href"
              text "A"
            link href="/item/a" press=toggle testId="local"
              text "Pick"
            button "Open" press=open testId="push"
          button "Toggle" press=toggle testId="toggle"
"#).unwrap();
    let sites = crate::emit::Sites::new(&plan).unwrap();
    let parts = crate::style::project(&plan, &sites, &mut Vec::new()).unwrap();
    let part = |name: &str| {
        parts
            .iter()
            .flatten()
            .find(|part| part.props.get("data-testid").is_some_and(|id| id == name))
            .unwrap()
    };
    assert_eq!(
        part("rows")
            .props
            .get("data-exact-list")
            .map(String::as_str),
        Some("")
    );
    assert!(!part("rows")
        .props
        .contains_key("data-exact-estimated-item-width"));
    assert_eq!(
        part("strip")
            .props
            .get("data-exact-list")
            .map(String::as_str),
        Some("")
    );
    assert_eq!(
        part("strip")
            .props
            .get("data-exact-estimated-item-width")
            .map(String::as_str),
        Some("180")
    );
    for (name, fact) in [
        ("href", None),
        ("local", Some("false")),
        ("push", Some("true")),
    ] {
        assert_eq!(
            part(name)
                .props
                .get("data-exact-router-press")
                .map(String::as_str),
            fact,
            "{name}"
        );
    }
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert!(js.contains("data-exact-estimated-item-width"), "{js}");
    assert!(js.contains("data-exact-router-press"), "{js}");
    assert!(js.contains("data-exact-list"), "{js}");
}

/// LLP 1075.003 §9.14: a tab panel's contents are built when it is first
/// selected (`tp`), a virtualized list's rows too; the dev loop carries a
/// panel component's state by the panel's `id`.
#[test]
fn a_tab_panel_waits_and_its_state_is_carried_by_its_id() {
    let source = r#"routes nav
  tab a "/"
  tab b "/b"

component App
  state items = [1, 2]
  action pick(name: string)
    nav = select(nav, name)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back"
      for t in nav.tabs key=t.name
        if t.name == "a"
          column role="tabpanel" id="panel-a"
            Editor()
            for e in t.stack key=e.id
              column navigationKey=`${e.id}`
                text e.name
        else
          list role="tabpanel" id="panel-b" virtualized=true height=200
            for item in items key=item
              text `${item}`
      row role="tablist"
        button press=pick("a") role="tab" aria-controls="panel-a"
          text "A"
        button press=pick("b") role="tab" aria-controls="panel-b"
          text "B"

component Editor
  state draft = ""
  view
    text draft
"#;
    let plan = contract::compile(source).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    assert_eq!(js.matches("tp(").count(), 2, "{js}");
    let dev = crate::emit::emit(&plan, false, true).unwrap().js;
    assert!(dev.contains(r#"$devSig("draft#1"+"@"+e"#), "{dev}");
}

/// The boot's settlement asks `exactMIDI` and `exactTransport`, so their
/// readers are installed before the first resource; a sequence is armed
/// after mount, as the runner's boot arms it.
#[test]
fn transport_and_midi_readers_precede_the_resources_and_sequences_follow_mount() {
    let plan = contract::compile(
        r#"shape Transport
  state: string
shape MIDI
  state: string
component App
  state on = false
  resource transport = exactTransport() as shape Transport
  resource midi = exactMIDI() as shape MIDI
  sequence groove when on
    play([], bpm=120)
  view
    text `${transport.state} ${midi.state}`
"#,
    )
    .unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    let at = |s: &str| js.find(s).unwrap_or_else(|| panic!("no {s} in {js}"));
    assert!(at("$midi(") < at("const r_0="), "{js}");
    assert!(at("$transport(") < at("const r_0="), "{js}");
    assert!(at("mount(") < at("$sequence("), "{js}");
}

#[test]
fn inline_time_does_not_reserve_a_css_box() {
    for (body, reserve) in [
        ("    time datetime=0 format=\"relative\"\n", true),
        ("    text\n      text \"Updated \"\n      time datetime=0 format=\"relative\"\n      text \" here\"\n", false),
    ] {
        let plan = contract::compile(&format!("component App\n  view\n{body}")).unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        let call = js.split("()=>[").nth(1).expect("live time arguments").split(';').next().unwrap();
        assert!(call.ends_with(&format!("],{reserve})")), "{js}");
        assert!(!js.contains("white-space:nowrap"), "{js}");
    }
}

/// LLP 1121 stage 9 B (D10): a symbol nobody tinted whose `color` its author
/// wrote, on it or on the nearest node that sets one, is marked for
/// symbols.js's sheet to draw it in that colour, a bound source too whatever
/// names its role. Under a bound `color` the mark is symbols.js's to make
/// after each commit: the image says how many bound colours are above it
/// and what decides past them, and each says whether it is written.
#[test]
fn a_symbol_under_a_written_colour_is_marked() {
    let plan = contract::compile(
        r##"component App
  state red = true
  state icon = "symbol:star"
  view
    column
      column color="#ff0000"
        image "symbol:star" testId="literal"
        image (red ? "symbol:star" : "symbol:heart") testId="bound-source"
        image icon testId="named-elsewhere"
        image "assets/mark.png" testId="photo"
        column color=(red ? "#0000ff" : none) testId="maybe"
          image "symbol:star" testId="over-written"
      column color=(red ? "#ff0000" : none) testId="conditional"
        image "symbol:star" testId="bound-colour"
      column
        image "symbol:star" testId="bare"
        image icon testId="bare-bound"
"##,
    )
    .unwrap();
    let sites = crate::emit::Sites::new(&plan).unwrap();
    let parts = crate::style::project(&plan, &sites, &mut Vec::new()).unwrap();
    let props = |name: &str| {
        parts
            .iter()
            .flatten()
            .find(|part| part.props.get("data-testid").is_some_and(|id| id == name))
            .unwrap()
            .props
            .clone()
    };
    for (name, color, ink) in [
        ("literal", true, None),
        ("bound-source", true, None),
        ("named-elsewhere", true, None),
        ("photo", false, None),
        // One bound colour up, and past it a written one.
        ("over-written", false, Some("11")),
        // One bound colour up, and past it nothing written.
        ("bound-colour", false, Some("10")),
        ("bare", false, None),
        ("bare-bound", false, None),
    ] {
        let p = props(name);
        assert_eq!(p.contains_key("data-symbol-color"), color, "{name}: {p:?}");
        assert_eq!(
            p.get("data-symbol-ink").map(String::as_str),
            ink,
            "{name}: {p:?}"
        );
    }
    for name in ["maybe", "conditional"] {
        let p = props(name);
        assert_eq!(
            p.get("data-exact-ink").map(String::as_str),
            Some("1"),
            "{name}: {p:?}"
        );
    }
}
