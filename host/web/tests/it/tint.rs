//! A tint (LLP 1011 §3, LLP 1062 D6): a raster's alpha masking the
//! registered `--exact-tint`, which the browser also transitions.

use exact_runner::Event;
use exact_web::Host;

fn view_with_test_id(host: &Host<caltrain_data::Caltrain>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn a_tinted_raster_is_its_alpha_masking_the_tint() {
    let plan = contract::compile(
        r##"component App
  state wide = false
  action toggle writes wide
    wide = not wide
  view
    column
      button "Toggle" press=toggle testId="toggle"
      image "assets/mark.png" testId="mark" width=80 height=40 object-fit=(wide ? "cover" : "contain") tint-color="light-dark(#000000, #ffffff)"
      image "assets/mark.png" testId="down" object-fit="scale-down" tint-color="#ff0000" transition="tint-color 200ms linear"
      image "assets/mark.png" testId="plain" object-fit="cover"
      image "symbol:search" testId="symbol" tint-color="#ff0000"
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
    let create = |test_id: &str| {
        let id = view_with_test_id(&host, test_id);
        let at = first.find(&format!("\"id\":{id},\"tag\":\"img\"")).unwrap();
        first[at..].split("\"op\":").next().unwrap().to_string()
    };
    let mark = create("mark");
    assert!(mark.contains("--exact-tint:light-dark("), "{mark}");
    assert!(
        mark.contains("background-color:var(--exact-tint);mask-image:url(\\\"assets/mark.png\\\");mask-size:contain;"),
        "{mark}"
    );
    assert!(
        mark.contains("mask-origin:content-box;mask-clip:content-box;object-position:-100000px 0;"),
        "{mark}"
    );
    assert!(
        mark.contains("\"src\":\"assets/mark.png\""),
        "the page still loads the picture: {mark}"
    );
    let down = create("down");
    assert!(down.contains("mask-size:var(--exact-tint-fit,contain);"));
    assert!(
        down.contains("transition:--exact-tint 0.2s linear 0s;"),
        "{down}"
    );
    assert!(!create("plain").contains("mask-image"));
    assert!(
        !create("symbol").contains("mask-image:url("),
        "a symbol's mask is the glue's"
    );
    let wide = host.dispatch(view_with_test_id(&host, "toggle"), Event::Press);
    assert!(
        wide.contains("mask-size:cover;"),
        "the mask follows object-fit: {wide}"
    );
}
