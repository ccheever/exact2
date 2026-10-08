use super::*;
use exact_linux::app::{boot_presenter, Config};
use exact_runner::DataSource;
use exact_runner::{DataError, Value};
use exact_update::{sha256_hex, Client, Outcome};
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const UPDATE_COMPAT: &str = r#"{"id":"fixture00000000","inputs":{"app":"com.exact.fixture","keys":null,"trust":"development","store":{"L":"A"}},"delivery":{"channel":"prod","origin":"https://updates.example"}}"#;
/// Dev binaries check only a named origin; name the fixture's.
const ORIGIN: &str = "https://updates.example";

#[derive(Default)]
struct Named;

impl DataSource for Named {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn app_id(&self) -> &str {
        "com.exact.fixture"
    }
}

fn stage(dir: &Path, baked: &[u8], plan: &[u8], assets: &[(&str, Vec<u8>)]) -> Client {
    checked(dir, baked, plan, assets, false)
}

fn checked(
    dir: &Path,
    baked: &[u8],
    plan: &[u8],
    assets: &[(&str, Vec<u8>)],
    running: bool,
) -> Client {
    let mut client = Client::open_at(dir, dir, UPDATE_COMPAT, baked, Some(ORIGIN)).unwrap();
    if running {
        client.boot_started().unwrap();
    }
    let head_url = client.head_url().unwrap().to_string();
    let plan_url = head_url.replace("exact.json", "app.plan");
    let cards: Vec<_> = assets
        .iter()
        .map(|(name, bytes)| {
            serde_json::json!({
                "name": name,
                "url": format!("./{name}"),
                "sha256": sha256_hex(bytes),
                "bytes": bytes.len()
            })
        })
        .collect();
    let head = serde_json::to_vec(&serde_json::json!({
            "exact": 1,
            "app": { "id": "com.exact.fixture", "name": "Fixture" },
            "plan": { "url": "./app.plan", "sha256": sha256_hex(plan), "bytes": plan.len() },
            "assets": cards,
            "stream": { "app": "com.exact.fixture", "channel": "prod", "compatibilityId": "fixture00000000", "seq": 1 }
        }))
        .unwrap();
    let mut fetch = |url: &str| {
        if url == head_url {
            Ok(head.clone())
        } else if url == plan_url {
            Ok(plan.to_vec())
        } else if let Some((_, bytes)) = assets
            .iter()
            .find(|(name, _)| url.ends_with(&format!("/{name}")))
        {
            Ok(bytes.clone())
        } else {
            Err(format!("unexpected fetch {url}"))
        }
    };
    assert!(matches!(
        client.check(&mut fetch),
        Outcome::Staged { seq: 1, .. }
    ));
    if running {
        client
    } else {
        drop(client);
        Client::open_at(dir, dir, UPDATE_COMPAT, baked, Some(ORIGIN)).unwrap()
    }
}

fn selected_config(dir: &Path, baked: &[u8], client: Client) -> Config {
    let mut config = Config {
        content_region: None,
        launch: "/".into(),
        plan: baked.to_vec().into(),
        fallback_plan: None,
        assets: dir.to_path_buf(),
        scale: 1.0,
        size: (390.0, 844.0),
        agent: false,
        smoke: false,
        shot: None,
        dev_plan: None,
        dev_url: None,
        dev_identity: None,
        card: String::new(),
        vnc: None,
        compat: UPDATE_COMPAT.into(),
        explicit: false,
        entry: None,
        selected_assets: None,
        module: Ok(None),
        updates: None,
        hatches: None,
    };
    config.use_updates(Box::new(Updates::from_client(client).unwrap()), baked);
    config
}

