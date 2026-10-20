//! The real harness screen boots against this data module (LLP 1101.002
//! P2): `terminal.contract`, compiled as the terminal host compiles it,
//! takes `session(mode)` and `animation(...)` through the shape check.

use exact_kernel::Kernel;
use exact_runner::{DataSource, Runner};
use exact_terminal::host::{Host, Mode};
use exact_terminal::vt::Vt;
use harness_data::shapes::{ContractValue, Session};
use harness_data::{Harness, Options};

#[test]
fn the_real_screen_boots_and_reads_the_session() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../terminal.contract");
    let plan = contract::compile_path_terminal(&path).unwrap_or_else(|errors| {
        panic!(
            "terminal.contract does not compile: {}",
            errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let dir = std::env::temp_dir().join(format!("exact-harness-screen-{}", std::process::id()));
    let harness = Harness::with_options(Options::offline(Some(dir.clone())));
    let mut r = Runner::boot(
        plan,
        harness,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap_or_else(|e| panic!("the screen refused to boot: {e:?}"));
    r.data_ready().unwrap();
    let session = Session::from_value(r.resource("session").expect("a session resource"))
        .expect("the session is a Session");
    assert_eq!(session.entries[0].kind, "banner");
    assert!(session.models.iter().any(|m| m.id == "mock"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_inline_burst_reaches_scrollback_in_bounded_batches() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../terminal.contract");
    let plan = contract::compile_path_terminal(&path).unwrap();
    let mut harness = Harness::with_options(Options::offline(None));
    let mut host = Host::boot(plan, harness.clone(), Mode::Inline, 100, 30).unwrap();
    host.listen(std::sync::Arc::new(|| {}));
    let start = std::time::Instant::now();
    while Session::from_value(&harness.session())
        .unwrap()
        .entries
        .iter()
        .any(|entry| entry.kind == "banner" && entry.busy)
    {
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        host.announced();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    harness
        .query("submit", &[exact_plan::Value::str("/stress 95")])
        .unwrap();
    host.announced();
    let mut frames = 0;
    while !Session::from_value(&harness.session())
        .unwrap()
        .entries
        .is_empty()
    {
        assert!(host.transcript().len() <= 32, "mounted the whole burst");
        vt.render(&mut host);
        let due = host.timer_due_ms().expect("printed entries can retire");
        host.tick(due);
        host.announced();
        frames += 1;
        assert!(frames < 20, "the burst stopped draining");
    }
    vt.render(&mut host);
    assert!(frames >= 3, "the burst used more than one mounted batch");
    let text = vt.text(true);
    let mut cursor = 0;
    for i in 0..95 {
        let marker = if i % 9 == 4 {
            format!("Bash(seq {i})")
        } else if i % 11 == 0 {
            format!("Prompt number {i}\n")
        } else {
            format!("#{i} ")
        };
        assert_eq!(
            text.matches(&marker).count(),
            1,
            "{marker}: missing or repeated"
        );
        cursor += text[cursor..].find(&marker).expect("entries stay in order") + marker.len();
    }
    assert_eq!(vt.unanswered(), 0);
}
