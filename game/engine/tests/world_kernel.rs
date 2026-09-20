//! Both libraries are dependencies of a temporary consumer, never of the root
//! workspace or of one another; the publication adapter has separate tests.
use std::{fs, process::Command};
#[test]
fn generic_world_hash_and_data_are_identical_to_the_kernel() {
    run_cross(false);
}
#[test]
#[ignore = "comparative throughput; run explicitly in release"]
fn dense_and_sparse_page_throughput() {
    run_cross(true);
}
fn run_cross(benchmark: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let scratch = root
        .parent()
        .unwrap()
        .join("scratch")
        .join("K1e")
        .join(format!("cross-{}", std::process::id()));
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
        w.emit("delivery"); let _ = w.publish("score",batch as u32);
        history.push((w.hash(),w.save()));
        let bytes = w.save(); w.load(&bytes).unwrap(); assert_eq!(w.save(),bytes);
    }
    let data = vec![bin::to_vec(&Choice::Some { n:u64::MAX, values:vec![0,u32::MAX] }), bin::to_vec(&(-0f32,f64::NAN,Some(entities[0]),vec![1u8,2,3])),bin::to_vec(&Value::record(vec![Value::Bool(true),Value::str("x")]))];
    (history,data)
}
#[derive(Default, Component)] struct Throughput(u64, [u64;4]);
pub fn throughput(label: &str, dense: bool) {
    const N: usize = 200_000;
    const REPEAT: usize = 100;
    let mut w = World::new(60, 0);
    w.register::<Throughput>();
    let mut es: Vec<_> = (0..N).map(|_| w.spawn(())).collect();
    for i in (0..N).step_by(11) { w.despawn(es[i]); es[i] = w.spawn(()); }
    let mut expected = 0u64;
    let mut rows = 0;
    for i in (0..N).rev() {
        if dense || i % 97 == 0 {
            w.insert(es[i], Throughput(i as u64, [i as u64;4]));
            expected += i as u64;
            rows += 1;
        }
    }
    assert!(rows > 2000);
    let pages = w.pages::<Throughput>().iter().count();
    let start = std::time::Instant::now();
    for _ in 0..REPEAT {
        let sum = std::hint::black_box(&w).query::<&Throughput>().iter().map(|(_,v)|v.0).sum::<u64>();
        assert_eq!(std::hint::black_box(sum), expected);
    }
    let query = start.elapsed().as_secs_f64();
    let start = std::time::Instant::now();
    for _ in 0..REPEAT {
        let mut sum = 0u64;
        for page in std::hint::black_box(&w).pages::<Throughput>().iter() {
            for (_, values) in page.runs() { for v in values { sum += v.0; } }
        }
        assert_eq!(std::hint::black_box(sum), expected);
    }
    let runs = start.elapsed().as_secs_f64();
    println!("{label} dense={dense} slots={N} rows={rows} pages={pages} query_ns_per_row={:.3} runs_ns_per_row={:.3} query_ms={:.3} runs_ms={:.3}", query*1e9/(rows*REPEAT) as f64, runs*1e9/(rows*REPEAT) as f64, query*1000., runs*1000.);
}
"#;
    let fixture_new = fixture
        .replace(
            "Value::record(vec![Value::Bool(true),Value::str(\"x\")])",
            "Published::Record(vec![Published::Bool(true),Published::Str(\"x\".into())])",
        )
        .replace("ENGINE", "exact_world")
        .replace("w.hash()", "w.hash().unwrap()")
        .replace(
            "register::<Throughput>()",
            "register::<Throughput>().unwrap()",
        )
        .replace("w.spawn(())", "w.spawn(()).unwrap()")
        .replace(
            "Throughput(i as u64, [i as u64;4]))",
            "Throughput(i as u64, [i as u64;4])).unwrap()",
        )
        .replace("register::<Other>()", "register::<Other>().unwrap()")
        .replace("register::<Counter>()", "register::<Counter>().unwrap()")
        .replace(
            "register_resource::<Ledger>()",
            "register_resource::<Ledger>().unwrap()",
        )
        .replace("}, ())", "}, ()).unwrap()")
        .replace(
            "link: Some(entities[0]) });",
            "link: Some(entities[0]) }).unwrap();",
        )
        .replace("Other(vec![0,65535]))", "Other(vec![0,65535])).unwrap()")
        .replace(
            "insert_resource(ledger)",
            "insert_resource(ledger).unwrap()",
        )
        .replace(
            "..Counter::default() },))",
            "..Counter::default() },)).unwrap()",
        );
    // Old EXGAME v3 retained empty columns. Compare canonical content against
    // the old engine's never-populated history, and exercise churn only in new.
    let fixture_new = fixture_new.replace("let mut history =", "#[derive(Default, Component)] struct Temporary; w.register::<Temporary>().unwrap(); w.insert(entities[1999], Temporary).unwrap(); w.remove::<Temporary>(entities[1999]); let mut history =");
    let fixture_new =
        fixture_new.replace("w.save()", "{ let mut b = w.save().unwrap(); b[7] = 3; b }");
    let fixture_new = fixture_new.replace(
        "w.load(&bytes)",
        "w.load(&{ let mut b = bytes.clone(); b[7] = 4; b })",
    );
    let fixture_new = fixture_new
        .replace(
            "bin::to_vec(&Choice::Some { n:u64::MAX, values:vec![0,u32::MAX] })",
            "bin::to_vec(&Choice::Some { n:u64::MAX, values:vec![0,u32::MAX] }).unwrap()",
        )
        .replace(
            "bin::to_vec(&(-0f32,f64::NAN,Some(entities[0]),vec![1u8,2,3]))",
            "bin::to_vec(&(-0f32,f64::NAN,Some(entities[0]),vec![1u8,2,3])).unwrap()",
        )
        .replace(
            "bin::to_vec(&Published::Record(vec![Published::Bool(true),Published::Str(\"x\".into())]))",
            "bin::to_vec(&Published::Record(vec![Published::Bool(true),Published::Str(\"x\".into())])).unwrap()",
        );
    let mut source = format!("mod old {{ {} }}\nmod new {{ {} }}\n#[test] fn cross() {{ let old=old::run(); let new=new::run(); assert_eq!(old,new); assert!(old.0.windows(2).all(|p| p[0].0 != p[1].0)); assert!(!old.1[0].is_empty()); }}", fixture.replace("ENGINE", "exact_game"), fixture_new);
    if benchmark {
        source.push_str("\n#[test] fn throughput() { for _ in 0..7 { for dense in [true, false] { old::throughput(\"engine1024\", dense); new::throughput(\"kernel64\", dense); } } }");
    }
    fs::write(scratch.join("src/lib.rs"), source).unwrap();
    let mut command = Command::new("cargo");
    if benchmark {
        command.arg("test").arg("--release");
    } else {
        command.arg("test");
    }
    let output = command
        .args(["--offline", "--manifest-path"])
        .arg(scratch.join("Cargo.toml"))
        .args(if benchmark {
            vec!["throughput", "--", "--nocapture"]
        } else {
            vec!["--quiet"]
        })
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("EXACT_UPDATE_TRUST", "development")
        .output()
        .unwrap();
    if benchmark {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
    fs::remove_dir_all(&scratch).unwrap();
    assert!(
        output.status.success(),
        "cross-check failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
