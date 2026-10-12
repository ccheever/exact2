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
        "component App\n  view\n    column\n      input aria-invalid=true aria-required=true aria-describedby=\"err\" testId=\"email\"\n      text \"Wrong\" id=\"err\"\n      button aria-haspopup=\"menu\" testId=\"menu\"\n        text \"File\"\n      button aria-current=\"page\" testId=\"home\"\n        text \"Home\"\n",
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
