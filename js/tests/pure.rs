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
    let plan = contract::compile("component App\n  resource text = text() as shape string\n  resource url = url() as shape string\n  resource base64 = base64() as shape string\n  view\n    text text\n").unwrap();
    let mut module = Module::loaded(
        include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
        "test.pure",
        "",
    )
    .unwrap();
    module.bind(&plan);
    for (source, expected) in ["text", "url", "base64"].into_iter().zip(expected) {
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
    expression:fixture + '\n;JSON.stringify(["text","url","base64"].map(source=>globalThis.exact.answer(source)))',
    returnByValue:true,
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
    let declarations = "shape Leaf\n  text: string\nshape Pair\n  first: string\n  second: string\nshape Object\n  first: string\n  second: string\n  aliases: list<Leaf>\n  reads: number\n";
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
        ("nullable", "list<option<number>>"),
        ("extra", "Pair"),
        ("missing", "Pair"),
        ("throw", "Pair"),
        ("cycle", "Pair"),
        ("bigint", "Pair"),
        ("lone", "string"),
        ("reverseHook", "Pair"),
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
