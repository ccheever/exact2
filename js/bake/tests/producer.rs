//! Actual tsc → Rolldown → HBC → bake, then replacement in the native host.
use exact_apple::abi::{Bridge, Hooks};
use exact_js::{Module, Paired};
use exact_js_bake::{bake, Baked, Tools};
use exact_kernel::Kernel;
use exact_runner::{failure::FailureCode, DataError::Failed, DataSource, Runner, Store, Value};
use serde_json::Value as Json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const APP: &str = "test.exact.logic";
const CONTRACT: &str = r#"
component App
  state count = 0
  resource message = message(count) as shape string
  action increment
    count = count + 1
  view
    column
      text message testId="message"
      button "Increment" press=increment testId="increment"
"#;
const SOURCE: &str = r#"
import type { Sources, Answer } from './app.contract.d.ts';
import { prefix } from './logic';
export const appId = 'test.exact.logic';
export const grants = '';
const sources: Sources = {
  message: ([count]) => { console.log('message called'); return prefix + count; },
};
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "exact-producer-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.write("app.contract", CONTRACT);
        f.write("app.ts", SOURCE);
        f.write("logic.ts", "export const prefix = 'old: ';\n");
        f
    }
    fn write(&self, name: &str, bytes: &str) {
        std::fs::write(self.0.join(name), bytes).unwrap();
    }
    fn bake(&self) -> Baked {
        bake(&self.0, &Tools::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn paired(baked: &Baked) -> Paired {
    Paired::decode(&baked.receipt, &baked.plan, baked.bytecode.clone(), APP, "").unwrap()
}

#[test]
fn producer_bakes_the_bytecode_keeps_sources_untouched_and_refuses_bad_candidates() {
    let f = Fixture::new();
    if !exact_js::ENGINE_LINKED {
        assert!(bake(&f.0, &Tools::default())
            .err()
            .unwrap()
            .contains("requires the lean Hermes"));
        return;
    }
    let first = f.bake();
    assert!(
        first.source_map.is_none(),
        "standalone bakes carry no source map"
    );
    let repeat = f.bake();
    assert_eq!(first.plan, repeat.plan);
    assert_eq!(first.bytecode, repeat.bytecode);
    assert_eq!(first.script, repeat.script);
    assert_eq!(first.receipt, repeat.receipt);
    assert!(
        !f.0.join("app.contract.d.ts").exists(),
        "generated types stay in the snapshot"
    );
    assert_eq!(std::fs::read_to_string(f.0.join("app.ts")).unwrap(), SOURCE);
    let candidate = paired(&first);
    assert!(
        !candidate.module.is_loaded(),
        "admission and first frame need no engine"
    );
    let mut live = Runner::boot(
        candidate.plan,
        candidate.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(live.resource("message"), Some(&Value::str("old: 0")));
    assert!(
        !live.data().is_loaded(),
        "first frame comes entirely from baked values"
    );
    live.data().load().unwrap();
    live.data_ready().unwrap();
    live.act("increment", vec![]).unwrap();
    assert_eq!(live.resource("message"), Some(&Value::str("old: 1")));
    let out = f.0.join("dist");
    first.write_new(&out).unwrap();
    assert!(!out.join("app.plan.map.json").exists());
    served_pair(&out);
    assert!(
        first.write_new(&out).is_err(),
        "no overwriting a published generation"
    );
    f.write("logic.ts", "export const prefix = 'new: ';\n");
    let second = f.bake();
    let mut next = paired(&second);
    next.module.load().unwrap();
    let carried = live.carry();
    let mut changed = Runner::boot_carrying(
        next.plan,
        next.module,
        Kernel::with_monospace(),
        &carried,
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(changed.slot("count"), Some(&Value::Number(1.0)));
    assert_eq!(
        changed.resource("message"),
        Some(&Value::str("new: 1")),
        "same arguments must not reuse old logic's answer"
    );
    assert!(changed
        .journal()
        .any(|line| line.ends_with("message called")));
    assert!(changed.data().take_logs().is_empty());
    let mut same = paired(&second);
    same.module.load().unwrap();
    let mut reloaded = Runner::boot_carrying(
        same.plan,
        same.module,
        Kernel::with_monospace(),
        &changed.carry(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        reloaded.data().take_logs().is_empty(),
        "unchanged logic keeps matching resource answers"
    );

    for bad in [
        SOURCE.replace("return prefix + count", "return 42"),
        SOURCE.replace("return prefix + count", "return (42 as any)"),
        SOURCE.replace(
            "export const grants = ''",
            "export const grants = Date.now().toString()",
        ),
    ] {
        f.write("app.ts", &bad);
        assert!(
            bake(&f.0, &Tools::default()).is_err(),
            "types, runtime shapes, and ambient reads all gate publication"
        );
        assert_eq!(std::fs::read(out.join("app.plan")).unwrap(), first.plan);
        assert_eq!(
            std::fs::read_to_string(out.join("app.module.json")).unwrap(),
            first.receipt
        );
    }
    // A grant a device would refuse must not bake: a native host that
    // cannot parse one line holds none of the app's grants.
    f.write(
        "app.ts",
        &SOURCE.replace(
            "export const grants = ''",
            "export const grants = 'net.fetch https://a.example\\nsecret.keep jwtToken'",
        ),
    );
    let error = bake(&f.0, &Tools::default())
        .err()
        .expect("an unparseable grant must refuse the bake");
    assert!(
        error.contains("line 2") && error.contains("jwtToken"),
        "{error}"
    );
    assert_eq!(std::fs::read(out.join("app.plan")).unwrap(), first.plan);
    let outside = Fixture::new();
    f.write(
        "app.ts",
        &SOURCE.replace(
            "'./logic'",
            &serde_json::to_string(&outside.0.join("logic").to_string_lossy()).unwrap(),
        ),
    );
    let error = bake(&f.0, &Tools::default())
        .err()
        .expect("an absolute import outside the snapshot must refuse");
    assert!(error.contains("outside captured app"), "{error}");
    let mut corrupt = second.bytecode.clone();
    corrupt[20] ^= 1;
    assert!(Paired::decode(&second.receipt, &second.plan, corrupt, APP, "").is_err());
    assert!(Paired::decode(
        &second.receipt,
        &first.plan,
        second.bytecode.clone(),
        APP,
        ""
    )
    .is_err());
    assert!(Paired::decode(
        &second.receipt,
        &second.plan,
        second.bytecode.clone(),
        "test.other",
        ""
    )
    .is_err());
    assert!(Paired::decode(
        &second.receipt,
        &second.plan,
        second.bytecode.clone(),
        APP,
        "net.fetch https://x"
    )
    .is_err());
    let mut meta: Json = serde_json::from_str(&second.receipt).unwrap();
    let mut wrong_version = second.bytecode.clone();
    wrong_version[8] ^= 1;
    {
        use sha2::{Digest, Sha256};
        meta["module"]["sha256"] = format!("{:x}", Sha256::digest(&wrong_version)).into();
    }
    assert!(
        Paired::decode(&meta.to_string(), &second.plan, wrong_version, APP, "").is_err(),
        "the actual HBC header must match, even when the receipt claims compatibility"
    );
    meta["bytecodeVersion"] = 0.into();
    assert!(Paired::decode(
        &meta.to_string(),
        &second.plan,
        second.bytecode.clone(),
        APP,
        ""
    )
    .is_err());
}

fn served_pair(out: &Path) {
    // Exercise the actual retained HTTP namespace using real producer bytes.
    let probe = r#"
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {moduleCards,retainDevGeneration,readDevGeneration} from './host/web/serve.mjs';
import {sha256} from './scripts/origin.mjs';
const dir=process.argv[1], epoch='b'.repeat(32), prefix=`/__dev/generation/${epoch}/1/`;
const files=new Map(['app.plan','app.js','app.hbc','app.module.json'].map(name=>[name,readFileSync(resolve(dir,name))]));
const module=moduleCards(files,'test.exact.logic');
assert.deepEqual(Object.keys(module).sort(),['native','receipt','web']);
assert.throws(()=>moduleCards(files,'another.app'));
for(const name of files.keys()){
  const corrupt=new Map(files);corrupt.set(name,Buffer.from('corrupt'));
  assert.throws(()=>moduleCards(corrupt,'test.exact.logic'),name);
}
const plan=files.get('app.plan');
files.set('exact.json',Buffer.from(JSON.stringify({dev:{epoch,seq:1},plan:{bytes:plan.length,sha256:sha256(plan)},module})));
const cache=resolve(dir,'retained');retainDevGeneration(cache,epoch,1,files);
for(const [name,body] of files)assert.deepEqual(readDevGeneration(cache,prefix+name)?.body,body,name);
writeFileSync(resolve(cache,epoch,'1/app.js'),'corrupt');
assert.equal(readDevGeneration(cache,prefix+'app.js'),null);
assert.deepEqual(readDevGeneration(cache,prefix+'app.hbc')?.body,files.get('app.hbc'));
assert.equal(readDevGeneration(cache,prefix+'private.js'),null);
"#;
    let result = std::process::Command::new("bun")
        .args(["--input-type=module", "-e", probe])
        .arg(out)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn output(bridge: &Bridge<Module>, len: u32) -> Json {
    let value: Json = serde_json::from_slice(bridge.output_bytes(len as usize)).unwrap();
    value
}
fn ask(bridge: &mut Bridge<Module>, op: &str) -> Json {
    let len = bridge.input_write(format!("{{\"op\":\"{op}\"}}").as_bytes());
    let size = bridge.agent(len);
    output(bridge, size)
}
fn prepare(bridge: &mut Bridge<Module>, baked: &Baked) -> Result<(), String> {
    let mut payload = baked.plan.clone();
    payload.extend(baked.receipt.as_bytes());
    payload.extend(&baked.bytecode);
    bridge.input_write(&payload);
    let count = bridge.prepare_module(
        [baked.plan.len(), baked.receipt.len(), baked.bytecode.len()],
        Module::new(Vec::new(), APP, ""),
        Hooks::none(),
        390.0,
        844.0,
    );
    match output(bridge, count)["error"].as_str() {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

#[test]
fn native_host_sessions_prepare_together_and_keep_the_live_app_when_one_refuses() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    // This is a Bridge protocol test, not qualification of Apple's HOME/Library
    // defaults on another platform. A child owns its explicit storage-free drive.
    const CHILD: &str = "EXACT_PRODUCER_BRIDGE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native_host_sessions_prepare_together_and_keep_the_live_app_when_one_refuses",
            ])
            .env(CHILD, "1")
            .env("EXACT_AGENT", "1")
            .env_remove("EXACT_AGENT_STORAGE")
            .env_remove("EXACT_AGENT_STORAGE_FRESH")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let f = Fixture::new();
    let first = f.bake();
    let mut a = Bridge::<Module>::new();
    let mut b = Bridge::<Module>::new();
    for bridge in [&mut a, &mut b] {
        prepare(bridge, &first).unwrap();
        let count = bridge.commit_plan();
        let committed = output(bridge, count);
        assert!(committed["error"].is_null(), "{committed}");
        let count = bridge.data_ready();
        let activated = output(bridge, count);
        assert!(activated["error"].is_null(), "{activated}");
    }
    let tree = ask(&mut b, "tree");
    let button = tree["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["props"]["testId"] == "increment")
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    let count = b.dispatch(button as u32, 0, 0, 321.0);
    assert!(output(&b, count)["error"].is_null());
    let before_a = ask(&mut a, "state");
    let before_b = ask(&mut b, "state");
    let mut unpaired = f.bake();
    unpaired.receipt = "{}".into();
    assert!(prepare(&mut b, &unpaired)
        .unwrap_err()
        .contains("module generation"));
    assert_eq!(ask(&mut b, "state"), before_b);
    f.write("logic.ts", "export const prefix = 'new: ';\n");
    f.write(
        "app.ts",
        &SOURCE.replace(
            "return prefix + count",
            "if (count === 1) throw new Error('candidate refused'); return prefix + count",
        ),
    );
    let bad = f.bake(); // count=0 bakes; session b's carried count=1 refuses.
    prepare(&mut a, &bad).unwrap();
    assert!(prepare(&mut b, &bad).is_err());
    a.discard_plan();
    b.discard_plan();
    assert_eq!(ask(&mut a, "state"), before_a);
    assert_eq!(ask(&mut b, "state"), before_b);
    f.write("app.ts", SOURCE);
    let good = f.bake();
    prepare(&mut a, &good).unwrap();
    prepare(&mut b, &good).unwrap();
    assert_eq!(
        ask(&mut a, "state"),
        before_a,
        "prepare never publishes a candidate"
    );
    assert_eq!(ask(&mut b, "state"), before_b);
    a.commit_plan();
    b.commit_plan();
    assert_eq!(ask(&mut a, "state")["slots"]["count"], 0);
    assert_eq!(ask(&mut b, "state")["slots"]["count"], 1);
    assert!(ask(&mut a, "tree").to_string().contains("new: 0"));
    assert!(ask(&mut b, "tree").to_string().contains("new: 1"));
}

#[test]
fn cli_refuses_bad_arguments() {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_exact-js-bake"))
        .output()
        .unwrap();
    assert_eq!(status.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&status.stderr).contains("usage:"));
    assert!(Path::new(env!("CARGO_MANIFEST_DIR")).is_dir());
}

#[test]
fn storage_types_are_checked_by_the_actual_bake_without_granting_bake_io() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    let source = SOURCE.replace(
        "message: ([count]) => { console.log('message called'); return prefix + count; }",
        "message: async ([count], store, storage) => { try { await storage.fs.readFile(storage.fs.directories.data + '/note'); return 'unexpected storage'; } catch (error) { return prefix + count; } }",
    );
    f.write("app.ts", &source);
    let baked = f.bake();
    assert!(baked.declarations.contains(include_str!(
        "../../../vendor/ibex/crates/ibex2/src/bindings/storage.d.ts"
    )));
    let candidate = paired(&baked);
    let live = Runner::boot(
        candidate.plan,
        candidate.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(live.resource("message"), Some(&Value::str("old: 0")));
    f.write(
        "app.ts",
        &source.replace(
            "storage.fs.readFile(storage.fs.directories.data + '/note')",
            "storage.fs.writeFile(storage.fs.directories.data + '/note', 'not bytes')",
        ),
    );
    let error = bake(&f.0, &Tools::default())
        .err()
        .expect("wrong storage parameters must fail tsc");
    assert!(error.contains("error TS"), "{error}");
    // A source that lets the bake's refusal (`code: 'bake'`) through is the
    // device's to answer, as on the web build: no compiled value, the
    // placeholder until a launch asks it (kanban2 #5: the native build
    // stopped here).
    f.write(
        "app.ts",
        &SOURCE.replace(
            "message: ([count]) => { console.log('message called'); return prefix + count; }",
            "message: async ([count], store, storage) => { await storage.fs.readFile(storage.fs.directories.data + '/note'); return prefix + count; }",
        ),
    );
    let baked = f.bake();
    let plan = paired(&baked).plan;
    let row = &plan.resources[0];
    assert!(row.initial.len == 0 && row.reader);
}

#[test]
fn bake_defers_uncaught_storage_but_keeps_source_errors_fatal() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    let original =
        "message: ([count]) => { console.log('message called'); return prefix + count; }";
    let storage = "await storage.sqlite.open('app:/data/notes.db')";
    let source = SOURCE.replace(
        original,
        &format!(
            "message: async ([count], store, storage) => {{ {storage}; return prefix + count; }}"
        ),
    );
    f.write("app.ts", &source);
    let baked = f.bake();
    let candidate = paired(&baked);
    assert_eq!(candidate.plan.resources[0].initial.len, 0);
    let mut live = Runner::boot(
        candidate.plan,
        candidate.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(live.resource("message"), Some(&Value::str("")));
    assert!(
        live.take_requests().is_empty(),
        "bake deferral creates no fabricated request"
    );
    live.data().load().unwrap();
    // A runtime storage refusal remains a failure, `storage` (LLP 1109 D3):
    // the resource's, as on the web, not the commit's (LLP 1027.000 D3).
    let asked = live
        .data()
        .answer(&mut Store::new("", []), "message", &[Value::Number(0.)]);
    assert!(
        matches!(asked, Err(Failed(FailureCode::Storage, _))),
        "{asked:?}"
    );
    live.data_ready().expect("the commit stands");
    assert_eq!(live.resource("message"), Some(&Value::str("")));
    let failed = live.failed_resources();
    assert!(matches!(failed.as_slice(), [("message", why)] if why.contains("storage")));
    assert!(live
        .journal()
        .any(|l| l.contains("resource message failed: ") && l.contains("storage")));
    f.write(
        "app.contract",
        &CONTRACT.replace("as shape string", "as shape string else fallback()"),
    );
    f.write(
        "app.ts",
        &source.replace(
            "const sources: Sources = {",
            "const sources: Sources = { fallback: () => 'loading',",
        ),
    );
    let fallback = f.bake();
    let candidate = paired(&fallback);
    let live = Runner::boot(
        candidate.plan,
        candidate.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(live.resource("message"), Some(&Value::str("loading")));
    f.write("app.ts", &source.replace("const sources: Sources = {", &format!("const sources: Sources = {{ fallback: async (_, store, storage) => {{ {storage}; return 'loading'; }},")));
    let error = bake(&f.0, &Tools::default())
        .err()
        .expect("a storage-dependent placeholder cannot answer at bake");
    assert!(error.contains("placeholder answers later"), "{error}");
    f.write("app.contract", CONTRACT);
    for body in [
        "throw new Error('ordinary bug');".to_string(),
        format!("try {{ {storage}; }} catch (_) {{}} throw new Error('ordinary bug');"),
    ] {
        f.write(
            "app.ts",
            &SOURCE.replace(
                original,
                &format!("message: async ([count], store, storage) => {{ {body} }}"),
            ),
        );
        let error = bake(&f.0, &Tools::default())
            .err()
            .expect("ordinary source bugs must refuse baking");
        assert!(error.contains("ordinary bug"), "{error}");
    }
}

#[test]
fn resident_producer_rechecks_changed_deleted_and_added_sources_and_recovers() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    f.write("__exact_build.tsbuildinfo", "{}");
    assert!(producer
        .bake(&f.0, None)
        .err()
        .unwrap()
        .contains("reserved"));
    std::fs::remove_file(f.0.join("__exact_build.tsbuildinfo")).unwrap();
    let first = producer.bake(&f.0, None).unwrap();
    let standalone = f.bake();
    assert_eq!(first.script, standalone.script);
    assert_eq!(first.bytecode, standalone.bytecode);
    assert_eq!(first.plan, standalone.plan);
    assert_eq!(first.receipt, standalone.receipt);
    assert!(standalone.source_map.is_none());
    let map: Json = serde_json::from_str(first.source_map.as_ref().unwrap()).unwrap();
    assert_eq!(
        map["digest"],
        format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&first.plan))
    );
    assert_eq!(
        map["nodes"][0]["file"],
        f.0.canonicalize()
            .unwrap()
            .join("app.contract")
            .to_str()
            .unwrap()
    );
    let out = f.0.join("dist");
    first.write_new(&out).unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("app.plan.map.json")).unwrap(),
        *first.source_map.as_ref().unwrap()
    );
    assert_eq!(first.receipt, producer.bake(&f.0, None).unwrap().receipt);

    f.write("app.ts", &format!("import './app.js';\n{SOURCE}"));
    assert_eq!(
        producer.bake(&f.0, None).unwrap().receipt,
        f.bake().receipt,
        "previous generated JavaScript is not a captured source"
    );
    f.write("app.ts", SOURCE);

    f.write("logic.ts", "export const prefix = 'new: ';\n");
    assert_ne!(first.receipt, producer.bake(&f.0, None).unwrap().receipt);
    f.write("logic.ts", "export const prefix: string = 42;\n");
    let diagnostics = producer.bake(&f.0, None).err().unwrap();
    assert!(diagnostics.contains("TS2322"));
    assert!(
        !diagnostics.contains("lib.webworker.d.ts"),
        "the error is not the --listFiles inventory: {diagnostics}"
    );
    assert!(
        producer.bake(&f.0, None).is_err(),
        "unchanged invalid input is never cached as success"
    );
    std::fs::remove_file(f.0.join("logic.ts")).unwrap();
    assert!(
        producer.bake(&f.0, None).is_err(),
        "removed imports invalidate resolution"
    );
    f.write("logic.ts", "export { prefix } from './added';\n");
    assert!(producer.bake(&f.0, None).is_err());
    f.write("added.ts", "export const prefix = 'added: ';\n");
    assert!(
        producer.bake(&f.0, None).is_ok(),
        "new imports recover without a restart"
    );

    f.write(
        "app.contract",
        &CONTRACT.replace("as shape string", "as shape number"),
    );
    assert!(
        producer.bake(&f.0, None).is_err(),
        "generated declarations participate in checking"
    );
    f.write("app.contract", CONTRACT);
    assert!(producer.bake(&f.0, None).is_ok());

    f.write("logic.ts", "export const prefix = 'typed: ';\ndeclare global { interface Array<T> { length: string; } }\n");
    assert!(
        producer.bake(&f.0, None).is_err(),
        "global/library conflicts remain checked"
    );
    f.write("logic.ts", "export const prefix = 'final: ';\n");
    let final_bake = producer.bake(&f.0, None).unwrap();
    assert_eq!(final_bake.receipt, f.bake().receipt);

    // Capture must reconcile directories as well as bytes, including a
    // source-shaped directory changing into a source file and back again.
    std::fs::create_dir(f.0.join("shape.ts")).unwrap();
    f.write("shape.ts/inner.ts", "export const value = 1;");
    producer.bake(&f.0, None).unwrap();
    std::fs::remove_dir_all(f.0.join("shape.ts")).unwrap();
    f.write("shape.ts", "export const value = 2;");
    producer.bake(&f.0, None).unwrap();
    std::fs::remove_file(f.0.join("shape.ts")).unwrap();
    std::fs::create_dir(f.0.join("shape.ts")).unwrap();
    f.write("shape.ts/inner.ts", "export const value = 3;");
    producer.bake(&f.0, None).unwrap();

    let outside = Fixture::new();
    f.write(
        "logic.ts",
        &format!(
            "export {{ prefix }} from {:?};",
            outside.0.join("logic.ts").to_str().unwrap()
        ),
    );
    assert!(
        producer.bake(&f.0, None).is_err(),
        "absolute imports cannot escape the captured app"
    );
    outside.write("types.d.ts", "export interface External { value: string }");
    f.write(
        "logic.ts",
        &format!(
            "import type {{External}} from {:?}; export const prefix = 'final: ';",
            outside.0.join("types.d.ts").to_str().unwrap()
        ),
    );
    assert!(
        producer
            .bake(&f.0, None)
            .err()
            .unwrap()
            .contains("outside captured app"),
        "type-only imports cannot influence a captured app either"
    );
    f.write("logic.ts", "export const prefix = 'final: ';\n");
    assert_eq!(
        final_bake.receipt,
        producer.bake(&f.0, None).unwrap().receipt
    );
}

#[test]
fn resident_producer_honors_compiler_overrides() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    // The native test harness rejects tsc's --noEmit argument on every host.
    // This proves a real override ran and refused, without a Unix-only helper.
    let refusing = std::env::current_exe().unwrap();
    let tools = Tools {
        tsc: refusing.clone(),
        ..Tools::default()
    };
    let mut producer = exact_js_bake::Producer::new(tools).unwrap();
    assert!(producer
        .bake(&f.0, None)
        .err()
        .unwrap()
        .contains(&format!("{} refused", refusing.display())));
}

