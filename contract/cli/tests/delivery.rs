//! The `delivery` resource and its two commands (LLP 1030 D7), end to end
//! over `contract/corpus/delivery.contract`: the embedded answer a baked
//! plan carries, a host's facts arriving in one commit, `state.delivery` as
//! the agent's mirror, the commands reaching `take_commands` by name, and
//! the bake refusing a field the runner cannot fill.

use exact_kernel::{Kernel, PropId};
use exact_runner::{agent, DataError, DataSource, Delivery, Runner, Value};
use std::path::Path;

/// A data source with nothing in it: `exactDelivery` never reaches here.
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

fn corpus() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/delivery.contract");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let baked = contract::bake(plan, NoData).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(baked, NoData, Kernel::with_monospace()).unwrap()
}

fn text_of(r: &Runner<NoData>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}

/// Read the top-level delivery mirror independently of sibling diagnostics
/// and the fixture's resource, which is also called `delivery`.
fn mirror(r: &Runner<NoData>) -> serde_json::Value {
    let state: serde_json::Value = serde_json::from_str(&agent::state(r)).unwrap();
    state
        .get("delivery")
        .expect("state has no delivery")
        .clone()
}

#[test]
fn a_baked_plan_shows_the_embedded_answer_with_no_host_in_sight() {
    let r = boot(&corpus());
    assert_eq!(text_of(&r, "delivery-stream").as_deref(), Some("embedded"));
    assert_eq!(text_of(&r, "delivery-seq").as_deref(), Some("0"));
    assert_eq!(text_of(&r, "delivery-staged").as_deref(), Some("current"));
    assert_eq!(r.delivery(), &Delivery::default());
    assert_eq!(
        mirror(&r),
        serde_json::from_str::<serde_json::Value>(
            "{\"stream\":\"embedded\",\"seq\":0,\"embeddedSeq\":0,\"staged\":false,\
         \"sunset\":\"\",\"interpreted\":[],\"compatibilityId\":\"\",\"L\":\"A\",\
         \"E\":[\"native\"]}"
        )
        .unwrap()
    );
}

#[test]
fn a_hosts_facts_re_answer_the_resource_in_one_commit_and_the_agent_sees_them() {
    let mut r = boot(&corpus());
    let receipt = r
        .set_delivery(Delivery {
            stream: "prod/abc".into(),
            seq: 44,
            embedded_seq: 7,
            staged: true,
            sunset: Some("This build stops receiving updates in March".into()),
            interpreted: vec!["profile".into()],
            compatibility_id: "9f1c0a2b".into(),
            store: 'A',
            executors: vec!["hermes".into(), "native".into()],
        })
        .unwrap()
        .expect("the resource is declared, so its answer changed");
    // One commit: the two texts changed, and the `when` arm swapped one
    // text node for the other.
    assert!(
        !receipt.touched.is_empty() || !receipt.created.is_empty(),
        "the commit moved nothing"
    );
    assert_eq!(text_of(&r, "delivery-stream").as_deref(), Some("prod/abc"));
    assert_eq!(text_of(&r, "delivery-seq").as_deref(), Some("44"));
    assert_eq!(text_of(&r, "delivery-staged").as_deref(), Some("staged"));
    assert_eq!(
        mirror(&r),
        serde_json::from_str::<serde_json::Value>(
            "{\"stream\":\"prod/abc\",\"seq\":44,\"embeddedSeq\":7,\"staged\":true,\
         \"sunset\":\"This build stops receiving updates in March\",\
         \"interpreted\":[\"profile\"],\"compatibilityId\":\"9f1c0a2b\",\"L\":\"A\",\
         \"E\":[\"hermes\",\"native\"]}"
        )
        .unwrap()
    );
    // The same facts again are no commit at all.
    let same = r.delivery().clone();
    assert!(r.set_delivery(same).unwrap().is_none());
}

