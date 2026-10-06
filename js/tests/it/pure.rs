//! Actual Hermes versus Chrome's standard web utilities.
//! @ref LLP 1027.001 D1 — Unicode and URL semantics survive a language move
#![cfg(exact_js_engine)]
use exact_js::Module;
use exact_runner::DataSource;
use std::process::Command;

#[test]
fn pure_utilities_match_the_web_executor() {
    let chrome = std::env::var("CHROME")
        .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    if !std::path::Path::new(&chrome).exists() {
        eprintln!("pure utility browser sweep unavailable: set CHROME");
        return;
    }
    let script = concat!(env!("OUT_DIR"), "/pure.js");
    let reference = Command::new("bun")
        .args(["--input-type=module", "-e", CHROME_UTILITIES])
        .env("EXACT_PURE_SCRIPT", script)
        .env("CHROME", chrome)
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".."))
        .output()
        .expect("Bun launches the Chrome utility oracle; set CHROME to its executable");
    assert!(
        reference.status.success(),
        "{}",
        String::from_utf8_lossy(&reference.stderr)
    );
    let expected: Vec<String> = serde_json::from_slice(&reference.stdout).unwrap();
    let plan = contract::compile("component App\n  resource text = text() as shape string\n  resource url = url() as shape string\n  resource base64 = base64() as shape string\n  resource standard = standard() as shape string\n  resource microtask = microtask() as shape string\n  resource abort = abort() as shape string\n  resource intl = intl() as shape string\n  view\n    text text\n").unwrap();
    let mut module = Module::loaded(
        include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
        "test.pure",
        "",
    )
    .unwrap();
    module.bind(&plan);
    // A formatter per locale and option set is slow to make on Apple's Intl.
    module.set_budget_ms(f64::INFINITY);
    for (source, expected) in [
        "text",
        "url",
        "base64",
        "standard",
        "microtask",
        "abort",
        "intl",
    ]
    .into_iter()
    .zip(expected)
    {
        assert_eq!(
            module.query(source, &[]).unwrap().as_str().unwrap(),
            expected,
            "{source}"
        );
    }
}

const CHROME_UTILITIES: &str = r#"
import { Cdp } from './scripts/agent.mjs';
import { spawn } from 'node:child_process';
import { existsSync, readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
if (!existsSync(process.env.CHROME)) throw new Error('Chrome utility oracle unavailable: set CHROME to its executable');
const profile = mkdtempSync(join(tmpdir(), 'exact-pure-browser-'));
const child = spawn(process.env.CHROME, [
  '--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run',
  '--disable-background-networking', `--user-data-dir=${profile}`, 'about:blank',
], { detached:true, stdio:['ignore','ignore','ignore','pipe','pipe'] });
const cdp = new Cdp(child.stdio[3], child.stdio[4]);
const exited = new Promise(resolve => {
  child.on('error', error => { cdp.fail(String(error)); resolve(); });
  child.on('exit', () => { cdp.fail('Chrome closed'); resolve(); });
});
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const { sessionId } = await cdp.send('Target.attachToTarget', {
    targetId:targetInfos.find(target => target.type === 'page').targetId, flatten:true,
  });
  const fixture = readFileSync(process.env.EXACT_PURE_SCRIPT, 'utf8');
  const result = await cdp.send('Runtime.evaluate', {
    expression:fixture + '\n;Promise.all(["text","url","base64","standard","microtask","abort","intl"].map(source=>globalThis.exact.answer(source))).then(JSON.stringify)',
    returnByValue:true, awaitPromise:true,
  }, sessionId);
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  console.log(result.result.value);
} finally {
  if (child.pid && child.exitCode === null && child.signalCode === null) {
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
  }
  await exited;
  rmSync(profile, { recursive:true, force:true, maxRetries:3, retryDelay:100 });
}
"#;

#[test]
fn large_native_strings_preserve_json_semantics_and_call_ownership() {
    use exact_js::{from_json, to_json, Shape};
    use exact_plan::Value;
    let script = concat!(env!("OUT_DIR"), "/pure.js");
    let declarations = "shape Leaf\n  text: string\nshape Pair\n  first: string\n  second: string\nshape Branch\n  left: Leaf\n  right: Leaf\nshape Branches\n  first: Branch\n  second: Leaf\n  last: list<Branch>\n  reads: number\nshape Object\n  first: string\n  second: string\n  aliases: list<Leaf>\n  reads: number\n";
    let mut module = Module::loaded(
        include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
        "test.pure",
        "",
    )
    .unwrap();
    for (mode, ty) in [
        ("plain", "string"),
        ("boxed", "string"),
        ("object", "Object"),
        ("branches", "Branches"),
        ("nullable", "list<option<number>>"),
        ("extra", "Pair"),
        ("missing", "Pair"),
        ("throw", "Pair"),
        ("cycle", "Pair"),
        ("bigint", "Pair"),
        ("lone", "string"),
        ("arrayMethodsHook", "Pair"),
        ("arrayHook", "string"),
        ("private", "string"),
        ("small", "string"),
    ] {
        let plan = contract::compile(&format!(
            "{declarations}\ncomponent App\n  resource value = transfer(\"{mode}\") as shape {ty}\n  view\n    text \"fixture\"\n"
        ))
        .unwrap();
        let shape = Shape::from_plan(&plan, plan.sources[0].ty).unwrap();
        let reference = Command::new("bun")
            .args(["-e", &format!("require({}); process.stdout.write(JSON.stringify(globalThis.exact.answer('transfer',[{}])))", serde_json::to_string(script).unwrap(), serde_json::to_string(mode).unwrap())])
            .output()
            .unwrap();
        let expected = if reference.status.success() {
            serde_json::from_slice(&reference.stdout)
                .map_err(|error| error.to_string())
                .and_then(|json| from_json(&json, &shape))
        } else {
            Err("the standard serializer refused".into())
        };
        module.bind(&plan);
        let actual = module.query("transfer", &[Value::str(mode)]);
        if matches!(mode, "arrayMethodsHook" | "arrayHook") {
            assert!(
                actual.is_err(),
                "{mode}: universal hardening must refuse application mutation of Array intrinsics"
            );
            continue;
        }
        match expected {
            Ok(value) => assert_eq!(
                to_json(
                    &actual.unwrap_or_else(|error| panic!("{mode}: {error:?}")),
                    &shape
                )
                .unwrap(),
                to_json(&value, &shape).unwrap(),
                "{mode}"
            ),
            Err(_) => assert!(actual.is_err(), "{mode} must refuse"),
        }
    }
}

