//! The compiler's time grows with the plan, not its square: a root holding
//! four times the uses of one component compiles in about four times as long.
//! Each per-use pass that answered a question by scanning or copying every
//! declaration made that longer: such scans once took 8,000 uses to ten
//! seconds, and main measured 17.1x here before they went. Put back one at a
//! time, each of the thirteen takes this test's ratio from 3.9-4.3 to 4.8-16.
// Thread CPU time is read through `libc`, a dependency on unix only.
#![cfg(unix)]
use std::path::Path;
use std::time::Duration;

/// A component's states, actions and buttons, one of each per `j`.
const EACH: usize = 16;
/// Root states per use, each with an initializer that reads another state.
const ROOTS: usize = 4;

/// `uses` uses of a component with a prop and [`EACH`] states, actions and
/// buttons pressing them, each action calling one more whose parameter the
/// calls type, inlined into the root, beside [`ROOTS`] root states a use. The
/// buttons are not the platform's own, so the warning pass reads their faces'
/// rows at every use. Every per-use question the compiler asks (a slot's type
/// after a body or a call, a name in scope, a state or action by name, a
/// button's face) is asked many times a use, so a scan for any of them shows.
fn app(uses: usize) -> String {
    let mut source = String::from("component App\n  state first = 0\n");
    for i in 0..uses * ROOTS {
        source.push_str(&format!("  state s{i} = first\n"));
    }
    source.push_str("  view\n    column\n");
    for i in 0..uses {
        source.push_str(&format!("      Card(label=\"c{i}\")\n"));
    }
    source.push_str("\ncomponent Card\n  props\n    label: string\n");
    for j in 0..EACH {
        source.push_str(&format!("  state n{j} = 0\n"));
    }
    source.push_str("  state total = 0\n  action add(by)\n    total = total + by\n");
    for j in 0..EACH {
        source.push_str(&format!(
            "  action bump{j}\n    n{j} = n{j} + 1\n    add(1)\n"
        ));
    }
    source.push_str("  view\n    column\n      text label\n");
    for j in 0..EACH {
        source.push_str(&format!(
            "      button press=bump{j} appearance=\"none\"\n        text `${{n{j}}}`\n"
        ));
    }
    source
}

/// This thread's CPU time: what a busy machine's other work does not add to.
fn cpu() -> Duration {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid timespec for the call to fill.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) },
        0
    );
    Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
}

/// One build as `contract build` makes it (mapped, warnings collected): its
/// CPU time, and its plan's bytes.
fn timed(path: &Path) -> (Duration, Vec<u8>) {
    let start = cpu();
    let (compiled, _warnings) = contract::compile_path_all_warned(path, true);
    let took = cpu() - start;
    (took, compiled.unwrap().0.encode())
}

#[test]
fn four_times_the_uses_compile_in_about_four_times_as_long() {
    let uses = 250;
    let dir = std::env::temp_dir().join(format!("contract-growth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (small_app, large_app) = (dir.join("small.contract"), dir.join("large.contract"));
    std::fs::write(&small_app, app(uses)).unwrap();
    std::fs::write(&large_app, app(uses * 4)).unwrap();
    // The least CPU time of three of each, taken in turns.
    let (mut small, mut large) = (Duration::MAX, Duration::MAX);
    let (mut small_bytes, mut large_bytes) = (None, None);
    for _ in 0..3 {
        for (app, best, bytes) in [
            (&small_app, &mut small, &mut small_bytes),
            (&large_app, &mut large, &mut large_bytes),
        ] {
            let (took, plan) = timed(app);
            *best = (*best).min(took);
            assert!(
                bytes.as_ref().is_none_or(|b| *b == plan),
                "the same source compiled to different bytes"
            );
            *bytes = Some(plan);
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
    let ratio = large.as_secs_f64() / small.as_secs_f64();
    // Linear measures 3.9-4.3 (debug and host-dev); the least of the scans
    // put back, a handler's action found by `position`, measures 4.8-5.2.
    assert!(
        ratio <= 4.6,
        "{uses} uses took {small:?} and {} took {large:?}: {ratio:.1}x for 4x the uses",
        uses * 4
    );
}
