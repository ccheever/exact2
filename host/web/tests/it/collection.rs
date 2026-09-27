//! The real Contract -> runner -> web batch/ABI path; no synchronous fake list.
use exact_runner::{CollectionFeedback, CollectionSnapshot, RowMeasurement};
use exact_web::{abi::Bridge, Host};

#[derive(Default)]
struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        args: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        if source == "rows" {
            Ok(exact_runner::Value::list(
                (0..args
                    .first()
                    .and_then(exact_runner::Value::as_number)
                    .unwrap_or(1000.) as usize)
                    .map(|i| {
                        exact_runner::Value::record(vec![exact_runner::Value::Number(i as f64)])
                    })
                    .collect(),
            ))
        } else {
            Err(exact_runner::DataError::UnknownSource(source.into()))
        }
    }
}

fn plan() -> Vec<u8> {
    let source = r#"shape Row
  index: number
component App
  resource rows = rows() as shape list<Row>
  state shown = true
  state rowHeight = 32
  state typed = ""
  action edit(value) writes typed
    typed = value
  action grow writes rowHeight
    rowHeight = 64
  action hide writes shown
    shown = not shown
  view
    column
      button press=hide testId="hide"
        text "Hide"
      button press=grow testId="grow"
        text "Grow"
      text typed testId="echo"
      when shown
        list virtualized=true height=180 width=320 testId="list"
          each x in rows key=x.index
            column height=rowHeight
              input value=typed change=edit testId=`row-${x.index}` height=16
              text `${x.index}` height=16
"#;
    contract::compile(source).unwrap().encode()
}
fn facts(snapshot: &CollectionSnapshot) -> CollectionFeedback {
    CollectionFeedback {
        view: snapshot.view,
        revision: snapshot.revision,
        scroll_sequence: 1,
        offset: 3_000.0,
        port_cross: 320.0,
        port_main: 180.0,
        cross: 305.0,
        focus_view: None,
        interaction_view: None,
        measurements: snapshot
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: 32.0,
            })
            .collect(),
    }
}
#[test]
fn geometry_commits_bounded_rows_and_metadata_after_extent_then_clears_on_unmount() {
    exact_web::link(exact_web_capabilities::ALL);
    let (mut host, first) = Host::boot(&plan(), NoData, Default::default(), "/").unwrap();
    let before = host.runner().collections().remove(0);
    assert_eq!(before.count, 1_000);
    assert!(before.rows.len() <= 16);
    assert!(
        first.rfind("\"op\":\"collections\"").unwrap() > first.find("\"op\":\"children\"").unwrap()
    );
    let batch = host.collection_feedback(&facts(&before).encode().unwrap());
    assert!(batch.contains("\"error\":null"), "{batch}");
    let after = host.runner().collections().remove(0);
    assert!(after.rows.len() <= 24, "{} rows", after.rows.len());
    assert!(after.rows.iter().any(|row| row.index > 80));
    assert!(
        batch.rfind("\"op\":\"collections\"").unwrap() > batch.find("\"op\":\"children\"").unwrap()
    );
    let kernel = host.runner().kernel();
    let hide = kernel
        .node_by_key(kernel.find_by_test_id("hide")[0])
        .unwrap()
        .id;
    let batch = host.dispatch_at(hide, exact_runner::Event::Press, 0.0);
    assert!(
        batch.contains("{\"op\":\"collections\",\"items\":[]}"),
        "{batch}"
    );
    assert!(host.runner().collections().is_empty());
}
#[test]
fn stale_and_malformed_feedback_do_not_emit_mutations_or_advance_the_clock() {
    exact_web::link(exact_web_capabilities::ALL);
    let (mut host, _) = Host::boot(&plan(), NoData, Default::default(), "/").unwrap();
    let bytes = facts(&host.runner().collections()[0]).encode().unwrap();
    host.collection_feedback(&bytes);
    let before = host.runner().collections();
    let stale = host.collection_feedback(&bytes);
    assert!(stale.contains("\"ops\":[]"), "{stale}");
    let bad = host.collection_feedback(&bytes[..bytes.len() - 1]);
    assert!(bad.contains("malformed collection feedback"), "{bad}");
    assert!(!bad.contains("\"accepted\":true"), "{bad}");
    assert!(bad.contains("\"ops\":[]"), "{bad}");
    assert_eq!(host.runner().collections(), before);
    assert_eq!(host.runner().now_ms(), 0.0);
}
#[test]
fn bridge_rejects_oversized_input_and_uses_the_common_le_decoder() {
    let plan = plan();
    exact_web::link(exact_web_capabilities::ALL);
    let (host, _) = Host::boot(&plan, NoData, Default::default(), "/").unwrap();
    let mut bridge = Bridge::new();
    exact_web::link(exact_web_capabilities::ALL);
    bridge.boot(&plan, NoData, 390.0, 844.0, "/");
    let bytes = facts(&host.runner().collections()[0]).encode().unwrap();
    bridge.advance(25.0, false);
    bridge.input_write(&bytes);
    let len = bridge.collection_feedback(bytes.len() + 1);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize))
        .contains("collection input length"));
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("\"clock\":25"));
    let len = bridge.collection_feedback(bytes.len());
    let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(batch.contains("\"op\":\"collections\""), "{batch}");
    assert!(batch.contains("\"error\":null"), "{batch}");
}