#[test]
fn resident_maps_name_original_imports_and_bake_refusals_after_capture() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    std::fs::create_dir(f.0.join("ui")).unwrap();
    f.write(
        "app.contract",
        &format!(
            "use Label from \"./ui/label.contract\"\n{}",
            CONTRACT.replace("text message testId=\"message\"", "Label(body=message)")
        ),
    );
    f.write("ui/label.contract", "component Label\n  props\n    body: string\n  state width = 80\n  view\n    button width=width height=20\n      text body testId=\"message\"\n");
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    let baked = producer.bake(&f.0, None).unwrap();
    assert_eq!(baked.plan, f.bake().plan);
    let map: Json = serde_json::from_str(baked.source_map.as_ref().unwrap()).unwrap();
    let source = f.0.canonicalize().unwrap();
    let nodes = map["nodes"].as_array().unwrap();
    let child = nodes
        .iter()
        .find(|node| node["component"] == "Label" && node["line"] == 7)
        .unwrap();
    assert_eq!(
        child["file"],
        source.join("ui/label.contract").to_str().unwrap()
    );
    assert_eq!(
        child["chain"][0]["file"],
        source.join("app.contract").to_str().unwrap()
    );
    assert!(map["slots"]
        .as_object()
        .unwrap()
        .values()
        .any(|slot| slot["component"] == "Label"
            && slot["file"] == source.join("ui/label.contract").to_str().unwrap()));
    f.write("ui/label.contract", "component Label\n  props\n    body: string\n  state width = 0\n  view\n    button width=width height=0\n      text body testId=\"message\"\n");
    let error = producer.bake(&f.0, None).err().unwrap();
    assert!(
        error.contains("bake-zero-size")
            && error.contains(&format!(
                "{}:6:",
                source.join("ui/label.contract").display()
            )),
        "{error}"
    );
    assert!(
        error.contains(&format!("{}:", source.join("app.contract").display())),
        "{error}"
    );
    assert!(!error.contains(".exact-js-bake-"), "{error}");
    f.write(
        "ui/label.contract",
        "component Label\n  props\n    body: string\n  view\n    text missing\n",
    );
    let error = producer.bake(&f.0, None).err().unwrap();
    assert!(
        error.contains(&format!(
            "{}:5:",
            source.join("ui/label.contract").display()
        )),
        "{error}"
    );
    assert!(!error.contains(".exact-js-bake-"), "{error}");
}

