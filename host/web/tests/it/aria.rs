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