#[test]
fn an_analysis_boot_refuses_before_touching_an_attached_selection() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let candidate = contract::compile("component App\n  view\n    text \"selected\"\n")
        .unwrap()
        .encode();
    let dir = std::env::temp_dir().join(format!("exact-linux-analysis-{}", std::process::id()));
    let client = stage(&dir, &baked, &candidate, &[]);
    let record = client.dir().join("record.json");
    let before = std::fs::read(&record).unwrap();
    let mut config = selected_config(&dir, &baked, client);
    config.compat =
        r#"{"inputs":{"store":{"L":"A"}},"embedded":{"analysis":true,"seq":null}}"#.into();
    let error = boot_presenter::<Named>(&mut config, (390.0, 844.0))
        .err()
        .expect("analysis cannot boot either selected or fallback bytes");
    assert!(error.contains("compatibility analysis artifact"), "{error}");
    assert!(
        config.updates.is_some(),
        "refusal precedes taking the store"
    );
    assert_eq!(std::fs::read(record).unwrap(), before);
    drop(config);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_fetched_plan_refused_at_boot_falls_back_to_baked() {
    let source = "component App\n  view\n    text \"ok\"\n";
    let mut foreign = contract::compile(source).unwrap();
    foreign.app_id = "com.exact.foreign".into();
    let baked = contract::compile(source).unwrap().encode();
    let mut config = Config {
        content_region: None,
        launch: "/".into(),
        plan: foreign.encode().into(),
        fallback_plan: Some(baked),
        assets: std::env::current_dir().unwrap(),
        scale: 1.0,
        size: (390.0, 844.0),
        agent: false,
        smoke: false,
        shot: None,
        dev_plan: None,
        dev_url: None,
        dev_identity: None,
        card: String::new(),
        vnc: None,
        compat: r#"{"id":"fixture00000000","inputs":{"store":{"L":"0"}}}"#.into(),
        explicit: true,
        entry: None,
        selected_assets: None,
        module: Ok(None),
        updates: None,
        hatches: None,
    };
    let size = config.size;
    let (presenter, error) = boot_presenter::<Named>(&mut config, size).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(presenter.node_count(), 1);
    // The binary's facts reached the runner past the fallback (LLP 1030 D7).
    assert_eq!(presenter.host().runner().delivery().store, '0');
}