#[cfg(unix)]
#[test]
fn resident_compilation_refusals_are_drained_before_the_next_request() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let defaults = Tools::default();
    let wrapper = f.0.join("hermes-wrapper");
    let launch = format!(
        "#!/bin/sh\nexec '{}' \"$@\"\n",
        defaults.hermesc.to_str().unwrap().replace('\'', "'\"'\"'")
    );
    f.write("hermes-wrapper", "#!/bin/sh\nexit 1\n");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut producer = exact_js_bake::Producer::new(Tools {
        hermesc: wrapper,
        ..defaults
    })
    .unwrap();
    assert!(producer
        .bake(&f.0, None)
        .err()
        .unwrap()
        .contains("hermes-wrapper refused"));
    f.write("hermes-wrapper", &launch);
    assert_eq!(producer.bake(&f.0, None).unwrap().receipt, f.bake().receipt);
    f.write("logic.ts", "export const prefix: string = 42;");
    f.write("hermes-wrapper", "#!/bin/sh\nexit 1\n");
    let error = producer.bake(&f.0, None).err().unwrap();
    assert!(
        error.contains("TS2322") && error.contains("hermes-wrapper refused"),
        "{error}"
    );
    f.write("hermes-wrapper", &launch);
    assert!(producer.bake(&f.0, None).err().unwrap().contains("TS2322"));
    f.write("logic.ts", "export const prefix = 'recovered: ';");
    assert_eq!(producer.bake(&f.0, None).unwrap().receipt, f.bake().receipt);
}

