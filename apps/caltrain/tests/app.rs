//! The v1 app end to end: `app.contract` → plan → bake → runner → kernel.
//! `rules/NOT-DOING.md`'s bar, on the one surface that exists so far.

use exact_kernel::{Kernel, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{DataSource, Event, Runner};

fn text_of<D: DataSource>(r: &Runner<D>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}

fn view_of<D: DataSource>(r: &Runner<D>, test_id: &str) -> u32 {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

fn ids_with_prefix<D: DataSource>(r: &Runner<D>, prefix: &str) -> Vec<String> {
    let k = r.kernel();
    let mut out = Vec::new();
    for root in k.roots() {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let n = k.node(id).unwrap();
            if let Some(t) = n.props.str(PropId::TestId) {
                if t.starts_with(prefix) && t != "station-name" && t != "station-search" {
                    out.push(t.to_string());
                }
            }
            let mut c = n.children();
            c.reverse();
            stack.extend(c);
        }
    }
    out
}

#[test]
fn the_app_compiles_deterministically_and_bakes_its_first_frame() {
    let a = caltrain::compile().unwrap();
    let b = caltrain::compile().unwrap();
    assert_eq!(a.encode(), b.encode(), "compiling twice is byte-identical");
    assert_eq!(a.kernel_schema_digest, exact_kernel::SCHEMA_DIGEST);
    // nowMs, screen, stationId, query; material, deck, focus (LLP 1014: the
    // sky's material, the card deck and its focus); sky (whether the app is
    // inside the aurora canvas at all — a phone's frame-rate switch);
    // hoverId, hoverOn, searchFocused, lastKey (the events beyond press and
    // change, LLP 1005 §3); deckLoaded, deckLoads, and deckMessage (the
    // iframe fixture, @ref LLP 1020 M1); and `nav`, the routes table's
    // location (LLP 1038 D2), there for `render=build` (LLP 1048.003 D5).
    assert_eq!(a.slots.len(), 16);
    // Seven data-crate resources, and `delivery` — the runner's own
    // `exactDelivery` (LLP 1030 D7), read by the update banner.
    assert_eq!(a.resources.len(), 8);
    // …and `activateUpdate`, the banner's `deliveryActivate`.
    assert_eq!(a.actions.len(), 18);
    assert_eq!(a.timers.len(), 1);
    assert!(
        a.resources.iter().all(|r| r.initial.len == 0),
        "unbaked: no compiled data yet"
    );

    let baked = caltrain::build().unwrap();
    assert!(
        baked.resources.iter().all(|r| r.initial.len > 0),
        "baked: every resource has its boot value"
    );
    let bytes = baked.encode();
    let decoded = Plan::decode(&bytes).unwrap();
    assert_eq!(decoded, baked);
    assert_eq!(
        caltrain::build().unwrap().encode(),
        bytes,
        "baking twice is byte-identical"
    );
}

#[test]
fn the_first_frame_needs_no_data_source() {
    /// A source that refuses everything: the baked plan must not ask.
    struct Refusing;
    impl exact_runner::DataSource for Refusing {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
            Err(exact_runner::DataError::Unavailable(format!(
                "boot asked for {source}"
            )))
        }
    }
    let baked = caltrain::build().unwrap();
    let r = Runner::boot(
        baked,
        Refusing,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(
        text_of(&r, "station-name").as_deref(),
        Some("Mountain View")
    );
    assert!(r.kernel().live_count() > 20);
}

#[test]
fn the_home_screen_shows_both_boards_with_live_countdowns() {
    let mut r = caltrain::boot(caltrain::build().unwrap(), Kernel::with_monospace()).unwrap();
    assert_eq!(
        text_of(&r, "station-name").as_deref(),
        Some("Mountain View")
    );
    assert!(text_of(&r, "home-screen").is_none());
    let north = ids_with_prefix(&r, "dep-mv-north");
    let south = ids_with_prefix(&r, "dep-mv-south");
    assert!(
        north.len() > 3 && south.len() > 3,
        "upcoming trains in both directions: {} / {}",
        north.len(),
        south.len()
    );
    // The first northbound departure after 11:10 UTC on the seeded schedule.
    let first = ids_with_prefix(&r, "countdown-mv-north");
    let countdown = text_of(&r, &first[0]).unwrap();
    let minutes: f64 = countdown.parse().unwrap();
    assert!(minutes > 0.0 && minutes < 60.0, "{countdown}");
    // Layout runs on the kernel the runner drives.
    let root = r.roots()[0];
    let receipt = r
        .kernel_mut()
        .compute_layout(root, exact_kernel::Offer::definite(390.0, 844.0))
        .unwrap();
    assert!(!receipt.changed.is_empty());
    let name = r.kernel().find_by_test_id("station-name")[0];
    let frame = r.kernel().node_by_key(name).unwrap().frame;
    assert!(frame.width > 0.0 && frame.height > 0.0);

    // A minute of ticks under the seekable clock: the countdown drops by one.
    r.advance(60_000.0).unwrap();
    let later: f64 = text_of(&r, &first[0]).unwrap().parse().unwrap();
    assert_eq!(later, minutes - 1.0);
}

#[test]
fn the_home_screen_explains_when_service_has_ended() {
    let now = caltrain_data::DAY_START_MS + 24.0 * 60.0 * 60_000.0;
    let plan = contract::compile(&caltrain::SOURCE.replace(
        "state nowMs = 1787915400000",
        &format!("state nowMs = {now}"),
    ))
    .unwrap();
    let r = caltrain::boot(plan, Kernel::with_monospace()).unwrap();
    let messages = r.kernel().find_by_test_id("board-north");
    assert_eq!(messages.len(), 1, "the north board stays present");
    assert!(r.kernel().roots().iter().any(|_| {
        let mut found = false;
        for root in r.kernel().roots() {
            let mut stack = vec![root];
            while let Some(id) = stack.pop() {
                let node = r.kernel().node(id).unwrap();
                found |= node.props.str(PropId::Text) == Some("No more trains today");
                stack.extend(node.children());
            }
        }
        found
    }));
}

#[test]
fn changing_station_re_requests_the_boards_and_search_filters_by_key() {
    let mut r = caltrain::boot(caltrain::build().unwrap(), Kernel::with_monospace()).unwrap();
    r.dispatch(view_of(&r, "change-station"), Event::Press)
        .unwrap();
    assert_eq!(r.slot("screen"), Some(&Value::str("stations")));
    let nearest = ids_with_prefix(&r, "station-");
    assert_eq!(
        &nearest[..3],
        ["station-mv", "station-sunnyvale", "station-paloalto"],
        "nearest three to Mountain View, then all"
    );
    assert_eq!(nearest.len(), 3 + caltrain_data::STATIONS.len());

    r.dispatch(view_of(&r, "station-search"), Event::Change("san".into()))
        .unwrap();
    let matches = ids_with_prefix(&r, "station-");
    assert_eq!(matches, ["station-sf", "station-sanmateo", "station-sj"]);

    r.dispatch(view_of(&r, "station-sf"), Event::Press).unwrap();
    assert_eq!(r.slot("stationId"), Some(&Value::some(Value::str("sf"))));
    assert_eq!(r.slot("screen"), Some(&Value::str("home")));
    assert_eq!(r.slot("query"), Some(&Value::str("")));
    assert_eq!(
        text_of(&r, "station-name").as_deref(),
        Some("San Francisco")
    );
    assert!(
        ids_with_prefix(&r, "dep-sf-north").is_empty(),
        "nothing goes north of San Francisco"
    );
    assert!(ids_with_prefix(&r, "dep-sf-south").len() > 3);
    assert_eq!(text_of(&r, "board-north"), None);

    // Theme is a command out the side, not app state.
    r.dispatch(view_of(&r, "scheme-dark"), Event::Press)
        .unwrap();
    let commands = r.take_commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "setScheme");
    assert_eq!(commands[0].args, vec![Value::str("dark")]);
}

