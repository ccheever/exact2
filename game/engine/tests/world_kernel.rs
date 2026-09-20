//! Both libraries are dependencies of a temporary consumer, never of the root
//! workspace or of one another. This is the only game-tree change for K1.
use std::{fs, process::Command};
#[test]
fn generic_world_hash_and_data_are_identical_to_the_kernel() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let scratch = root
        .parent()
        .unwrap()
        .join("scratch")
        .join("k1b-1").join(format!("cross-{}", std::process::id()));
    fs::create_dir_all(scratch.join("src")).unwrap();
    fs::write(
        scratch.join("Cargo.toml"),
        format!(
            r#"[package]
name = "world-cross-check"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
exact-world = {{ path = {:?} }}
exact-game = {{ path = {:?} }}
"#,
            root.join("world"),
            root.join("game/engine")
        ),
    )
    .unwrap();
    let fixture = r#"
use ENGINE::*;
#[derive(Default, Component)] struct Counter { n: u64, bits: f32, text: String, link: Option<Entity> }
#[derive(Default, Component)] struct Other(Vec<u16>);
#[derive(Default, Resource)] struct Ledger { entries: std::collections::BTreeMap<String, (i64, Vec<f32>)> }
#[derive(Default, Data)] enum Choice { #[default] None, Some { n: u64, values: Vec<u32> } }
pub fn run() -> (Vec<(u64,Vec<u8>)>, Vec<Vec<u8>>) {
    let mut w = World::new(120, 77);
    w.register::<Other>().register::<Counter>().register_resource::<Ledger>();
    let mut entities = Vec::new();
    for i in 0..2117 { entities.push(w.spawn_named(if i % 7 == 0 { "same" } else { "other" }, ())); }
    for &e in entities.iter().rev() {
        w.insert(e, Counter { n: u64::MAX-e.index() as u64, bits: if e.index() % 3 == 0 { -0. } else { f32::from_bits(0x7f800001) }, text: "héllo 🌕".into(), link: Some(entities[0]) });
        if e.index()%3 == 0 { w.insert(e,Other(vec![0,65535])); }
    }
    let mut ledger = Ledger::default(); ledger.entries.insert("key".into(), (i64::MIN,vec![-0.,f32::NAN])); w.insert_resource(ledger);
    let mut history = vec![(w.hash(),w.save())];
    for batch in 0..4 {
        for i in (batch..entities.len()).step_by(13) { w.despawn(entities[i]); }
        for i in (batch..entities.len()).step_by(13) { let n=w.rng().next_u32() as u64; entities[i] = w.spawn((Counter { n, ..Counter::default() },)); }
        for (_, (c,o)) in w.query::<(&mut Counter,Option<&Other>)>().iter() { c.n = c.n.wrapping_add(o.map_or(0,|o| o.0.len() as u64)); }
        w.emit("delivery"); w.publish("score",batch as u32);
        history.push((w.hash(),w.save()));
        let bytes = w.save(); w.load(&bytes).unwrap(); assert_eq!(w.save(),bytes);
    }
    let data = vec![bin::to_vec(&Choice::Some { n:u64::MAX, values:vec![0,u32::MAX] }), bin::to_vec(&(-0f32,f64::NAN,Some(entities[0]),vec![1u8,2,3])),bin::to_vec(&Value::record(vec![Value::Bool(true),Value::str("x")]))];
    (history,data)
}
"#;
    let fixture_new = fixture.replace("ENGINE", "exact_world")
        .replace("register::<Other>()", "register::<Other>().unwrap()")
        .replace("register::<Counter>()", "register::<Counter>().unwrap()")
        .replace("register_resource::<Ledger>()", "register_resource::<Ledger>().unwrap()")
        .replace("}, ())", "}, ()).unwrap()")
        .replace("link: Some(entities[0]) });", "link: Some(entities[0]) }).unwrap();")
        .replace("Other(vec![0,65535]))", "Other(vec![0,65535])).unwrap()")
        .replace("insert_resource(ledger)", "insert_resource(ledger).unwrap()")
        .replace("..Counter::default() },))", "..Counter::default() },)).unwrap()");
    // Old EXGAME v3 retained empty columns. Compare canonical content against
    // the old engine's never-populated history, and exercise churn only in new.
    let fixture_new = fixture_new.replace("let mut history =", "#[derive(Default, Component)] struct Temporary; w.register::<Temporary>().unwrap(); w.insert(entities[1999], Temporary).unwrap(); w.remove::<Temporary>(entities[1999]); let mut history =");
    let fixture_new = fixture_new.replace("w.save()", "{ let mut b = w.save().unwrap(); b[7] = 3; b }");
    let fixture_new = fixture_new.replace("w.load(&bytes)", "w.load(&{ let mut b = bytes.clone(); b[7] = 4; b })");
    let fixture_new = fixture_new
        .replace("bin::to_vec(&Choice::Some { n:u64::MAX, values:vec![0,u32::MAX] })", "bin::to_vec(&Choice::Some { n:u64::MAX, values:vec![0,u32::MAX] }).unwrap()")
        .replace("bin::to_vec(&(-0f32,f64::NAN,Some(entities[0]),vec![1u8,2,3]))", "bin::to_vec(&(-0f32,f64::NAN,Some(entities[0]),vec![1u8,2,3])).unwrap()")
        .replace("bin::to_vec(&Value::record(vec![Value::Bool(true),Value::str(\"x\")]))", "bin::to_vec(&Value::record(vec![Value::Bool(true),Value::str(\"x\")])).unwrap()");
    let source = format!("mod old {{ {} }}\nmod new {{ {} }}\n#[test] fn cross() {{ let old=old::run(); let new=new::run(); assert_eq!(old,new); assert!(old.0.windows(2).all(|p| p[0].0 != p[1].0)); assert!(!old.1[0].is_empty()); }}", fixture.replace("ENGINE", "exact_game"), fixture_new);
    fs::write(scratch.join("src/lib.rs"), source).unwrap();
    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(scratch.join("Cargo.toml"))
        .arg("--quiet")
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("EXACT_UPDATE_TRUST", "development")
        .output()
        .unwrap();
    fs::remove_dir_all(&scratch).unwrap();
    assert!(
        output.status.success(),
        "cross-check failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
