//! Actual tsc → Rolldown → HBC → bake, then replacement in the native host.
use exact_apple::abi::{Bridge, Hooks};
use exact_js::{Module, Paired};
use exact_js_bake::{bake, Baked, Tools};
use exact_kernel::Kernel;
use exact_runner::{Runner, Value};
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
    assert_eq!(changed.data().take_logs(), ["message called"]);
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
            &format!("'{}'", outside.0.join("logic").display()),
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
    let f = Fixture::new();
    let first = f.bake();
    let mut a = Bridge::<Module>::new();
    let mut b = Bridge::<Module>::new();
    for bridge in [&mut a, &mut b] {
        prepare(bridge, &first).unwrap();
        let count = bridge.commit_plan();
        assert!(output(bridge, count)["error"].is_null());
        let count = bridge.data_ready();
        assert!(output(bridge, count)["error"].is_null());
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
        "../../../vendor/ibex2/src/bindings/storage.d.ts"
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
    let tools = Tools {
        tsc: PathBuf::from("/usr/bin/false"),
        ..Tools::default()
    };
    let mut producer = exact_js_bake::Producer::new(tools).unwrap();
    assert!(producer
        .bake(&f.0, None)
        .err()
        .unwrap()
        .contains("/usr/bin/false refused"));
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