/// A data source that counts what it is asked.
struct Counting {
    inner: caltrain_data::Caltrain,
    queries: usize,
}

impl DataSource for Counting {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, exact_runner::DataError> {
        self.queries += 1;
        self.inner.query(source, args)
    }
}

#[test]
fn a_reload_keeps_its_place_and_re_requests_only_what_changed() {
    let counting = || Counting {
        inner: caltrain_data::Caltrain,
        queries: 0,
    };
    let mut r = Runner::boot(
        caltrain::build().unwrap(),
        counting(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.dispatch(view_of(&r, "change-station"), Event::Press)
        .unwrap();
    r.dispatch(view_of(&r, "station-sf"), Event::Press).unwrap();
    r.advance(30_000.0).unwrap();
    assert_eq!(
        text_of(&r, "station-name").as_deref(),
        Some("San Francisco")
    );
    let carried = r.carry();
    assert_eq!(carried.slots.len(), 16);
    assert_eq!(carried.now_ms, 30_000.0);

    // The edited plan: unbaked (no compiled data) and with a visible change.
    let edited = contract::compile(
        &caltrain::SOURCE.replace("text \"Caltrain\" ", "text \"Caltrain Live\" "),
    )
    .unwrap();
    let mut again = Runner::boot_carrying(
        edited,
        counting(),
        Kernel::with_monospace(),
        &carried,
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(
        again.slot("stationId"),
        Some(&Value::some(Value::str("sf")))
    );
    assert_eq!(again.slot("screen"), Some(&Value::str("home")));
    assert_eq!(
        text_of(&again, "station-name").as_deref(),
        Some("San Francisco")
    );
    assert!(
        ids_with_prefix(&again, "dep-sf-south").len() > 3,
        "the boards are San Francisco's"
    );
    assert_eq!(again.now_ms(), 30_000.0, "the clock carries");
    assert_eq!(
        again.data().queries,
        0,
        "every resource's arguments still matched: nothing was re-requested"
    );
    let root = again.roots()[0];
    let k = again.kernel();
    let mut stack = vec![root];
    let mut found = false;
    while let Some(id) = stack.pop() {
        let node = k.node(id).unwrap();
        if node.props.str(PropId::Text) == Some("Caltrain Live") {
            found = true;
        }
        stack.extend(node.children());
    }
    assert!(found, "the edit is on screen");

    // A tick after the reload fires the timer from the carried clock.
    let receipts = again.advance(31_000.0).unwrap();
    assert!(
        !receipts.is_empty(),
        "the ticker runs from the carried clock"
    );
}

/// The events beyond press and change (LLP 1005 §3): a hover carries
/// whether the pointer is over, a key its name, focus and blur nothing —
/// each reaching the slot its action writes, and the view following.
#[test]
fn hover_focus_and_keys_reach_their_actions() {
    let mut r = Runner::boot(
        caltrain::build().unwrap(),
        caltrain_data::Caltrain,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.dispatch(view_of(&r, "change-station"), Event::Press)
        .unwrap();
    let sf = view_of(&r, "station-sf");
    r.dispatch(sf, Event::Hover(true)).unwrap();
    assert_eq!(
        r.kernel().find_by_test_id("station-hot-sf").len(),
        1,
        "the hovered row is highlighted"
    );
    r.dispatch(sf, Event::Hover(false)).unwrap();
    assert!(
        r.kernel().find_by_test_id("station-hot-sf").is_empty(),
        "the highlight goes when the pointer leaves"
    );
    let search = view_of(&r, "station-search");
    assert!(r.kernel().find_by_test_id("search-hint").is_empty());
    r.dispatch(search, Event::Focus).unwrap();
    r.dispatch(search, Event::Key("Enter".into())).unwrap();
    assert_eq!(
        text_of(&r, "search-hint").as_deref(),
        Some("searching · last key Enter")
    );
    r.dispatch(search, Event::Blur).unwrap();
    assert!(r.kernel().find_by_test_id("search-hint").is_empty());
    // A hover on a node without a hover handler is a typed refusal.
    let err = r.dispatch(view_of(&r, "station-name"), Event::Hover(true));
    assert!(matches!(
        err,
        Err(exact_runner::RunnerError::NoHandler { event: "hover", .. })
    ));
}