#[test]
fn both_producer_paths_check_worker_web_types_and_refuse_dom_ui_types() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    let accepted = "export const prefix = 'web: '; export async function request(url: URL, init: RequestInit): Promise<string> { const response: Response = await fetch(url, init); const headers: Headers = response.headers; return headers.get('content-type') ?? await response.text(); }";
    f.write("logic.ts", accepted);
    assert_eq!(producer.bake(&f.0, None).unwrap().receipt, f.bake().receipt);
    f.write(
        "logic.ts",
        &format!("{accepted} type UI = Document | HTMLElement | Window;"),
    );
    for error in [
        producer.bake(&f.0, None).err().unwrap(),
        bake(&f.0, &Tools::default()).err().unwrap(),
    ] {
        for name in ["Document", "HTMLElement", "Window"] {
            assert!(
                error.contains(name),
                "missing UI-type diagnostic for {name}: {error}"
            );
        }
    }
    f.write(
        "logic.ts",
        &accepted.replace("Promise<string>", "Promise<number>"),
    );
    for error in [
        producer.bake(&f.0, None).err().unwrap(),
        bake(&f.0, &Tools::default()).err().unwrap(),
    ] {
        assert!(
            error.contains("TS2322"),
            "fetch result types remain checked: {error}"
        );
    }
    f.write("logic.ts", accepted);
    assert_eq!(producer.bake(&f.0, None).unwrap().receipt, f.bake().receipt);
}

