use super::*;
use exact_linux::app::{boot_presenter, Config};
use exact_runner::DataSource;
use exact_runner::{DataError, Value};
use exact_update::{sha256_hex, Client, Outcome};
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const UPDATE_COMPAT: &str = r#"{"id":"fixture00000000","inputs":{"app":"com.exact.fixture","keys":null,"trust":"development","store":{"L":"A"}},"delivery":{"channel":"prod","origin":"https://updates.example"}}"#;

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
    let mut client = Client::open(dir, dir, UPDATE_COMPAT, baked).unwrap();
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
        Client::open(dir, dir, UPDATE_COMPAT, baked).unwrap()
    }
}

fn selected_config(dir: &Path, baked: &[u8], client: Client) -> Config {
    let mut config = Config {
        plan: baked.to_vec(),
        fallback_plan: None,
        assets: dir.to_path_buf(),
        scale: 1.0,
        size: (390.0, 844.0),
        agent: false,
        smoke: false,
        shot: None,
        dev_plan: None,
        card: String::new(),
        vnc: None,
        compat: UPDATE_COMPAT.into(),
        explicit: false,
        entry: None,
        selected_assets: None,
        updates: None,
    };
    config.use_updates(Box::new(Updates::from_client(client).unwrap()), baked);
    config
}

#[test]
fn a_fetched_plan_refused_at_boot_falls_back_to_baked() {
    let source = "component App\n  view\n    text \"ok\"\n";
    let mut foreign = contract::compile(source).unwrap();
    foreign.app_id = "com.exact.foreign".into();
    let baked = contract::compile(source).unwrap().encode();
    let mut config = Config {
        plan: foreign.encode(),
        fallback_plan: Some(baked),
        assets: std::env::current_dir().unwrap(),
        scale: 1.0,
        size: (390.0, 844.0),
        agent: false,
        smoke: false,
        shot: None,
        dev_plan: None,
        card: String::new(),
        vnc: None,
        compat: r#"{"id":"fixture00000000","inputs":{"store":{"L":"0"}}}"#.into(),
        explicit: true,
        entry: None,
        selected_assets: None,
        updates: None,
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
    let mut client = Client::open(&dir, Path::new("."), compat, &baked).unwrap();
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
        plan: b"EXPL".to_vec(), // the compiler was interrupted mid-write
        fallback_plan: Some(baked.clone()),
        assets: PathBuf::from("."),
        scale: 1.0,
        size: (390.0, 844.0),
        agent: false,
        smoke: false,
        shot: None,
        dev_plan: Some(PathBuf::from("app.plan")),
        card: String::new(),
        vnc: None,
        compat: compat.into(),
        explicit: true,
        entry: None,
        selected_assets: None,
        updates: None,
    };
    config.use_updates(Box::new(updates), &baked);
    assert_eq!(config.plan, b"EXPL");
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
        contract::compile("component App\n  view\n    image \"assets/mark.png\" width=96\n")
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
        contract::compile("component App\n  view\n    image \"assets/mark.png\" width=96\n")
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
    let source = "component App\n  state divisor = 1\n  action breakIt writes divisor\n    divisor = 0\n  action repair writes divisor\n    divisor = 1\n  view\n    column width=100\n      text \"running\"\n      button press=breakIt testId=\"break\"\n        text \"break\"\n      button press=repair testId=\"repair\"\n        text \"repair\"\n";
    let baked = contract::compile(source).unwrap().encode();
    let candidate = contract::compile(
        &source
            .replace("width=100", "width=(100 / divisor)")
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
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_verified_but_undecodable_selection_is_demoted_after_failed_launches() {
    let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
        .unwrap()
        .encode();
    let dir = std::env::temp_dir().join(format!("exact-linux-bad-launch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let client = stage(&dir, &baked, b"EXPL", &[]);
    let record = client.dir().join("record.json");
    drop(client);
    for expected in 1..=2 {
        let client = Client::open(&dir, &dir, UPDATE_COMPAT, &baked).unwrap();
        let mut config = selected_config(&dir, &baked, client);
        assert!(config.entry.is_some());
        let (mut presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
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
    let client = Client::open(&dir, &dir, UPDATE_COMPAT, &baked).unwrap();
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
