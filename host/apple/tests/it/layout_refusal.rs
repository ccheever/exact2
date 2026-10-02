//! @ref LLP 1039 D2 — a valid viewport can still fail the first layout.
use exact_apple::abi::{Bridge, Hooks};
use exact_runner::{DataError, DataSource, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected data source {name}")
    }
}

fn plan(text: &str, fails_layout: bool) -> Vec<u8> {
    // Finite, accepted style; the ABI's default measurer overflows its line
    // height at layout, yielding InvalidTextMetrics after viewport validation.
    let size = if fails_layout {
        "300000000000000000000000000000000000000"
    } else {
        "16"
    };
    contract::compile(&format!(
        "component App\n  view\n    text \"{text}\" font-size={size}\n"
    ))
    .unwrap()
    .encode()
}

fn assert_batch(bridge: &Bridge<NoData>, n: u32, expected: &str) {
    let batch = String::from_utf8_lossy(bridge.output_bytes(n as usize));
    assert!(batch.contains(expected), "{batch}");
}

fn assert_running(bridge: &mut Bridge<NoData>) {
    let n = bridge.resize(500.0, 844.0);
    assert_batch(bridge, n, "\"error\":null");
    assert_batch(bridge, n, "\"w\":500");
    let n = bridge.input_write(br#"{"op":"tree"}"#);
    let n = bridge.agent(n);
    assert_batch(bridge, n, "running");
    let tree = String::from_utf8_lossy(bridge.output_bytes(n as usize));
    assert!(!tree.contains("candidate"), "{tree}");
}

fn running() -> Bridge<NoData> {
    let mut bridge = Bridge::new();
    let n = bridge.boot(&plan("running", false), NoData, Hooks::none(), 390.0, 844.0);
    assert_batch(&bridge, n, "\"error\":null");
    bridge
}

#[test]
fn a_layout_failure_with_a_valid_initial_viewport_publishes_no_host() {
    let mut bridge = Bridge::new();
    let n = bridge.boot(
        &plan("candidate", true),
        NoData,
        Hooks::none(),
        390.0,
        844.0,
    );
    assert_batch(&bridge, n, "InvalidTextMetrics");
    let n = bridge.resize(500.0, 844.0);
    assert_batch(&bridge, n, "not booted");
}

#[test]
fn a_layout_failure_with_a_valid_plan_viewport_keeps_the_running_host() {
    let mut bridge = running();
    let bytes = plan("candidate", true);
    bridge.input_write(&bytes);
    let n = bridge.boot_plan(bytes.len(), NoData, Hooks::none(), 390.0, 844.0);
    assert_batch(&bridge, n, "InvalidTextMetrics");
    assert_running(&mut bridge);
}

#[test]
fn a_layout_failure_with_a_valid_fresh_viewport_keeps_the_running_host() {
    let mut bridge = running();
    let n = bridge.boot(
        &plan("candidate", true),
        NoData,
        Hooks::none(),
        390.0,
        844.0,
    );
    assert_batch(&bridge, n, "InvalidTextMetrics");
    assert_running(&mut bridge);
}

#[test]
fn two_prepared_sessions_keep_live_hosts_when_one_valid_viewport_layout_fails() {
    let mut a = running();
    let mut b = running();
    let good = plan("candidate", false);
    let bad = plan("candidate", true);
    a.input_write(&good);
    b.input_write(&bad);
    let n = a.prepare_plan(good.len(), NoData, Hooks::none(), 390.0, 844.0);
    assert_batch(&a, n, "\"error\":null");
    let n = b.prepare_plan(bad.len(), NoData, Hooks::none(), 390.0, 844.0);
    assert_batch(&b, n, "InvalidTextMetrics");
    a.discard_plan();
    b.discard_plan();
    for bridge in [&mut a, &mut b] {
        assert_running(bridge);
        bridge.input_write(&good);
        let n = bridge.prepare_plan(good.len(), NoData, Hooks::none(), 390.0, 844.0);
        assert_batch(bridge, n, "\"error\":null");
    }
    for bridge in [&mut a, &mut b] {
        bridge.commit_plan();
        let n = bridge.input_write(br#"{"op":"tree"}"#);
        let n = bridge.agent(n);
        assert_batch(bridge, n, "candidate");
    }
}

#[test]
fn url_location_boot_and_live_navigate_share_the_apple_buffer_abi() {
    // @ref LLP 1038 D5/D8 — both ordinary and prepared first boots use the URL.
    let source = "routes nav\n  home \"/\"\n    post \"/post/:post\"\ncomponent App\n  state first = top(nav).url\n  action follow(location: string)\n    nav = open(nav, location)\n  view\n    main navigate=follow navigationKey=`${top(nav).id}` navigationBack=\"back\"\n      text top(nav).url\n";
    let plan = contract::compile(source).unwrap().encode();
    for prepared in [false, true] {
        let mut bridge = Bridge::new();
        let n = bridge.input_write(b"s2b://post/42?q=a b#fragment");
        let n = bridge.location_of(n);
        let location = bridge.output_bytes(n as usize).to_vec();
        assert_eq!(location, b"/post/42?q=a%20b");
        let n = bridge.input_write(&location);
        bridge.set_launch_location(n);
        let n = if prepared {
            let n = bridge.input_write(&plan);
            bridge.boot_plan(n, NoData, Hooks::none(), 390.0, 844.0)
        } else {
            bridge.boot(&plan, NoData, Hooks::none(), 390.0, 844.0)
        };
        assert_batch(&bridge, n, "/post/42?q=a%20b");
        let n = bridge.input_write(br#"{"op":"state"}"#);
        let n = bridge.agent(n);
        assert_batch(&bridge, n, "\"first\":\"/post/42?q=a%20b\"");
        let n = bridge.input_write(b"/post/7");
        let n = bridge.dispatch(1, 14, n, 0.0);
        assert_batch(&bridge, n, "/post/7");
        let n = bridge.input_write(br#"{"op":"logs"}"#);
        let n = bridge.agent(n);
        assert_eq!(
            String::from_utf8_lossy(bridge.output_bytes(n as usize))
                .matches("navigate view")
                .count(),
            1
        );
    }
}