#[test]
fn a_partial_initial_dev_plan_falls_back_without_counting_the_store() {
    let source = "component App\n  view\n    text \"baked\"\n";
    let baked = contract::compile(source).unwrap().encode();
    let update = contract::compile("component App\n  view\n    text \"selected\"\n")
        .unwrap()
        .encode();
    let dir =
        std::env::temp_dir().join(format!("exact-linux-dev-precedence-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let compat = UPDATE_COMPAT;
    let mut client = Client::open_at(&dir, Path::new("."), compat, &baked, Some(ORIGIN)).unwrap();
    let head_url = client.head_url().unwrap().to_string();
    let plan_url = head_url.replace("exact.json", "app.plan");
    let head = serde_json::to_vec(&serde_json::json!({
            "exact": 1,
            "app": { "id": "com.exact.fixture", "name": "Fixture" },
            "plan": { "url": "./app.plan", "sha256": sha256_hex(&update), "bytes": update.len() },
            "assets": [],
            "stream": { "app": "com.exact.fixture", "channel": "prod", "compatibilityId": "fixture00000000", "seq": 1 }
        }))
        .unwrap();
    let mut fetch = |url: &str| {
        if url == head_url {
            Ok(head.clone())
        } else if url == plan_url {
            Ok(update.clone())
        } else {
            Err(format!("unexpected fetch {url}"))
        }
    };
    assert!(matches!(
        client.check(&mut fetch),
        Outcome::Staged { seq: 1, .. }
    ));
    let record = client.dir().join("record.json");
    let updates = Updates::from_client(client).unwrap();
    let mut config = Config {
        content_region: None,
        launch: "/".into(),
        plan: b"EXPL".to_vec().into(), // the compiler was interrupted mid-write
        fallback_plan: Some(baked.clone()),
        assets: PathBuf::from("."),
        scale: 1.0,
        size: (390.0, 844.0),
        agent: false,
        smoke: false,
        shot: None,
        dev_plan: Some(PathBuf::from("app.plan")),
        dev_url: None,
        dev_identity: None,
        card: String::new(),
        vnc: None,
        compat: compat.into(),
        explicit: true,
        entry: None,
        selected_assets: None,
        module: Ok(None),
        updates: None,
        hatches: None,
    };
    config.use_updates(Box::new(updates), &baked);
    assert_eq!(&*config.plan, b"EXPL");
    assert!(config.entry.is_none(), "the selected entry did not win");
    let (presenter, _) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
    assert_eq!(presenter.node_count(), 1, "the baked plan booted");
    let saved = std::fs::read_to_string(record).unwrap();
    assert!(
        saved.contains("\"failures\":0"),
        "the store boot was not counted: {saved}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_selected_image_loads_from_its_verified_generation() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let selected =
        contract::compile("shape Delivery\n  seq: number\ncomponent App\n  resource delivery = exactDelivery() as shape Delivery\n  view\n    column\n      when delivery.seq > 0\n        image \"assets/mark.png\" width=96\n      else\n        text \"embedded\"\n")
            .unwrap()
            .encode();
    let png = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/caltrain/assets/caltrain.png"
    ))
    .unwrap();
    let dir =
        std::env::temp_dir().join(format!("exact-linux-selected-asset-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(&dir, &baked, &selected, &[("assets/mark.png", png)]);
    let mut config = selected_config(&dir, &baked, client);

    let (mut presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
    assert!(error.is_none(), "{error:?}");
    presenter.wait_images(Duration::from_secs(2));
    assert_eq!(
        presenter.images().loaded,
        vec![("assets/mark.png".to_string(), (320, 120))]
    );
    assert_eq!(
        presenter.host().runner().delivery().stream,
        "prod/fixture00000000"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_selected_font_loads_from_its_verified_generation() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/fonts");
    let selected = contract::compile_path_source(
            &fixture.join("app.contract"),
            "font \"Body\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"selected\" font-family=\"Body\"\n",
        )
        .unwrap()
        .encode();
    let font = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/fixtures/fonts/assets/DejaVuSans.ttf"
    ))
    .unwrap();
    let dir =
        std::env::temp_dir().join(format!("exact-linux-selected-font-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(&dir, &baked, &selected, &[("assets/DejaVuSans.ttf", font)]);
    let mut config = selected_config(&dir, &baked, client);

    let (presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
    assert!(error.is_none(), "{error:?}");
    let text = presenter.text().clone();
    let mut text = text.borrow_mut();
    let declared = text
        .declared_face_id(8, 400, false)
        .expect("the selected face bytes were registered");
    assert_eq!(text.resolved_face_id(8, 400, false), Some(declared));
    assert_eq!(
        presenter.host().runner().delivery().stream,
        "prod/fixture00000000"
    );
    drop(text);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_absent_selected_asset_tombstones_the_embedded_file() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let selected =
        contract::compile("component App\n  view\n    image \"assets/caltrain.png\" width=96\n")
            .unwrap()
            .encode();
    let dir = std::env::temp_dir().join(format!(
        "exact-linux-selected-tombstone-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(&dir, &baked, &selected, &[]);
    let mut config = selected_config(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        &baked,
        client,
    );

    let (mut presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
    assert!(error.is_none(), "{error:?}");
    presenter.wait_images(Duration::from_millis(50));
    assert!(presenter.images().loaded.is_empty());
    assert_eq!(
        presenter.host().runner().delivery().stream,
        "prod/fixture00000000"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_corrupt_selected_asset_falls_back_before_first_pixel() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let selected =
        contract::compile("shape Delivery\n  seq: number\ncomponent App\n  resource delivery = exactDelivery() as shape Delivery\n  view\n    column\n      when delivery.seq > 0\n        image \"assets/mark.png\" width=96\n      else\n        text \"embedded\"\n")
            .unwrap()
            .encode();
    let dir = std::env::temp_dir().join(format!(
        "exact-linux-corrupt-selected-asset-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(
        &dir,
        &baked,
        &selected,
        &[("assets/mark.png", b"signed asset".to_vec())],
    );
    let asset = client
        .selection()
        .assets_dir
        .unwrap()
        .join("assets/mark.png");
    std::fs::write(asset, b"corrupt").unwrap();
    let record = client.dir().join("record.json");
    let mut config = selected_config(&dir, &baked, client);

    let (presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(presenter.node_count(), 1, "entry zero booted");
    assert_eq!(presenter.host().runner().delivery().stream, "embedded");
    assert!(config.selected_assets.is_none());
    let saved = std::fs::read_to_string(record).unwrap();
    assert!(saved.contains("\"selected\":null"), "{saved}");
    assert!(saved.contains("\"failures\":0"), "{saved}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn activation_refuses_carried_layout_without_advancing_then_commits_after_repair() {
    let source = "component App\n  state divisor = 1\n  action breakIt\n    divisor = 0\n  action repair\n    divisor = 1\n  view\n    column width=100\n      text \"running\"\n      button press=breakIt testId=\"break\"\n        text \"break\"\n      button press=repair testId=\"repair\"\n        text \"repair\"\n";
    let baked = contract::compile(source).unwrap().encode();
    // A derive must be a finite number: carried `divisor = 0` refuses the
    // candidate's boot (a bound style value would only be unset).
    let candidate = contract::compile(
        &source
            .replace("  view\n", "  derive width = 100 / divisor\n  view\n")
            .replace("width=100", "width=width")
            .replace("running", "candidate"),
    )
    .unwrap()
    .encode();
    let dir = std::env::temp_dir().join(format!("exact-linux-activation-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let client = checked(&dir, &baked, &candidate, &[], true);
    let record = client.dir().join("record.json");
    let (mut presenter, error) =
        exact_linux::Presenter::boot(&baked, Named, (390.0, 844.0), 1.0, dir.clone()).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(presenter.set_delivery_from_compat(UPDATE_COMPAT).is_none());
    presenter.set_updates(Some(Box::new(Updates::from_client(client).unwrap())));
    let view = |p: &exact_linux::Presenter<Named>, name: &str| {
        let kernel = p.host().kernel();
        kernel
            .node_by_key(kernel.find_by_test_id(name)[0])
            .unwrap()
            .id
    };
    presenter.tap(view(&presenter, "break")).unwrap();
    let tree = presenter.host().agent("{\"op\":\"tree\"}");
    let before = std::fs::read(&record).unwrap();
    assert!(presenter.activate_update(Named).is_err());
    assert_eq!(presenter.host().agent("{\"op\":\"tree\"}"), tree);
    assert_eq!(std::fs::read(&record).unwrap(), before);
    presenter.tap(view(&presenter, "repair")).unwrap();
    assert!(presenter.activate_update(Named).unwrap());
    assert!(presenter
        .host()
        .agent("{\"op\":\"tree\"}")
        .contains("candidate"));
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert!(saved["selected"].is_string());
    assert!(saved["pending"].is_null());
    assert_eq!(saved["failures"], 1, "only the accepted generation started");
    // A stale pre-activation frame must not bless the new running entry.
    presenter.first_pixel();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(saved["failures"], 1);
    let shot = dir.join("activated.png");
    let reply = exact_linux::agent::handle(
        &mut presenter,
        &serde_json::json!({"op":"screenshot", "path":shot}).to_string(),
    );
    assert!(!reply.contains("error"), "{reply}");
    assert!(shot.is_file());
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(saved["failures"], 0, "its rendered frame blesses it");
    assert_eq!(saved["lastGood"], saved["selected"]);
    // A successful frame's unchanged delivery facts must leave presentation idle.
    // Repeated reads still check first pixel and must not request another paint.
    assert!(!presenter.dirty());
    let tree = exact_linux::agent::handle(&mut presenter, r#"{"op":"tree"}"#);
    let record_after = std::fs::read(&record).unwrap();
    for _ in 0..3 {
        presenter.first_pixel();
        assert!(!presenter.dirty());
        assert_eq!(
            exact_linux::agent::handle(&mut presenter, r#"{"op":"tree"}"#),
            tree
        );
        assert!(!presenter.dirty());
        assert_eq!(std::fs::read(&record).unwrap(), record_after);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn verified_launch_refusals_are_demoted_after_failed_launches() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let bad_layout = contract::compile(
        "component App\n  view\n    text \"x\" font-size=300000000000000000000000000000000000000\n",
    )
    .unwrap()
    .encode();
    for (label, candidate) in [
        ("decode", b"EXPL".as_slice()),
        ("layout", bad_layout.as_slice()),
    ] {
        let dir = std::env::temp_dir().join(format!(
            "exact-linux-bad-launch-{label}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let client = stage(&dir, &baked, candidate, &[]);
        let record = client.dir().join("record.json");
        drop(client);
        for expected in 1..=2 {
            let client = Client::open_at(&dir, &dir, UPDATE_COMPAT, &baked, Some(ORIGIN)).unwrap();
            let mut config = selected_config(&dir, &baked, client);
            assert!(config.entry.is_some());
            let (mut presenter, error) =
                boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
            assert!(error.is_none(), "{error:?}");
            assert!(presenter
                .host()
                .agent("{\"op\":\"tree\"}")
                .contains("baked"));
            let _ = exact_linux::agent::handle(&mut presenter, "{\"op\":\"layout\"}");
            let saved: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
            assert_eq!(
                saved["failures"], expected,
                "fallback pixels cannot bless the refused selection"
            );
            assert!(saved["lastGood"].is_null());
            assert_eq!(
                saved["stream"]["seq"], 1,
                "the admitted sequence floor survives refusal"
            );
        }
        let client = Client::open_at(&dir, &dir, UPDATE_COMPAT, &baked, Some(ORIGIN)).unwrap();
        let config = selected_config(&dir, &baked, client);
        assert!(
            config.entry.is_none(),
            "the next open demotes the twice-refused entry"
        );
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
        assert!(saved["selected"].is_null());
        assert_eq!(saved["bad"].as_array().unwrap().len(), 1);
        assert_eq!(saved["stream"]["seq"], 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn a_crash_during_selected_app_construction_is_counted_before_it_happens() {
    struct Crashing;
    impl Default for Crashing {
        fn default() -> Self {
            panic!("app construction crashed before the host returned")
        }
    }
    impl DataSource for Crashing {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let selected = contract::compile("component App\n  view\n    text \"selected\"\n")
        .unwrap()
        .encode();
    let dir = std::env::temp_dir().join(format!(
        "exact-linux-crash-before-return-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(&dir, &baked, &selected, &[]);
    let record = client.dir().join("record.json");
    drop(client);
    for expected in 1..=2 {
        let client = Client::open_at(&dir, &dir, UPDATE_COMPAT, &baked, Some(ORIGIN)).unwrap();
        let mut config = selected_config(&dir, &baked, client);
        assert!(config.entry.is_some());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            boot_presenter::<Crashing>(&mut config, (390.0, 844.0))
        }))
        .is_err());
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
        assert_eq!(saved["failures"], expected);
        assert!(saved["lastGood"].is_null());
    }
    let client = Client::open_at(&dir, &dir, UPDATE_COMPAT, &baked, Some(ORIGIN)).unwrap();
    let mut config = selected_config(&dir, &baked, client);
    assert!(config.entry.is_none());
    let (mut presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(exact_linux::agent::handle(&mut presenter, "{\"op\":\"tree\"}").contains("baked"));
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(saved["bad"].as_array().unwrap().len(), 1);
    assert_eq!(saved["stream"]["seq"], 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_refused_initial_layout_releases_no_network_requests() {
    use exact_runner::{Answer, Request, Store};
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, OnceLock};
    static ORIGIN: OnceLock<(String, String)> = OnceLock::new();
    struct Seed;
    impl DataSource for Seed {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::record(vec![Value::str("")]))
        }
        fn answer(&mut self, store: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            let _ = store.get("fixture.pending");
            Ok(Answer::Now(Value::record(vec![Value::str("")])))
        }
    }
    #[derive(Default)]
    struct Later;
    impl DataSource for Later {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(&mut self, _: &mut Store, _: &str, args: &[Value]) -> Result<Answer, DataError> {
            assert_eq!(
                args,
                &[Value::Number(1.0)],
                "only the accepted delivery facts may reach the data source"
            );
            Ok(Answer::Later(Request::post_json(
                &ORIGIN.get().unwrap().0,
                "{}",
            )))
        }
        fn grants(&self) -> &'static str {
            &ORIGIN.get().unwrap().1
        }
        fn app_id(&self) -> &str {
            "com.exact.fixture"
        }
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    ORIGIN
        .set((origin.clone(), format!("net.fetch {origin}\n")))
        .unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let stopped = Arc::new(AtomicBool::new(false));
    let worker = {
        let requests = requests.clone();
        let stopped = stopped.clone();
        std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let _ = stream.read(&mut [0; 4096]);
                    requests.fetch_add(1, Ordering::SeqCst);
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    );
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        })
    };
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let source = "shape Reply\n  value: string\nshape Delivery\n  seq: number\ncomponent App\n  resource delivery = exactDelivery() as shape Delivery\n  resource reply = ping(delivery.seq) as shape Reply\n  view\n    text \"x\" font-size=SIZE\n";
    let dir = std::env::temp_dir().join(format!(
        "exact-linux-refused-network-{}",
        std::process::id()
    ));
    let result = std::panic::catch_unwind(|| {
        let seed = contract::bake(
            // @ref LLP 1038 D5 — bake a placeholder keyed to this candidate's
            // delivery sequence, so a Later answer may retain it at boot.
            contract::compile(
                &source
                    .replace("SIZE", "16")
                    .replace("ping(delivery.seq)", "ping(1)"),
            )
            .unwrap(),
            Seed,
        )
        .unwrap();
        for (label, size, expected) in [
            ("refused", "300000000000000000000000000000000000000", 0),
            ("accepted", "16", 1),
        ] {
            let root = dir.join(label);
            std::fs::create_dir_all(&root).unwrap();
            // Preserve an ordinary baked store-reader placeholder while
            // constructing the signed but layout-invalid candidate.
            let mut plan = contract::compile(&source.replace("SIZE", size)).unwrap();
            let initial = seed.bytes(seed.resources[1].initial);
            plan.resources[1].initial.offset = plan.data.len() as u32;
            plan.resources[1].initial.len = initial.len() as u32;
            plan.resources[1].reader = seed.resources[1].reader;
            plan.data.to_mut().extend_from_slice(initial);
            let initial_args = seed.bytes(seed.resources[1].initial_args);
            plan.resources[1].initial_args.offset = plan.data.len() as u32;
            plan.resources[1].initial_args.len = initial_args.len() as u32;
            plan.data.to_mut().extend_from_slice(initial_args);
            let plan = plan.encode();
            let client = stage(&root, &baked, &plan, &[]);
            let mut config = selected_config(&root, &baked, client);
            let (presenter, error) = boot_presenter::<Later>(&mut config, (390.0, 844.0)).unwrap();
            assert!(error.is_none(), "{error:?}");
            let until = std::time::Instant::now()
                + Duration::from_millis(if expected == 0 { 250 } else { 3000 });
            while std::time::Instant::now() < until
                && (expected == 0 || requests.load(Ordering::SeqCst) == 0)
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(requests.load(Ordering::SeqCst), expected, "{label}");
            drop(presenter);
        }
    });
    stopped.store(true, Ordering::SeqCst);
    worker.join().unwrap();
    let _ = std::fs::remove_dir_all(dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[derive(Default)]
struct DeferredRust {
    replacement: bool,
    active: bool,
    fails: bool,
}

static RUST_ACTIVATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static RUST_VALIDATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl DataSource for DeferredRust {
    fn app_id(&self) -> &str {
        "com.exact.fixture"
    }
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn ready(&self) -> bool {
        !self.replacement || self.active
    }
    fn replacement(&self, _: &[u8], _: &str, bytes: Vec<u8>) -> Result<Self, DataError> {
        Ok(Self {
            replacement: true,
            active: false,
            fails: bytes == b"fails",
        })
    }
    fn activate(&mut self) -> Result<(), DataError> {
        if self.replacement {
            assert!(!self.active);
            RUST_ACTIVATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        self.active = true;
        Ok(())
    }
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        assert!(self.replacement && !self.active);
        RUST_VALIDATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fails {
            return Err(DataError::Unavailable("candidate refused".into()));
        }
        self.active = true;
        Ok(())
    }
}

#[test]
fn signed_rust_pair_is_deferred_and_failed_live_replacement_keeps_the_old_generation() {
    use std::sync::atomic::Ordering::SeqCst;
    RUST_ACTIVATIONS.store(0, SeqCst);
    RUST_VALIDATIONS.store(0, SeqCst);
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let candidate = contract::compile("component App\n  view\n    text \"candidate\"\n")
        .unwrap()
        .encode();
    let receipt = br#"{"module":{"file":"app.module.wasm"}}"#.to_vec();
    let dir = std::env::temp_dir().join(format!("exact-linux-rust-pair-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(
        &dir,
        &baked,
        &candidate,
        &[
            ("rust/app.module.json", receipt.clone()),
            ("rust/app.module.wasm", b"works".to_vec()),
        ],
    );
    let mut config = selected_config(&dir, &baked, client);
    let (mut presenter, error) =
        boot_presenter::<DeferredRust>(&mut config, (390.0, 844.0)).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(presenter
        .host()
        .agent("{\"op\":\"tree\"}")
        .contains("candidate"));
    assert_eq!(
        RUST_ACTIVATIONS.load(SeqCst),
        0,
        "boot only verifies and defers the module"
    );
    let reply = exact_linux::agent::handle(
        &mut presenter,
        &serde_json::json!({"op":"screenshot", "path":dir.join("first.png")}).to_string(),
    );
    assert!(!reply.contains("error"), "{reply}");
    assert_eq!(RUST_ACTIVATIONS.load(SeqCst), 1);
    let tree = presenter.host().agent("{\"op\":\"tree\"}");
    let refused = exact_linux::delivery::Module {
        receipt: String::from_utf8(receipt).unwrap(),
        bytes: b"fails".to_vec(),
    };
    assert!(presenter
        .reload_module(&baked, DeferredRust::default(), Some(refused))
        .is_err());
    assert_eq!(RUST_VALIDATIONS.load(SeqCst), 1);
    assert_eq!(RUST_ACTIVATIONS.load(SeqCst), 1);
    assert_eq!(presenter.host().agent("{\"op\":\"tree\"}"), tree);
    // A plan-only reload keeps the admitted pair and recreates its deferred source.
    presenter
        .reload_module(&candidate, DeferredRust::default(), None)
        .unwrap();
    assert_eq!(RUST_VALIDATIONS.load(SeqCst), 2);
    assert_eq!(RUST_ACTIVATIONS.load(SeqCst), 1);
    let reply = exact_linux::agent::handle(
        &mut presenter,
        &serde_json::json!({"op":"screenshot", "path":dir.join("second.png")}).to_string(),
    );
    assert!(!reply.contains("error"), "{reply}");
    assert_eq!(RUST_ACTIVATIONS.load(SeqCst), 2);
    drop(presenter);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupt_or_disabled_signed_rust_pair_falls_back_before_booting_its_plan() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let candidate = contract::compile("component App\n  view\n    text \"candidate\"\n")
        .unwrap()
        .encode();
    for corrupt in [false, true] {
        let dir = std::env::temp_dir().join(format!(
            "exact-linux-rust-refused-{corrupt}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let client = stage(
            &dir,
            &baked,
            &candidate,
            &[
                (
                    "rust/app.module.json",
                    br#"{"module":{"file":"app.module.wasm"}}"#.to_vec(),
                ),
                ("rust/app.module.wasm", b"works".to_vec()),
            ],
        );
        if corrupt {
            let path = client
                .selection()
                .assets_dir
                .unwrap()
                .join("rust/app.module.wasm");
            std::fs::write(path, b"corrupt").unwrap();
        }
        let mut config = selected_config(&dir, &baked, client);
        let (presenter, _) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
        assert!(presenter
            .host()
            .agent("{\"op\":\"tree\"}")
            .contains("baked"));
        assert!(!presenter
            .host()
            .agent("{\"op\":\"tree\"}")
            .contains("candidate"));
        assert!(config.entry.is_none());
        drop(presenter);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn failed_post_pixel_rust_activation_does_not_bless_the_selected_bundle() {
    #[derive(Default)]
    struct CannotActivate;
    impl DataSource for CannotActivate {
        fn app_id(&self) -> &str {
            "com.exact.fixture"
        }
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
        fn ready(&self) -> bool {
            false
        }
        fn replacement(&self, _: &[u8], _: &str, _: Vec<u8>) -> Result<Self, DataError> {
            Ok(Self)
        }
        fn activate(&mut self) -> Result<(), DataError> {
            Err(DataError::Unavailable("malformed module".into()))
        }
    }
    let plan = contract::compile("component App\n  view\n    text \"candidate\"\n")
        .unwrap()
        .encode();
    let dir = std::env::temp_dir().join(format!(
        "exact-linux-rust-activation-failure-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(
        &dir,
        &plan,
        &plan,
        &[
            (
                "rust/app.module.json",
                br#"{"module":{"file":"app.module.wasm"}}"#.to_vec(),
            ),
            ("rust/app.module.wasm", b"malformed".to_vec()),
        ],
    );
    let record = client.dir().join("record.json");
    let mut config = selected_config(&dir, &plan, client);
    let (mut presenter, _) = boot_presenter::<CannotActivate>(&mut config, (390.0, 844.0)).unwrap();
    let _ = presenter.frame();
    presenter.first_pixel();
    let _ = presenter.frame();
    presenter.first_pixel();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(record).unwrap()).unwrap();
    assert_eq!(
        saved["failures"], 1,
        "first pixel cannot bless a module that failed activation"
    );
    assert!(saved["lastGood"].is_null());
    drop(presenter);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_live_rust_swap_refuses_before_loading_while_a_request_is_pending() {
    use exact_runner::{Answer, Store};
    #[derive(Clone)]
    struct Held(Arc<std::sync::atomic::AtomicUsize>);
    impl DataSource for Held {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
        fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            Ok(Answer::Later(exact_runner::Request::post_json(
                "https://pending.invalid",
                "{}",
            )))
        }
        fn replacement(&self, _: &[u8], _: &str, _: Vec<u8>) -> Result<Self, DataError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.clone())
        }
    }
    let plan = contract::compile("shape Reply\n  value: string\ncomponent App\n  mutation reply as shape Reply\n  action start\n    send reply = write()\n  view\n    button press=start testId=\"start\"\n      text \"send\"\n").unwrap().encode();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let source = Held(calls.clone());
    let (mut presenter, _) = exact_linux::Presenter::boot(
        &plan,
        source.clone(),
        (390.0, 844.0),
        1.0,
        std::env::temp_dir(),
    )
    .unwrap();
    let kernel = presenter.host().kernel();
    let button = kernel
        .node_by_key(kernel.find_by_test_id("start")[0])
        .unwrap()
        .id;
    presenter.tap(button).unwrap();
    let pending = presenter.host().runner().pending();
    assert!(!pending.is_empty());
    let module = exact_linux::delivery::Module {
        receipt: String::new(),
        bytes: Vec::new(),
    };
    let error = presenter
        .reload_module(&plan, source, Some(module))
        .unwrap_err();
    assert!(error.to_string().contains("retry after they settle"));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(presenter.host().runner().pending(), pending);
}
