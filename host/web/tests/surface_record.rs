use exact_runner::{DataError, DataSource, Value};
use exact_web::abi::Bridge;
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("runner fact reached {source}")
    }
}
#[test]
fn surface_record_abi_distinguishes_an_invalid_empty_record_from_disposal() {
    let plan = contract::compile("shape Hud\n  beacons: number\ncomponent App\n  resource hud = exactSurface(\"world\") as shape Hud\n  view\n    text `${hud.beacons}`\n").unwrap();
    let mut bridge = Bridge::new();
    bridge.boot(&plan.encode(), NoData, 390., 844., "/");
    let mut publish = |text: &[u8]| {
        let n = bridge.input_write(text);
        let n = bridge.surface_record(n);
        String::from_utf8_lossy(bridge.output_bytes(n as usize)).into_owned()
    };
    assert!(publish(b"world\0{\"beacons\":2}").contains("\"error\":null"));
    assert!(publish(b"world\0{\"beacons\":9,\"extra\":\"\xff\"}").contains("UTF-8"));
    assert!(publish(b"wor\xffld").contains("UTF-8"));
    assert!(publish(b"world\0{\"beacons\":2}").contains("\"ops\":[]"));
    let refused = publish(b"world\0");
    assert!(
        refused.contains("hud") && refused.contains("expected a value"),
        "{refused}"
    );
    assert!(publish(b"world").contains("\"error\":null"));
    let n = bridge.input_write(br#"{"op":"state"}"#);
    let n = bridge.agent(n);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize))
        .contains("\"hud\":{\"beacons\":0}"));
}

mod exported {
    use super::NoData;
    const PLAN: &[u8] = &[];
    const COMPAT: &str = "{}";
    fn app_data() -> NoData {
        NoData
    }
    exact_web::host!(NoData, PLAN, COMPAT, app_data);

    #[test]
    fn nested_surface_export_is_refused_without_a_refcell_panic() {
        EXACT_BRIDGE.with(|cell| {
            let _busy = cell.borrow_mut();
            assert_eq!(exact_surface_record(0), 0);
        });
    }
}

/// Production JS staging/boot glue talks synchronously to a real compiled
/// Contract candidate through this bridge. GPU/DOM are deterministic doubles;
/// shape validation, resource recommits, bindings, state and clock are real.
#[test]
fn candidate_publications_validate_before_the_presenter_commits() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    let source = r#"shape Hud
  count: number
shape OtherHud
  count: number
component App
  state n = 7
  resource a = exactSurface("world") as shape Hud
  resource b = exactSurface("other") as shape OtherHud
  action inc writes n
    n = n + 1
  view
    column
      button press=inc testId="inc"
        text `${n}`
      canvas surface=world(9) testId="world"
      canvas surface=other(9) testId="other"
      text `old ${a.count}/${b.count}` testId="hud"
"#;
    let good = source
        .replace("world(9)", "world(b.count)")
        .replace("other(9)", "other(a.count + b.count)")
        .replace("`old ", "`new ");
    let wrong = source.replace(
        "shape OtherHud\n  count: number",
        "shape OtherHud\n  count: string",
    );
    let plans: Vec<_> = [source, &good, &wrong]
        .iter()
        .map(|s| contract::compile(s).unwrap().encode())
        .collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("bun")
        .args([
            "test",
            "host/web/tests/reload.test.mjs",
            "--test-name-pattern",
            "actual candidate host",
        ])
        .current_dir(root)
        .env("EXACT_TEST_CANDIDATE_HOST", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = BufReader::new(child.stdout.take().unwrap());
    let mut bridge = Bridge::new();
    for line in output.lines() {
        let line = line.unwrap();
        let Some(line) = line.strip_prefix("@exact ") else {
            continue;
        };
        let (op, payload) = line.split_once(' ').unwrap();
        let n = bridge.input_write(payload.as_bytes());
        let len = match op {
            "initial" => bridge.boot(&plans[0], NoData, 390., 844., "/"),
            "boot" => {
                let n = bridge.input_write(&plans[payload.parse::<usize>().unwrap()]);
                bridge.boot_plan(n, NoData, 390., 844., "/")
            }
            "begin" => {
                writeln!(input, "{}", bridge.begin_boot()).unwrap();
                input.flush().unwrap();
                continue;
            }
            "finish" => {
                bridge.finish_boot(payload == "1");
                writeln!(input, "null").unwrap();
                input.flush().unwrap();
                continue;
            }
            "record" => bridge.surface_record(n),
            "stage" => bridge.stage_surface_record(n),
            "agent" => bridge.agent(n),
            "advance" => bridge.advance(payload.parse().unwrap()),
            _ => panic!("unknown test request {op}"),
        };
        input.write_all(bridge.output_bytes(len as usize)).unwrap();
        writeln!(input).unwrap();
        input.flush().unwrap();
    }
    assert!(
        child.wait().unwrap().success(),
        "candidate host/glue regression failed"
    );
}