/// The web build of the fixture app (the JS target), into `web-out` inside it.
fn web_build(app: &Path) -> Result<(), String> {
    let out = app.join("web-out");
    let _ = std::fs::remove_dir_all(&out);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let built = std::process::Command::new("bun")
        .arg(root.join("host/web-js/build.mjs"))
        .args(["logic", "--render", "none", "--out"])
        .arg(&out)
        .env("EXACT_APP_DIR", app)
        .current_dir(&root)
        .output()
        .unwrap();
    if built.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&built.stderr).into_owned())
    }
}

/// A package's declarations reach the type check of every producer from the
/// capture (LLP 1027 D5, "Type-only package imports"): a type-only import
/// checks against them, a changed `.d.ts` checks again, and a value import,
/// a library the configuration does not load, or a global types package no
/// file names is refused alike.
#[test]
fn type_only_package_imports_check_alike_and_package_code_never_runs() {
    let f = Fixture::new();
    let put = |name: &str, bytes: &str| {
        std::fs::create_dir_all(f.0.join(name).parent().unwrap()).unwrap();
        f.write(name, bytes);
    };
    // A mounted directory names its own copy of a package of the same name,
    // installed beside it, outside the app.
    let shared = Fixture(f.0.with_extension("shared"));
    std::fs::create_dir_all(shared.0.join("node_modules/rows")).unwrap();
    shared.write(
        "node_modules/rows/package.json",
        r#"{"name":"rows","types":"index.d.ts"}"#,
    );
    shared.write(
        "node_modules/rows/index.d.ts",
        "export type Row = { id: 'shared' };\n",
    );
    shared.write(
        "kind.ts",
        "import type { Row } from 'rows';\nexport type Kind = Row['id'];\n",
    );
    put(
        "app.json",
        &format!(
            r#"{{"name":"Logic","app":{{"id":"test.exact.logic","name":"Logic"}},"typescript":{{"sources":{{"shared":"../{}"}}}}}}"#,
            shared.0.file_name().unwrap().to_str().unwrap()
        ),
    );
    put(
        "node_modules/rows/package.json",
        r#"{"name":"rows","type":"module","exports":{".":{"types":"./dist/index.d.ts","import":"./dist/index.js"}}}"#,
    );
    let index = "import type { Inner } from './inner.js';\nimport type { Unit } from 'units';\nimport type { Plain } from 'plain';\nexport type Row = { id: string; count: Inner; unit: Unit; plain: Plain };\nexport declare const rows: Row[];\n";
    put("node_modules/rows/dist/index.d.ts", index);
    put(
        "node_modules/rows/dist/inner.d.ts",
        "export type Inner = number;\n",
    );
    put(
        "node_modules/rows/dist/index.js",
        "export const rows = [];\n",
    );
    // Its own nested dependency, and a package typed by its `@types` companion.
    put(
        "node_modules/rows/node_modules/units/package.json",
        r#"{"name":"units","types":"index.d.ts"}"#,
    );
    put(
        "node_modules/rows/node_modules/units/index.d.ts",
        "export type Unit = 'kg';\n",
    );
    put("node_modules/plain/package.json", r#"{"name":"plain"}"#);
    // Its code says so if it ever runs.
    let ran = f.0.with_extension("ran");
    put(
        "node_modules/plain/index.js",
        &format!(
            "require('node:fs').writeFileSync({:?}, 'ran');\nmodule.exports = {{}};\n",
            ran.to_str().unwrap()
        ),
    );
    put(
        "node_modules/@types/plain/package.json",
        r#"{"name":"@types/plain"}"#,
    );
    put(
        "node_modules/@types/plain/index.d.ts",
        "export type Plain = true;\n",
    );
    // Installed, named by no file: its globals are not the app's.
    put(
        "node_modules/@types/ambient/package.json",
        r#"{"name":"@types/ambient"}"#,
    );
    put(
        "node_modules/@types/ambient/index.d.ts",
        "declare var process: { env: Record<string, string> };\n",
    );
    // A generated module beside app.ts that names the package's types, as
    // `snapback4 types` writes `snapback/generated/api.ts`.
    put("generated.ts", "import type { Row } from /* its types */ 'rows';\nexport type Rows = { item: Row };\nexport const names = ['item'];\n");
    let logic = |count: &str| {
        format!("import type {{ Rows }} from './generated.ts';\nimport {{ names }} from './generated.ts';\nconst row: Rows['item'] = {{ id: 'a', count: {count}, unit: 'kg', plain: true }};\nexport const prefix = names[0] + row.count + ': ';\nimport type {{ Kind }} from './shared/kind.ts';\nexport const kind: Kind = 'shared';\n")
    };
    put("logic.ts", &logic("2"));
    let engine = exact_js::ENGINE_LINKED;
    let mut producer = engine.then(|| exact_js_bake::Producer::new(Tools::default()).unwrap());
    // The web build's output sits inside the app here, where a native
    // capture would take its stage's sources for the app's own.
    let mut every = |f: &Fixture| {
        let mut results = vec![web_build(&f.0)];
        let _ = std::fs::remove_dir_all(f.0.join("web-out"));
        if let Some(producer) = producer.as_mut() {
            results.push(producer.bake(&f.0, None).map(|_| ()));
            results.push(bake(&f.0, &Tools::default()).map(|_| ()));
        }
        results
    };
    web_build(&f.0).unwrap();
    let stage = f.0.join("web-out/.gen/typescript");
    assert!(stage
        .join("node_modules/rows/node_modules/units/index.d.ts")
        .exists());
    assert!(stage.join("shared/node_modules/rows/index.d.ts").exists());
    assert!(
        !stage.join("node_modules/rows/dist/index.js").exists()
            && !stage.join("node_modules/@types/ambient").exists(),
        "only the declarations of named packages are captured"
    );
    for result in every(&f) {
        result.unwrap();
    }
    if engine {
        let live = paired(&f.bake());
        let live = Runner::boot(
            live.plan,
            live.module,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        assert_eq!(live.resource("message"), Some(&Value::str("item2: 0")));
    }
    let refused = |every: &mut dyn FnMut(&Fixture) -> Vec<Result<(), String>>, why: &str| {
        let results = every(&f);
        assert_eq!(results.len(), if engine { 3 } else { 1 });
        for result in results {
            let error = result.expect_err(why);
            assert!(error.contains(why), "{why}: {error}");
        }
    };
    put("logic.ts", &logic("'two'"));
    refused(&mut every, "error TS2322");
    put("logic.ts", &logic("2"));
    // A changed declaration checks the app again (the resident checker's
    // incremental state included).
    put(
        "node_modules/rows/dist/inner.d.ts",
        "export type Inner = string;\n",
    );
    refused(&mut every, "error TS2322");
    put(
        "node_modules/rows/dist/inner.d.ts",
        "export type Inner = number;\n",
    );
    for result in every(&f) {
        result.unwrap();
    }
    // An uninstalled package is gone from the capture too (the resident
    // stage keeps nothing a previous request staged).
    std::fs::rename(
        f.0.join("node_modules/@types/plain"),
        f.0.join("plain-types"),
    )
    .unwrap();
    refused(&mut every, "'plain'");
    std::fs::rename(
        f.0.join("plain-types"),
        f.0.join("node_modules/@types/plain"),
    )
    .unwrap();
    put(
        "logic.ts",
        "import { rows } from 'rows';\nexport const prefix = rows.length + ': ';\n",
    );
    let real = f.0.canonicalize().unwrap();
    refused(
        &mut every,
        &format!(
            "module outside captured app: {}",
            real.join("node_modules/rows/dist/index.js").display()
        ),
    );
    // A side-effect import is a value import: refused before any producer,
    // the web build's own reading of app.ts included, runs the package.
    put("logic.ts", &format!("import 'plain';\n{}", logic("2")));
    refused(&mut every, "module outside captured app: ");
    assert!(!ran.exists(), "a refused package's code never runs");
    // A specifier that leaves its package names no file of it.
    put("helper.ts", "export const helper = 'helped: ';\n");
    put(
        "logic.ts",
        "import { helper } from 'plain/../../helper.ts';\nexport const prefix = helper;\n",
    );
    refused(
        &mut every,
        "module outside captured app: plain/../../helper.ts",
    );
    put(
        "logic.ts",
        "export const prefix = process.env.PREFIX + ': ';\n",
    );
    refused(&mut every, "Cannot find name 'process'");
    put("logic.ts", &logic("2"));
    // A declaration that reaches outside the capture, or loads a library the
    // configuration does not, is refused by every producer's graph check.
    let outside = Fixture(f.0.with_extension("outside"));
    std::fs::create_dir(&outside.0).unwrap();
    outside.write("outside.d.ts", "type Outside = string;\n");
    put(
        "node_modules/rows/dist/index.d.ts",
        &format!(
            "/// <reference path={:?} />\n{index}",
            outside.0.join("outside.d.ts").to_str().unwrap()
        ),
    );
    refused(&mut every, "module outside captured app: ");
    put(
        "node_modules/rows/dist/index.d.ts",
        &format!("/// <reference lib=\"es2024.arraybuffer\" />\n{index}"),
    );
    refused(&mut every, "lib.es2024.arraybuffer.d.ts");
}

/// One TypeScript configuration (`js/bake/src/typescript.mjs`) in both
/// producers and the web build: ES2023 runs on Hermes as in a browser, a
/// `.ts` import path resolves, and what one refuses every one refuses with
/// the same diagnostic (calc F2, calendar F9/F11).
#[test]
fn every_build_takes_one_typescript_configuration_and_refuses_alike() {
    let f = Fixture::new();
    f.write("app.ts", &SOURCE.replace("'./logic'", "'./logic.ts'"));
    f.write(
        "app.json",
        r#"{"name":"Logic","app":{"id":"test.exact.logic","name":"Logic"}}"#,
    );
    let accepted = "export const prefix = [['b', 2], ['a', 1]].toSorted().map(([k]) => k).join('').replaceAll('a', 'A') + [1, 2].at(-1) + [1, 2].findLast(n => n < 2) + Object.keys(Object.groupBy([1], n => 'g' + n)) + ': ';";
    f.write("logic.ts", accepted);
    let web = || web_build(&f.0);
    web().unwrap();
    // The output sits inside the app here; the capture leaves it out
    // rather than copying its own stage into itself (review r4a 2).
    let stage = f.0.join("web-out/.gen/typescript");
    assert!(stage.join("logic.ts").exists() && !stage.join("web-out").exists());
    assert!(
        f.0.join("app.contract.d.ts").exists(),
        "a development build writes the declarations beside app.ts"
    );
    std::fs::remove_file(f.0.join("app.contract.d.ts")).unwrap();
    let engine = exact_js::ENGINE_LINKED;
    if engine {
        let baked = f.bake();
        let candidate = paired(&baked);
        let live = Runner::boot(
            candidate.plan,
            candidate.module,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        assert_eq!(live.resource("message"), Some(&Value::str("Ab21g1: 0")));
    }
    // ES2024's RegExp `v` flag is not in Hermes, so not in the library.
    f.write("logic.ts", "export const prefix = /[a]/v.source;");
    let mut refusals = vec![web().unwrap_err()];
    if engine {
        let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
        refusals.push(producer.bake(&f.0, None).err().unwrap());
        refusals.push(bake(&f.0, &Tools::default()).err().unwrap());
    }
    for error in refusals {
        assert!(
            error.contains("logic.ts(1,28): error TS1501"),
            "the one diagnostic: {error}"
        );
    }
}

#[test]
fn an_alias_resolves_a_mounted_source_alike_in_both_producers_and_names_only_the_capture() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    // A shared directory that imports through its own project's alias.
    let shared = Fixture(f.0.with_extension("shared"));
    std::fs::create_dir_all(shared.0.join("deep")).unwrap();
    shared.write(
        "prefix.ts",
        "import { word } from '@/lib/deep/word';\nexport const prefix = word + ': ';\n",
    );
    shared.write("deep/word.ts", "export const word = 'aliased';\n");
    f.write("app.json", &format!(
        r#"{{"app":{{"id":"test.exact.logic","name":"Logic"}},"typescript":{{"sources":{{"lib":"../{}"}}}}}}"#,
        shared.0.file_name().unwrap().to_str().unwrap()
    ));
    f.write(
        "tsconfig.json",
        r#"{
        // Editors and both producers share these paths.
        "compilerOptions": {"baseUrl":".", "paths": {"@/lib/*":["missing/*", "lib/*"]}},
    }"#,
    );
    f.write("logic.ts", "export { prefix } from '@/lib/prefix';\n");
    let standalone = f.bake();
    assert!(String::from_utf8_lossy(&standalone.script).contains("aliased"));
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    assert_eq!(producer.bake(&f.0, None).unwrap().script, standalone.script);
    // Removing paths invalidates the resident compiler too.
    f.write("tsconfig.json", "{}");
    for error in [
        producer.bake(&f.0, None).err().unwrap(),
        bake(&f.0, &Tools::default()).err().unwrap(),
    ] {
        assert!(error.contains("@/lib/prefix"), "{error}");
    }
    // Neither compiler may resolve a path outside the captured source graph.
    for paths in [
        r#"{"@/lib/*":["../elsewhere/*"]}"#,
        r#"{"@/lib/*":"lib/*"}"#,
    ] {
        f.write(
            "tsconfig.json",
            &format!(r#"{{"compilerOptions":{{"paths":{paths}}}}}"#),
        );
        for error in [
            producer.bake(&f.0, None).err().unwrap(),
            bake(&f.0, &Tools::default()).err().unwrap(),
        ] {
            assert!(error.contains("tsconfig"), "{error}");
        }
    }
}

/// A first-frame source over the runtime's 100 ms budget still bakes: the
/// budget is about a device, and a build machine's load must not fail a
/// build (LLP 1027 §6). The loop is deterministic work, far over 100 ms
/// on any machine this runs on, so the test never depends on timing.
#[test]
fn a_slow_first_frame_source_bakes_whatever_the_wall_clock() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    f.write(
        "app.ts",
        &SOURCE.replace(
            "return prefix + count;",
            "let n = 0; for (let i = 0; i < 10_000_000; i++) n = (n + i) % 7; return prefix + count + n;",
        ),
    );
    let baked = f.bake();
    let candidate = paired(&baked);
    let live = Runner::boot(
        candidate.plan,
        candidate.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        live.resource("message").is_some(),
        "the slow source's first frame is baked"
    );
}

/// The clock in the app's own code is refused at build, by file and line,
/// in both compilers, so a Bun test (no guard there) cannot hide it from a
/// device that refuses it on first use (LLP 1027.000).
#[test]
fn the_clock_in_a_data_module_is_refused_at_build_by_file_and_line() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    f.write(
        "logic.ts",
        "export const prefix = 'old: ';\nexport const stamp = () => Date.now();\n",
    );
    let error = bake(&f.0, &Tools::default())
        .err()
        .expect("Date.now() refused");
    assert!(
        error.contains("logic.ts:2:28: Date.now() is unavailable in data sources; pass time or a random seed as an argument"),
        "{error}"
    );
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    assert!(producer
        .bake(&f.0, None)
        .err()
        .unwrap()
        .contains("logic.ts:2:28: Date.now()"));
    // Through a global object, past `!`, after a CR line break.
    f.write("logic.ts", "export const prefix = 'old: ';\rexport const a = () => globalThis.Date.now();\nexport const b = () => Date.now!();\nexport const c = () => window.setTimeout(() => {}, 1);\n");
    let error = producer.bake(&f.0, None).err().unwrap();
    assert!(error.contains("logic.ts:2:24: Date.now()"), "{error}");
    assert!(error.contains("logic.ts:3:24: Date.now()"), "{error}");
    assert!(
        error.contains(
            "logic.ts:4:24: setTimeout() is unavailable in data sources: there are no timers"
        ),
        "{error}"
    );
    // An angle-bracket assertion, a `declare`d class and a type-only
    // namespace are erased: the global runs.
    f.write("logic.ts", "export const prefix = 'old: ';\ndeclare class Date { static now(): number }\nnamespace Math { export type R = number }\nexport const a = () => (<any>Date).now() + Math.random();\n");
    let error = producer.bake(&f.0, None).err().unwrap();
    assert!(error.contains("logic.ts:4:24: Date.now()"), "{error}");
    assert!(error.contains("logic.ts:4:44: Math.random()"), "{error}");
    // An explicit date, a member named `now` elsewhere, a comment, and a
    // `Date` or `performance` the module binds itself are fine.
    f.write("logic.ts", "export const prefix = 'old: ';\n// Date.now() is refused\nexport const epoch = new Date(0).getTime() + ({ now: () => 1 }).now();\nexport const stamp = (performance: { now(): number }) => performance.now();\n");
    assert!(
        producer.bake(&f.0, None).is_ok(),
        "{:?}",
        producer.bake(&f.0, None).err()
    );
    f.write("logic.ts", "export const prefix = 'old: ';\nnamespace Date { export function now() { return 1; } }\nexport const local = () => Date.now();\n");
    assert!(
        producer.bake(&f.0, None).is_ok(),
        "{:?}",
        producer.bake(&f.0, None).err()
    );
}

