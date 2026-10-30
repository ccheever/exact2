//! `wakelock="screen"` (LLP 1125 D11): with a shell (Android's reader) a
//! statement told when it changes and anew to a reader made again; with
//! none, one journal line.
use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const APP: &str = r#"component App
  state on = true
  state shown = true
  action flip
    on = ! on
  action hide
    shown = ! shown
  view
    column
      button press=flip testId="flip" height=30
        text "Flip"
      button press=hide testId="hide" height=30
        text "Hide"
      column display=(shown ? "flex" : "none")
        text "Playing" wakelock=(on ? "screen" : "none")
"#;

fn boot() -> Presenter<Nothing> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        Nothing,
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn tap(p: &mut Presenter<Nothing>, name: &str) {
    let kernel = p.host().kernel();
    let view = kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id;
    p.tap(view).unwrap();
    p.run_commands(|| Nothing);
}

/// The wake lock statements the shell takes now: kind 8's word each.
fn told(p: &mut Presenter<Nothing>) -> Vec<i32> {
    p.tell_shell();
    let words = p.take_requests().unwrap_or_default();
    let (mut out, mut at) = (Vec::new(), 0);
    while at < words.len() {
        let n = words[at + 2] as usize;
        if words[at] == 8 {
            out.push(words[at + 3]);
        }
        at += 3 + n;
    }
    out
}

#[test]
fn a_shell_is_told_when_it_changes_and_anew_after_a_reset() {
    let mut p = boot();
    p.serve_requests();
    assert_eq!(told(&mut p), [1]);
    assert!(
        told(&mut p).is_empty(),
        "a statement is told when it changes"
    );
    tap(&mut p, "flip");
    assert_eq!(told(&mut p), [0]);
    tap(&mut p, "flip");
    assert_eq!(told(&mut p), [1]);
    tap(&mut p, "hide");
    assert_eq!(told(&mut p), [0], "a display: none ancestor releases it");
    tap(&mut p, "hide");
    assert_eq!(told(&mut p), [1]);
    p.requests_reset();
    assert_eq!(told(&mut p), [1], "a reader made again is told anew");
}

#[test]
fn a_host_with_no_shell_journals_it_once() {
    let mut p = boot();
    for _ in 0..4 {
        tap(&mut p, "flip");
    }
    let journal: Vec<&str> = p.host().runner().journal().collect();
    let said = journal
        .iter()
        .filter(|l| l.contains("wakelock: unavailable"))
        .count();
    assert_eq!(said, 1, "{journal:?}");
    p.tell_shell();
    assert_eq!(p.take_requests(), None);
}

fn unavailable(p: &Presenter<Nothing>) -> usize {
    p.host()
        .runner()
        .journal()
        .filter(|l| l.contains("wakelock: unavailable"))
        .count()
}

/// Android's reader commits after boot (the place and time, a resize) and
/// serves its shell before its loop's first turn: nothing says the lock is
/// unavailable, and the shell is told it is on.
#[test]
fn commits_before_the_shell_is_served_say_nothing() {
    let mut p = boot();
    assert!(p.resize(400., 700.).is_none());
    p.serve_requests();
    p.run_commands(|| Nothing);
    assert_eq!(unavailable(&p), 0);
    assert_eq!(told(&mut p), [1]);
}

/// A plan replaced by one with no holder at the same kernel epoch still
/// tells the shell the lock is off.
#[test]
fn a_replacement_at_the_same_epoch_is_read_again() {
    let mut p = boot();
    p.serve_requests();
    assert_eq!(told(&mut p), [1]);
    let epoch = p.host().kernel().epoch();
    let plan = crate::test_plan("component App\n  view\n    text \"Nothing held\"\n");
    p.reload(&plan, Nothing).unwrap();
    assert_eq!(p.host().kernel().epoch(), epoch, "the case under test");
    assert_eq!(told(&mut p), [0]);
}
