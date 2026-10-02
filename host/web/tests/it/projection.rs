//! The web host's element projection (LLP 1007.001): a lone plain text is
//! its box's text content.

use exact_runner::Event;
use exact_web::Host;

fn view_with_test_id(host: &Host<caltrain_data::Caltrain>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

/// A text that is its box's only child, with nothing of its own, is the box's
/// text content (LLP 1007.001): its element makes no box. One with a test id,
/// a row of its own, a sibling or a handler keeps its box, and a sibling that
/// arrives later takes the fold back. A button folds only where its height
/// is its text's.
#[test]
fn a_lone_plain_text_is_its_boxs_text_content() {
    let plan = contract::compile(
        r##"component App
  state more = false
  action grow
    more = not more
  view
    column
      button press=grow testId="grow"
        text "Grow"
      button testId="named"
        text "x" testId="label"
      button testId="styled"
        text "x" color="#ff0000"
      column testId="pair"
        text "a"
        when more
          text "b"
      button testId="tall" height=48
        text "t"
      row
        button testId="stretched"
          text "s"
      row align-items=(more ? "stretch" : "center")
        button testId="flips"
          text "f"
"##,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    let k = host.runner().kernel();
    let child = |test_id: &str| {
        let key = k.find_by_test_id(test_id)[0];
        k.node_by_key(key).unwrap().children()[0]
    };
    let css = |batch: &str, id: u32| {
        let at = batch.find(&format!("\"id\":{id},"))?;
        let op = &batch[at..];
        let op = &op[..op.find("{\"op\"").unwrap_or(op.len())];
        op.split("\"css\":\"")
            .nth(1)
            .map(|c| c[..c.find('"').unwrap()].to_owned())
    };
    let (grow, named, styled, pair) = (
        child("grow"),
        child("named"),
        child("styled"),
        child("pair"),
    );
    assert_eq!(
        css(&first, grow).as_deref(),
        Some("display:contents;"),
        "{first}"
    );
    assert!(!css(&first, named).unwrap().contains("contents"), "{first}");
    assert!(
        !css(&first, styled).unwrap().contains("contents"),
        "{first}"
    );
    assert!(css(&first, pair).unwrap().contains("contents"), "{first}");
    // Its button lays it out as a block, with no anonymous item around it.
    let button = view_with_test_id(&host, "grow");
    assert!(
        css(&first, button).unwrap().ends_with("display:block;"),
        "{first}"
    );
    let named_button = view_with_test_id(&host, "named");
    assert!(
        !css(&first, named_button)
            .unwrap()
            .ends_with("display:block;"),
        "{first}"
    );
    // A block button centers its text in its height: one whose height is
    // its own, or a row's it stretches across, keeps its text a box.
    let (tall, stretched, flips) = (child("tall"), child("stretched"), child("flips"));
    assert!(!css(&first, tall).unwrap().contains("contents"), "{first}");
    assert!(
        !css(&first, stretched).unwrap().contains("contents"),
        "{first}"
    );
    assert!(css(&first, flips).unwrap().contains("contents"), "{first}");
    // A second text arrives: the first is a box again.
    let grown = host.dispatch(view_with_test_id(&host, "grow"), Event::Press);
    assert!(
        !css(&grown, pair).expect("restyled").contains("contents"),
        "{grown}"
    );
    let pair_box = view_with_test_id(&host, "pair");
    assert!(
        !css(&grown, pair_box)
            .expect("restyled")
            .ends_with("display:block;"),
        "{grown}"
    );
    // The row comes to stretch its button: the button's text is a box again.
    assert!(
        !css(&grown, flips).expect("restyled").contains("contents"),
        "{grown}"
    );
    assert!(
        !css(&grown, view_with_test_id(&host, "flips"))
            .expect("restyled")
            .ends_with("display:block;"),
        "{grown}"
    );
}