#[test]
fn edge_action_receipts_and_refusal_keep_committed_feedback_in_the_host_batch() {
    struct RefusingData;
    impl exact_runner::DataSource for RefusingData {
        fn query(
            &mut self,
            source: &str,
            args: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            if args == [exact_runner::Value::Bool(true)] {
                return Err(exact_runner::DataError::BadArguments("edge refused".into()));
            }
            NoData.query(source, args)
        }
    }
    for refuse in [false, true] {
        let source = format!(
            r#"shape Row
  index: number
component App
  state refused = false
  state reached = false
  resource rows = rows(refused) as shape list<Row>
  action end writes refused, reached
    refused = {refuse}
    reached = true
  view
    column
      text (reached ? "edge committed" : "before") testId="status"
      list virtualized=true height=180 width=320 reachend=end
        each x in rows key=x.index
          text `${{x.index}}` height=32
"#
        );
        exact_web::link(exact_web_capabilities::ALL);
        let (mut host, boot) = Host::boot(
            &contract::compile(&source).unwrap().encode(),
            RefusingData,
            Default::default(),
            "/",
        )
        .unwrap();
        assert!(
            !boot.contains("reachend"),
            "runner edge must not attach a listener"
        );
        let before = host.runner().collections().remove(0);
        let mut feedback = facts(&before);
        feedback.offset = before.total_extent - 180.;
        let batch = host.collection_feedback(&feedback.encode().unwrap());
        let after = host.runner().collections().remove(0);
        assert_eq!(after.rows.last().unwrap().index, 999);
        for row in &after.rows {
            for id in [row.view, row.root] {
                assert!(
                    batch.contains(&format!("\"op\":\"create\",\"id\":{id},")),
                    "feedback create missing after edge action: {batch}"
                );
            }
        }
        assert!(batch.contains("\"op\":\"collections\""));
        assert!(batch.contains("\"accepted\":true"), "{batch}");
        assert_eq!(batch.contains("edge refused"), refuse);
        assert_eq!(
            host.runner().slot("reached"),
            Some(&exact_runner::Value::Bool(!refuse))
        );
        assert!(!host.runner().is_poisoned());
        if !refuse {
            assert!(batch.contains("edge committed"));
        }
        let mut repeat = facts(&after);
        repeat.scroll_sequence = 2;
        repeat.offset = feedback.offset;
        let batch = host.collection_feedback(&repeat.encode().unwrap());
        assert_eq!(batch.contains("edge refused"), refuse, "{batch}");
        assert!(batch.contains("\"accepted\":true"), "{batch}");
    }
}

#[test]
#[ignore = "build a pure Rust web dist with EXACT_WEB_LINK=all; set EXACT_COLLECTION_DIST and CHROME"]
fn real_browser_collection_feedback_and_navigation() {
    use std::{path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("exact-collection-browser-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Bake the constant data: the carrier wasm supplies only the host, never a
    // fixture-specific data implementation or an authored scroll event handler.
    let baked = contract::bake(exact_plan::Plan::decode(&plan()).unwrap(), NoData).unwrap();
    std::fs::write(dir.join("app.plan"), baked.encode()).unwrap();
    let output = Command::new("bun")
        .arg("host/web/tests/collection.mjs")
        .env("EXACT_COLLECTION_TEST", &dir)
        .current_dir(root)
        .output()
        .unwrap();
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "build a pure Rust web dist with EXACT_WEB_LINK=all; set EXACT_COLLECTION_DIST and CHROME"]
fn real_browser_bidirectional_edges_and_state_only_deferral_become_idle() {
    use std::{path::Path, process::Command};
    let source = r#"shape Row
  index: number
component App
  state first = 0
  state ended = 0
  resource rows = rows(2) as shape list<Row>
  action start writes first
    if first > 0
      first = 0
  action end writes ended, first
    ended = ended + 1
    first = 6
  view
    column
      text `${first}` testId="steps"
      text `${ended}` testId="ends"
      list virtualized=true height=180 width=320 reachstart=start reachend=end testId="list"
        each x in rows key=x.index + first * 2
          text `${x.index + first * 2}` height=1
"#;
    for (case, source) in [
        ("bidirectional", source.to_owned()),
        (
            "state",
            source
                .replace("if first > 0\n      first = 0", "first = 1")
                .replace("    first = 6\n", "")
                .replace("key=x.index + first * 2", "key=x.index")
                .replace("`${x.index + first * 2}`", "`${x.index}`"),
        ),
    ] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir =
            std::env::temp_dir().join(format!("exact-collection-edges-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let baked = contract::bake(contract::compile(&source).unwrap(), NoData).unwrap();
        std::fs::write(dir.join("app.plan"), baked.encode()).unwrap();
        let output = Command::new("bun")
            .arg("host/web/tests/collection.mjs")
            .env("EXACT_COLLECTION_TEST", &dir)
            .env("EXACT_COLLECTION_EDGES", case)
            .current_dir(root)
            .output()
            .unwrap();
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        std::fs::remove_dir_all(dir).unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "build a pure Rust web dist with EXACT_WEB_LINK=all; set EXACT_COLLECTION_DIST and CHROME"]
fn real_browser_authored_jump_builds_its_rows_before_it_moves() {
    use std::{path::Path, process::Command};
    let source = r#"shape Row
  index: number
component App
  resource rows = rows() as shape list<Row>
  state target = 0
  action go writes target
    target = 20000
  view
    column
      button press=go testId="go"
        text "Go"
      list virtualized=true height=180 width=320 scrollTop=target testId="list"
        each x in rows key=x.index
          text `${x.index}` height=32
"#;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("exact-collection-jump-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let baked = contract::bake(contract::compile(source).unwrap(), NoData).unwrap();
    std::fs::write(dir.join("app.plan"), baked.encode()).unwrap();
    let output = Command::new("bun")
        .arg("host/web/tests/collection.mjs")
        .env("EXACT_COLLECTION_TEST", &dir)
        .env("EXACT_COLLECTION_JUMP", "1")
        .current_dir(root)
        .output()
        .unwrap();
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
