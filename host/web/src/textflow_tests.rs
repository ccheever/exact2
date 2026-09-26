//! @ref LLP 1043.000 §3 D7/D8 — host contexts and committed positions.
#[test]
fn host_announces_contexts_inline_paragraphs_and_css_without_taffy_layout() {
    let plan = contract::compile(r#"component Test
  view
    box width=200 height=200
      text testId="para" height=180
        text "First "
        text "link" href="https://example.test"
      box position="absolute" width=20 height=20 wrap-flow="both" shape-outside="circle()" shape-margin=3
        text "inside"
      text "auto" testId="auto"
"#).unwrap();
    let (mut host, batch) = crate::Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(batch.contains("\"op\":\"textflow\""));
    assert_eq!(batch.matches("\"definite\":true").count(), 1);
    assert_eq!(batch.matches("\"definite\":false").count(), 1);
    assert!(batch.contains("shape-margin:3px;"));
    assert!(batch.contains("data-wrap-flow"));
    assert!(!host.advance(16.).contains("\"op\":\"textflow\""));
    let plain = contract::compile("component Test\n  view\n    text \"plain\"\n").unwrap();
    let (_, batch) = crate::Host::boot(
        &plain.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!batch.contains("textflow"));
}

#[test]
fn pan_abi_20_commits_deltas_and_refuses_nonfinite_payload_before_clock() {
    let plan = contract::compile(
        r#"component Test
  state x = 0
  action move(dx: number, dy: number) writes x
    x = x + dx + dy
  action playback(seconds: number) writes x
    x = seconds
  view
    box pan=move testId="pan"
      text `${x}`
      video timeupdate=playback testId="video"
"#,
    )
    .unwrap();
    let mut bridge = crate::abi::Bridge::new();
    let n = bridge.boot(&plan.encode(), caltrain_data::Caltrain, 300., 200., "/");
    let batch = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    let view_id = |name: &str| {
        let create = batch
            .split("\"op\":\"create\"")
            .find(|part| {
                part.split("\"op\"")
                    .next()
                    .unwrap()
                    .contains(&format!("\"data-testid\":\"{name}\""))
            })
            .unwrap();
        create
            .split("\"id\":")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let id = view_id("pan");
    let video = view_id("video");
    bridge.input_write(b"10,-3");
    let n = bridge.dispatch(id, 20, 5, 16.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"7\""), "{out}");
    bridge.input_write(b"NaN,1");
    let n = bridge.dispatch(id, 20, 5, f64::INFINITY);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid pan deltas"));
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 20, 3, 17.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"9\""), "{out}");
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 18, 3, 18.);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid reorder event"));
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 19, 3, 18.);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid media event"));
    bridge.input_write(b"timeupdate\n100");
    let n = bridge.dispatch(video, 19, 14, 19.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"100\""), "{out}");
    bridge.input_write(b"1,1");
    let n = bridge.dispatch(id, 20, 3, 20.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"text\":\"102\""), "{out}");
}

// @ref LLP 1043.000 §8 — the kernel's admission rule reaches the browser's executor.
#[test]
fn a_drop_cap_admits_its_auto_height_paragraph_and_a_flex_context_refuses() {
    let source = |container: &str| {
        format!(
            r#"component Test
  view
    {container} width=400 padding=20 position="relative"
      box position="absolute" left=20 top=20 width=58 height=58 wrap-flow="both" shape-outside="inset(0)" shape-margin=6
        text "T" font-size=64 line-height=1
      text "here is an hour when the garden belongs to neither day nor night." testId="lede"
"#
        )
    };
    let boot = |container: &str| {
        let plan = contract::compile(&source(container)).unwrap();
        crate::Host::boot(
            &plan.encode(),
            caltrain_data::Caltrain,
            Default::default(),
            "/",
        )
        .unwrap()
        .1
    };
    let batch = boot("box");
    assert!(
        batch.contains("\"definite\":false,\"refusal\":null"),
        "{batch}"
    );
    let batch = boot("column");
    assert!(
        batch.contains(&format!(
            "\"definite\":false,\"refusal\":\"{}\"",
            exact_kernel::FlowRefusal::Context.message()
        )),
        "{batch}"
    );
}