#[test]
fn universal_hardening_prevents_application_envelope_parser_replacement() {
    use exact_plan::Value;
    let plan = contract::compile("component App\n  resource reply = wire(\"\", \"sync\") as shape string\n  resource later = transfer(\"small\") as shape string\n  view\n    text \"fixture\"\n").unwrap();
    let big = "a\\\0é😀\u{2028}\u{2029}".repeat(8192);
    for (wire, mode, _previously_injectable) in [
        (r#"{"tag":0,"value":7,"value":"last"}"#, "sync", Ok("last")),
        (r#"{"value":"first","tag":2,"tag":0}"#, "sync", Ok("first")),
        (
            r#"{"tag":0,"value":"first","value":7}"#,
            "sync",
            Err("expected a string"),
        ),
        (
            r#"{"tag":0,"value":7,"extra":1e400}"#,
            "sync",
            Err("number out of range"),
        ),
        (
            r#"{"tag":0,"value":"\ud800"}"#,
            "sync",
            Err("answered something other than JSON"),
        ),
        (
            r#"{"tag":2,"value":7,"kind":"BadArguments","message":"wire refusal"}"#,
            "sync",
            Err("BadArguments(\"wire refusal\")"),
        ),
        (
            r#"{"tag":2,"value":1e400,"message":"wire refusal"}"#,
            "sync",
            Err("number out of range"),
        ),
        (r#"{"tag":3}"#, "async", Err("a call with no id")),
        (r#"{"value":7,"call":1,"tag":3}"#, "async", Ok("settled")),
        (
            r#"{"tag":3,"call":1,"value":1e400}"#,
            "async",
            Err("number out of range"),
        ),
        (r#"{"tag":1,"call":1}"#, "sync", Err("pending on no ticket")),
        (
            r#"{"tag":1,"call":1,"ticket":123}"#,
            "sync",
            Err("awaits a fetch it never made"),
        ),
        (
            r#"{"value":"missing tag"}"#,
            "sync",
            Err("answered with no tag"),
        ),
        ("[]", "sync", Err("answered with no tag")),
        ("null", "sync", Err("answered with no tag")),
        (
            r#"{"tag":0,"value":"valid"} false"#,
            "sync",
            Err("trailing characters"),
        ),
        (
            r#"{"tag":0,"value":""} false"#,
            "capture",
            Err("trailing characters"),
        ),
    ] {
        let mut module = Module::loaded(
            include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
            "test.pure",
            "",
        )
        .unwrap();
        module.set_budget_ms(f64::INFINITY);
        module.bind(&plan);
        let answer = module.query("wire", &[Value::str(wire), Value::str(mode)]);
        let value = answer.unwrap();
        let expected = if mode.contains("capture") {
            big.as_str()
        } else {
            "settled"
        };
        assert_eq!(value.as_str(), Some(expected), "{wire}");
        // A refused or captured reply cannot contaminate the next answer.
        assert_eq!(
            module
                .query("transfer", &[Value::str("small")])
                .unwrap()
                .as_str(),
            Some("a later call")
        );
    }
    for (wire, mode) in [
        (r#"{"tag":0,"value":""}"#, "capture"),
        (r#"{"tag":3,"call":1}"#, "async-capture"),
    ] {
        let mut module = Module::loaded(
            include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
            "test.pure",
            "",
        )
        .unwrap();
        module.bind(&plan);
        assert_eq!(
            module
                .query("wire", &[Value::str(wire), Value::str(mode)])
                .unwrap()
                .as_str(),
            Some(big.as_str())
        );
        module.set_budget_ms(0.0);
        assert!(format!(
            "{:?}",
            module
                .query("transfer", &[Value::str("plain")])
                .unwrap_err()
        )
        .contains("over the 0 ms budget"));
        assert_eq!(module.overruns(), 1);
        module.set_budget_ms(f64::INFINITY);
        assert_eq!(
            module
                .query("transfer", &[Value::str("small")])
                .unwrap()
                .as_str(),
            Some("a later call")
        );
    }
}
