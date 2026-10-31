//! ARIA states as the DOM's attributes.

use exact_web::Host;

/// `aria-modal` (LLP 1080.003) is the DOM's attribute, beside the dialog role.
#[test]
fn aria_modal_is_the_html_attribute() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      column role=\"dialog\" aria-modal=true testId=\"sheet\"\n        text \"Call\"\n",
    )
    .unwrap();
    let (_host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(batch.contains("\"aria-modal\":\"true\""), "{batch}");
    assert!(batch.contains("\"role\":\"dialog\""), "{batch}");
}

/// Onboarding F22 and spreadsheet F20: a form's states and a menu button's
/// are the DOM's attributes.
#[test]
fn form_states_and_haspopup_are_the_html_attributes() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      input aria-invalid=true aria-required=true aria-describedby=\"err\" testId=\"email\"\n      text \"Wrong\" id=\"err\"\n      button aria-haspopup=\"menu\" testId=\"menu\"\n        text \"File\"\n      button aria-current=\"page\" testId=\"home\"\n        text \"Home\"\n      text \"Name\" role=\"columnheader\" aria-sort=\"ascending\"\n",
    )
    .unwrap();
    let (_host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    for attr in [
        "\"aria-invalid\":\"true\"",
        "\"aria-required\":\"true\"",
        "\"aria-describedby\":\"err\"",
        "\"aria-haspopup\":\"menu\"",
        "\"aria-current\":\"page\"",
        "\"aria-sort\":\"ascending\"",
    ] {
        assert!(batch.contains(attr), "{attr} in {batch}");
    }
}

/// LLP 1116 D8: a drawn range's value is ARIA's own attributes, numbers as
/// HTML writes them.
#[test]
fn range_values_are_the_aria_attributes() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      box role=\"progressbar\" aria-valuenow=3 aria-valuemin=0 aria-valuemax=8 aria-valuetext=\"3 of 8 glasses\" aria-label=\"Water\" testId=\"bar\" width=200 height=8\n",
    )
    .unwrap();
    let (_host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    for attr in [
        "\"role\":\"progressbar\"",
        "\"aria-valuenow\":\"3\"",
        "\"aria-valuemin\":\"0\"",
        "\"aria-valuemax\":\"8\"",
        "\"aria-valuetext\":\"3 of 8 glasses\"",
    ] {
        assert!(batch.contains(attr), "{attr} in {batch}");
    }
}

/// LLP 1115.000 D13.4: an alert has no outside. The page's `<dialog>` is
/// given HTML's own `closedby`, which the browser enforces: for a dialog of
/// the alert's shape `closerequest` (Escape and the browser's Back close it,
/// a click on its backdrop does nothing), where every `alertdialog` was
/// given `any`; `any`, a light-dismiss dialog, where its author writes it or
/// its shape is a chooser's.
#[test]
fn an_alert_shaped_dialog_is_closedby_closerequest_on_the_page() {
    let rows = "        text \"This cannot be undone.\"\n        button press=go commandfor=\"ID\" command=\"close\"\n          text \"Delete\"\n        button commandfor=\"ID\" command=\"close\"\n          text \"Cancel\"\n";
    let dialog = |id: &str, more: &str, rows: &str| {
        format!(
            "      dialog id=\"{id}\" role=\"alertdialog\"{more} testId=\"{id}\"\n{}",
            rows.replace("ID", id)
        )
    };
    let chooser = rows.split_once('\n').unwrap().1;
    let src = format!(
        "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column\n{}{}{}",
        dialog("alert", "", rows),
        dialog("light", " closedby=\"any\"", rows),
        dialog("chooser", "", chooser),
    );
    let plan = contract::compile(&src).unwrap_or_else(|e| panic!("{e}"));
    let (host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    let closedby = |test_id: &str| {
        let k = host.runner().kernel();
        let view = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
        let at = batch
            .find(&format!(
                "{{\"op\":\"create\",\"id\":{view},\"tag\":\"dialog\""
            ))
            .unwrap_or_else(|| panic!("no dialog `{test_id}` in {batch}"));
        let op = &batch[at..];
        let value = &op[op.find("\"closedby\":\"").expect("its closedby") + 12..];
        value[..value.find('"').unwrap()].to_owned()
    };
    assert_eq!(closedby("alert"), "closerequest", "no outside");
    assert_eq!(closedby("light"), "any", "its author's");
    assert_eq!(closedby("chooser"), "any", "a chooser's");
}

/// A heading is HTML's heading element at its level: a text block with
/// `role="heading"`, at its `aria-level` or ARIA's default 2. Past h6 it is
/// a div with its role; an inline run stays a span. An `aria-level` with no
/// `role="heading"` is no heading, on any host (LLP 1115.000 D12.5): it
/// stays a div, where Chrome's accessibility tree ignores the level, as it
/// does beside another role.
#[test]
fn headings_are_heading_elements_and_a_level_alone_is_not() {
    let plan = contract::compile(
        r#"component App
  view
    column
      text "One" role="heading" aria-level=1 testId="one"
      text "Six" role="heading" aria-level=6 testId="six"
      text "Seven" role="heading" aria-level=7 testId="seven"
      text "Two" role="heading" testId="two"
      text "Bare" aria-level=1 testId="bare"
      text "Item" aria-level=2 role="treeitem" testId="item"
      text "Plain" testId="plain"
      text testId="para"
        text "run" aria-level=3 testId="run"
"#,
    )
    .unwrap();
    let (host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = host.runner().kernel();
    for (test_id, tag) in [
        ("one", "h1"),
        ("six", "h6"),
        ("seven", "div"),
        ("two", "h2"),
        ("bare", "div"),
        ("item", "div"),
        ("plain", "div"),
        ("para", "div"),
        ("run", "span"),
    ] {
        let id = kernel
            .node_by_key(kernel.find_by_test_id(test_id)[0])
            .unwrap()
            .id;
        let create = format!("\"op\":\"create\",\"id\":{id},\"tag\":\"{tag}\"");
        assert!(batch.contains(&create), "{test_id} is not a {tag}: {batch}");
    }
    assert!(batch.contains("\"role\":\"treeitem\""), "{batch}");
    // The role is the author's: four headings wrote one, and the host adds
    // none to the level that has none.
    assert_eq!(batch.matches("\"role\":\"heading\"").count(), 4, "{batch}");
}

/// A role an element has natively is not restated (ARIA in HTML): a single
/// `select` is a combobox already, and a restated one makes an accessibility
/// checker demand the `aria-expanded` a native control never carries (LLP
/// 1153 §1, the Pokédex runs' accessibility score). A role an element lacks
/// stays.
#[test]
fn a_native_elements_own_role_is_not_restated() {
    let plan = contract::compile(
        "component App\n  state v = \"a\"\n  action pick(s: string)\n    v = s\n  view\n    column\n      select value=v change=pick testId=\"kind\"\n        option value=\"a\"\n          text \"A\"\n      button press=pick(\"b\") role=\"radio\" aria-checked=true testId=\"b\"\n        text \"B\"\n",
    )
    .unwrap();
    let (_host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!batch.contains("\"role\":\"combobox\""), "{batch}");
    assert!(batch.contains("\"role\":\"radio\""), "{batch}");
}
