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