/// Literal property access has the same build diagnostic as dot access.
#[test]
fn literal_members_keep_ambient_diagnostics_in_both_producers() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    f.write(
        "logic.ts",
        r#"export const prefix = 'old: ';
export const a = () => Date['now']();
export const b = () => globalThis['setTimeout'](() => {}, 1);
export const c = () => Math[`random`]();
export const d = () => globalThis['performance'][`now`]();
export const e = () => new globalThis['Date']();
export const f = () => self[`Date`]();
export const g = () => globalThis['Date'].now?.();
export const h = () => Date[('now' as const)]!();
export const i = () => Date['n\u006fw']();
"#,
    );
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    for result in [bake(&f.0, &Tools::default()), producer.bake(&f.0, None)] {
        let error = result
            .err()
            .expect("literal ambient calls refused at build");
        for (line, api) in [
            (2, "Date.now()"),
            (3, "setTimeout()"),
            (4, "Math.random()"),
            (5, "performance.now()"),
            (6, "new Date()"),
            (7, "Date()"),
            (8, "Date.now()"),
            (9, "Date.now()"),
            (10, "Date.now()"),
        ] {
            assert!(
                error.contains(&format!(
                    "logic.ts:{line}:24: {api} is unavailable in data sources"
                )),
                "{error}"
            );
        }
    }
    // Dynamic keys still reach the runtime guard; explicit dates and
    // unrelated objects are valid. None of these functions reads the clock at bake.
    f.write("logic.ts", "export const prefix = 'old: ';\nexport const epoch = new globalThis['Date'](0).getTime();\nexport const dynamic = (key: 'now') => Date[key]();\nexport const template = (part: 'ow') => Date[`n${part}`]();\nexport const other = () => ({ now: () => 1 })['now']();\n");
    for result in [bake(&f.0, &Tools::default()), producer.bake(&f.0, None)] {
        assert!(result.is_ok(), "{:?}", result.err());
    }
    // Literal access on locally bound names is not an ambient read.
    f.write("logic.ts", "export const prefix = 'old: ';\nexport const a = (Date: { now(): number }) => Date['now']();\nexport const b = (Math: { random(): number }) => Math[`random`]();\nexport const c = (globalThis: { setTimeout(): number }) => globalThis['setTimeout']();\nexport const d = (performance: { now(): number }) => performance[`now`]();\n");
    for result in [bake(&f.0, &Tools::default()), producer.bake(&f.0, None)] {
        assert!(result.is_ok(), "{:?}", result.err());
    }
}