#[test]
fn a_compat_file_names_the_cohort_and_leaves_the_stream_alone() {
    let mut r = boot(&corpus());
    let compat = concat!(
        r#"{"id":"1a2b3c4d5e6f70819a2b3c4d5e6f7081","inputs":{"app":"io.exact.fixture","#,
        r#""executors":["hermes","native"],"store":{"L":"0","acceptedKinds":["plan"]}}}"#,
        "\n"
    );
    assert!(r.set_delivery_from_compat(compat).unwrap().is_some());
    assert_eq!(
        r.delivery().compatibility_id,
        "1a2b3c4d5e6f70819a2b3c4d5e6f7081"
    );
    assert_eq!(r.delivery().store, '0');
    assert_eq!(r.delivery().executors, ["hermes", "native"]);
    // An L = 0 client still answers its embedded entry and nothing staged.
    assert_eq!(text_of(&r, "delivery-stream").as_deref(), Some("embedded"));
    assert_eq!(text_of(&r, "delivery-staged").as_deref(), Some("current"));
    assert_eq!(mirror(&r)["L"], "0");
    assert_eq!(
        mirror(&r)["compatibilityId"],
        "1a2b3c4d5e6f70819a2b3c4d5e6f7081"
    );
}

#[test]
fn the_two_commands_reach_the_host_by_name() {
    let mut r = boot(&corpus());
    let check = r.kernel().find_by_test_id("delivery-check")[0];
    let activate = r.kernel().find_by_test_id("delivery-activate")[0];
    let (check, activate) = (
        r.kernel().node_by_key(check).unwrap().id,
        r.kernel().node_by_key(activate).unwrap().id,
    );
    r.dispatch(check, exact_runner::Event::Press).unwrap();
    r.dispatch(activate, exact_runner::Event::Press).unwrap();
    let commands: Vec<String> = r.take_commands().into_iter().map(|c| c.name).collect();
    assert_eq!(commands, ["deliveryCheck", "deliveryActivate"]);
    // The same two by name, which is what an agent or a test drives.
    r.act("check", Vec::new()).unwrap();
    r.act("activate", Vec::new()).unwrap();
    let commands: Vec<String> = r.take_commands().into_iter().map(|c| c.name).collect();
    assert_eq!(commands, ["deliveryCheck", "deliveryActivate"]);
}

#[test]
fn a_field_the_runner_cannot_fill_is_refused_at_bake_by_name() {
    let src = corpus().replace("  stream: string\n", "  stream: string\n  foo: string\n");
    let plan = contract::compile(&src).unwrap();
    let error = contract::bake(plan, NoData).unwrap_err();
    let message = error.to_string();
    assert!(
        message.starts_with("[bake-delivery-field]")
            && message.contains('`')
            && message.contains("foo"),
        "{message}"
    );
}

#[test]
fn manifest_identity_survives_an_unnamed_data_standin_and_conflicts_fail() {
    let directory =
        std::env::temp_dir().join(format!("exact-manifest-plan-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("app.json"),
        r#"{"app":{"id":"com.exact.standin","name":"Stand-in"}}"#,
    )
    .unwrap();
    let path = directory.join("app.contract");
    std::fs::write(&path, corpus()).unwrap();
    let compiled = contract::compile_path(&path).unwrap();
    let baked = contract::bake(compiled.clone(), NoData).unwrap();
    assert_eq!(baked.app_id, "com.exact.standin");
    assert_eq!(
        exact_plan::Plan::decode(&baked.encode()).unwrap().app_id,
        "com.exact.standin"
    );
    struct Conflict;
    impl DataSource for Conflict {
        fn app_id(&self) -> &str {
            "com.exact.conflict"
        }
        fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }
    assert!(contract::bake(compiled, Conflict).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn macos_window_metadata_is_validated_and_baked_from_the_manifest() {
    let script = r#"
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { validate } from './scripts/app.mjs';
import { macInfoPlist } from './host/apple/build.mjs';
const schema = JSON.parse(readFileSync('./scripts/app.schema.json'));
const app = {name:'Notebook',app:{id:'test.exact.notebook',name:'Notebook'},host:{macos:{window:{width:1100,height:760,minWidth:760,minHeight:560}}}};
assert.deepEqual(validate(app,schema,'',schema),[]);
const plist = macInfoPlist({id:app.app.id,displayName:app.app.name,manifest:app});
for (const key of ['ExactWindow','width','height','minWidth','minHeight']) assert.ok(plist.includes(`<key>${key}</key>`));
assert.ok(plist.includes('<string>Notebook</string>'));
for (const bad of [0,-1,16385,'1100',null]) {
  app.host.macos.window.width=bad;
  assert.ok(validate(app,schema,'',schema).length,JSON.stringify(bad));
}
delete app.host.macos.window;
assert.deepEqual(validate(app,schema,'',schema),[]);
assert.ok(!macInfoPlist({id:app.app.id,displayName:app.app.name,manifest:app}).includes('ExactWindow'));
"#;
    let output = std::process::Command::new("bun")
        .args(["--input-type=module", "-e", script])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