/// LLP 1091.001: a native producer keeps package identity without symlink privilege.
#[cfg(windows)]
#[test]
fn windows_imported_packages_keep_identity_through_real_bakes() {
    // No stub-engine skip: bake must succeed through real Hermes below.
    let f = Fixture::new();
    let package = f.0.join("node_modules/.packages/café # 雪");
    std::fs::create_dir_all(package.join("src")).unwrap();
    std::fs::create_dir_all(package.join(".hidden")).unwrap();
    let inputs = [
        (
            package.join("package.json"),
            r#"{"name":"fixture-ui","version":"1.0.0","exports":"./src/card.contract"}"#,
        ),
        (
            package.join(".hidden/pad.contract"),
            "style Pad\n  padding-top=10\n",
        ),
        (
            package.join("src/card.contract"),
            "use Pad from \"../.hidden/pad.contract\"\ncomponent Card\n  view\n    column class=Pad\n      text \"from package\"\n",
        ),
    ];
    for (path, text) in &inputs {
        std::fs::write(path, text).unwrap();
    }
    // These are the app's installed aliases, before the producer creates its stage.
    let linked = exact_bake::bun()
        .args([
            "-e",
            "const fs=require('node:fs'),p=require('node:path'); const root=fs.realpathSync.native(process.argv[1]),app=process.argv[2]; for(const name of ['ui','@survey/ui']){const link=p.join(app,'node_modules',name);fs.mkdirSync(p.dirname(link),{recursive:true});fs.symlinkSync(root,link,'junction');}",
        ])
        .arg(&package)
        .arg(&f.0)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let contract = format!(
        "use Card as First from \"ui\"\nuse Card as Second from \"@survey/ui\"\n{}",
        CONTRACT.replace(
            "      text message",
            "      First()\n      Second()\n      text message"
        )
    );
    f.write("app.contract", &contract);
    contract::compile_path(&f.0.join("app.contract")).unwrap();
    let graph = contract::source_graph(&f.0.join("app.contract"));
    assert!(graph.errors.is_empty());
    assert_eq!(graph.packages.len(), 2);
    let canonical = package.canonicalize().unwrap();
    assert!(graph.packages.iter().all(|p| p.root == canonical));
    let card = package.join("src/card.contract").canonicalize().unwrap();
    assert_eq!(graph.sources.iter().filter(|s| s.path == card).count(), 1);

    let first = f.bake();
    let candidate = paired(&first);
    let live = Runner::boot(
        candidate.plan,
        candidate.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(live.resource("message"), Some(&Value::str("old: 0")));
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    let resident = producer.bake(&f.0, None).unwrap();
    let again = producer.bake(&f.0, None).unwrap();
    assert_eq!(resident.plan, first.plan);
    assert_eq!(again.receipt, resident.receipt);
    let map: Json = serde_json::from_str(resident.source_map.as_ref().unwrap()).unwrap();
    let nodes: Vec<_> = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["component"] == "Card")
        .collect();
    assert!(!nodes.is_empty());
    for node in nodes {
        assert_eq!(node["file"], card.to_str().unwrap());
    }
    for (path, text) in inputs {
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }
    for (name, text) in [
        ("app.contract", contract.as_str()),
        ("app.ts", SOURCE),
        ("logic.ts", "export const prefix = 'old: ';\n"),
    ] {
        assert_eq!(std::fs::read_to_string(f.0.join(name)).unwrap(), text);
    }
}
