//! The deadline: one per runtime, held at every entrance.
//!
//! Its own file for the reason `hermes_tests.rs` is: the 1,500-line cap in
//! `rules/RULES.md`. Every test here is wall-clock — the deadline is one —
//! so the windows are wide enough to hold under a loaded machine and narrow
//! enough to tell a stopped program from one that ran to its end. Where a
//! property has a sharper witness than the clock — an `Err` where a fresh
//! budget would have given an `Ok`, a cycle that ran where a leaked drive
//! flag would have refused it forever — the test leans on that instead.

use super::*;
use crate::stdlib::console::Level;
use std::time::{Duration, Instant};

fn runtime() -> Hermes {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib());
    rt.install_bindings().expect("bindings");
    // The console buffer is per-thread and the test harness reuses threads.
    let _ = rt.drain_console();
    rt
}

/// A program that busy-waits `millis` on the monotonic clock — the clock the
/// deadline is on — and then returns.
fn spin(millis: u64) -> String {
    format!(
        "(function () {{ const end = performance.now() + {millis}; \
         while (performance.now() < end) {{}} return 'spun'; }})()"
    )
}

/// Pump until the runtime reports its deadline, or fail if it never does.
fn pump_until_stopped(rt: &mut Hermes, started: Instant) -> (JsError, i32) {
    let mut cycles = 0;
    loop {
        match rt.pump() {
            Ok(ran) => cycles += ran,
            Err(stop) => return (stop, cycles),
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the pump was never stopped"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Pump until one cycle runs a task, for a bounded while. `Ok(0)` after the
/// bound is what a drive flag left set looks like: every cycle refused as
/// nested, forever.
fn pump_one_task(rt: &mut Hermes) -> Result<i32, JsError> {
    let started = Instant::now();
    loop {
        match rt.pump() {
            Ok(0) if started.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(1));
            }
            outcome => return outcome,
        }
    }
}

/// A compiler for check-free bytecode, in a cache of its own so two tests
/// compiling at once do not share one.
fn compiler(name: &str) -> (crate::bytecode::Compiler, std::path::PathBuf) {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cache = std::env::temp_dir().join(format!("ibex2-deadline-{name}-{}", std::process::id()));
    let compiler = crate::bytecode::Compiler::discover(&repo_root, cache.clone())
        .expect("the hermesc this build compiled its bindings with");
    (compiler, cache)
}

/// A project root holding one entry module, removed with the test. Loaded
/// without a compiler, so the module runs as evaluated source — the form
/// that carries the break checks.
#[cfg(feature = "loader")]
struct Project(std::path::PathBuf);

#[cfg(feature = "loader")]
impl Project {
    fn with_index(name: &str, source: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ibex2-deadline-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("project dir");
        std::fs::write(dir.join("index.js"), source).expect("index.js");
        Self(dir)
    }

    fn load_into(&self, rt: &mut Hermes) {
        rt.set_loader(
            crate::loader::Root::Declared(self.0.clone()),
            crate::loader::ModuleGrants::none(),
        )
        .expect("loader");
    }
}

#[cfg(feature = "loader")]
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The ticket's first case: `while (true) {}` in a host evaluation.
#[test]
fn a_synchronous_loop_is_stopped_at_the_deadline() {
    let mut rt = runtime();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    assert_eq!(rt.eval("while (true) {}"), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "stopped late: {elapsed:?}"
    );
    // Reusable once the deadline is cleared.
    rt.clear_deadline();
    assert_eq!(rt.eval("1 + 1").unwrap(), "2");
}

/// The stop is not JavaScript's to catch: the engine's timeout is raised as
/// an uncatchable error, so a `catch` around the loop sees nothing and a
/// `finally` does not turn the outcome into anything but the deadline.
#[test]
fn the_stop_cannot_be_caught_by_javascript() {
    let mut rt = runtime();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    assert_eq!(
        rt.eval(
            "try { while (true) {} } catch (e) { globalThis.caught = e; } \
             finally { globalThis.finalized = true; }"
        ),
        Err(JsError::Deadline)
    );
    assert!(
        started.elapsed() >= Duration::from_millis(150),
        "stopped early"
    );
    rt.clear_deadline();
    assert_eq!(
        rt.eval("typeof globalThis.caught").unwrap(),
        "undefined",
        "JavaScript caught the stop"
    );
}

/// The ticket's second case: a microtask that schedules itself forever.
///
/// The engine's check sits at a function's return, after the callback has
/// scheduled its successor — so the stop pauses the chain rather than
/// killing it, and a drain with no deadline would resume it. What the
/// deadline promises is that no drain outlives it: a second deadline stops
/// the same chain again, and the runtime is still there for a fresh
/// evaluation. Whether to keep such a runtime is the consumer's call;
/// Snapback 2 discards one whose deadline fired.
#[test]
fn a_recursive_microtask_chain_is_stopped_at_the_deadline() {
    let mut rt = runtime();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    rt.eval(
        "globalThis.steps = 0; \
         function again() { steps++; Promise.resolve().then(again); } again();",
    )
    .unwrap();
    assert_eq!(rt.drain_microtasks(), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "stopped late: {elapsed:?}"
    );

    let rearmed = Instant::now();
    rt.set_deadline(rearmed + Duration::from_millis(100));
    assert_eq!(rt.drain_microtasks(), Err(JsError::Deadline));
    let elapsed = rearmed.elapsed();
    assert!(
        elapsed >= Duration::from_millis(100),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "stopped late: {elapsed:?}"
    );

    rt.clear_deadline();
    assert_eq!(
        rt.eval("String(steps > 0)").unwrap(),
        "true",
        "the chain never ran"
    );
}

/// The same chain through `queueMicrotask`, whose binding wraps the callback
/// in a `try`/`catch` of its own: the engine's stop passes through that too.
#[test]
fn a_queue_microtask_chain_is_stopped_at_the_deadline() {
    let mut rt = runtime();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    rt.eval(
        "globalThis.steps = 0; \
         function again() { steps++; queueMicrotask(again); } again();",
    )
    .unwrap();
    assert_eq!(rt.drain_microtasks(), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "stopped late: {elapsed:?}"
    );
    rt.clear_deadline();
    assert_eq!(
        rt.eval("String(steps > 0)").unwrap(),
        "true",
        "the chain never ran"
    );
    let errors = rt
        .drain_console()
        .into_iter()
        .filter(|record| record.level == Level::Error)
        .count();
    assert_eq!(errors, 0, "the binding's catch saw the stop");
}

/// A timer callback that never returns is stopped inside the pump, and the
/// stopped cycle releases the drive flag: the next cycle runs rather than
/// being refused as nested. A leaked flag would refuse every cycle after
/// it, so a later task that runs at all is the witness.
#[test]
fn a_timer_callback_is_stopped_at_the_deadline_and_the_drive_is_released() {
    let mut rt = runtime();
    rt.eval(
        "globalThis.entered = false; setTimeout(() => { entered = true; while (true) {} }, 1);",
    )
    .unwrap();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    let (stop, _) = pump_until_stopped(&mut rt, started);
    assert_eq!(stop, JsError::Deadline);
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "stopped early: {elapsed:?}"
    );
    rt.clear_deadline();
    assert_eq!(
        rt.eval("String(entered)").unwrap(),
        "true",
        "the callback never ran"
    );
    rt.eval("globalThis.again = false; setTimeout(() => { again = true; }, 1);")
        .unwrap();
    assert_eq!(pump_one_task(&mut rt), Ok(1), "the drive flag was left set");
    assert_eq!(rt.eval("String(again)").unwrap(), "true");
}

/// A chain of short timers, each rescheduling the next, is ended at the
/// door: the deadline passes between cycles and the next cycle is refused
/// without running anything.
#[test]
fn a_self_rescheduling_timer_chain_is_refused_at_the_deadline() {
    let mut rt = runtime();
    rt.eval("globalThis.ticks = 0; function tick() { ticks++; setTimeout(tick, 0); } tick();")
        .unwrap();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    let (stop, cycles) = pump_until_stopped(&mut rt, started);
    assert_eq!(stop, JsError::Deadline);
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(400),
        "the refusal waited: {elapsed:?}"
    );
    assert!(cycles > 0, "the chain never ran before the deadline");
    rt.clear_deadline();
    assert_eq!(rt.eval("String(ticks > 0)").unwrap(), "true");
}

/// A timer task that fails natively — a host function it calls throws a
/// `std::runtime_error` — is a console error like a task that throws in
/// JavaScript: `pump` reports the cycle ran, and the drive flag is released
/// so the next task runs. (The pin translates a host function's native
/// exception into a JavaScript error before the shim sees it; the shim's
/// own `std::exception` arm is there for what the engine throws natively
/// out of a call, and nothing unwinds through the C ABI either way.)
#[test]
fn a_timer_task_that_fails_natively_is_a_console_error_and_the_drive_is_released() {
    let mut rt = runtime();
    assert!(rt.install_throwing_probe("__ibex2_native_probe", true));
    rt.eval("setTimeout(() => { __ibex2_native_probe(); }, 0);")
        .unwrap();
    assert_eq!(pump_one_task(&mut rt), Ok(1));
    let records = rt.drain_console();
    assert!(
        records
            .iter()
            .any(|r| r.level == Level::Error && r.message.contains("__ibex2_native_probe threw")),
        "the failure was not reported: {records:?}"
    );
    rt.eval("globalThis.again = false; setTimeout(() => { again = true; }, 0);")
        .unwrap();
    assert_eq!(pump_one_task(&mut rt), Ok(1), "the drive flag was left set");
    assert_eq!(rt.eval("String(again)").unwrap(), "true");
}

/// A timer task that throws under a deadline that has not fired is still a
/// console error, never a status: the deadline changes nothing about a task
/// that fails for its own reasons.
#[test]
fn a_throwing_timer_task_under_an_unfired_deadline_is_a_console_error() {
    let mut rt = runtime();
    rt.set_deadline(Instant::now() + Duration::from_secs(10));
    rt.eval("setTimeout(() => { throw new Error('sentinel'); }, 0);")
        .unwrap();
    assert_eq!(pump_one_task(&mut rt), Ok(1));
    let records = rt.drain_console();
    assert!(
        records
            .iter()
            .any(|r| r.level == Level::Error && r.message.contains("sentinel")),
        "the throw was not reported: {records:?}"
    );
    rt.clear_deadline();
}

/// A host function that throws from inside a microtask, with no deadline
/// armed, is the promise's to settle — the checkpoint neither sees it nor
/// retries it, so a pump and a drain both return at once.
#[test]
fn a_microtask_host_function_that_throws_does_not_livelock_the_pump() {
    let mut rt = runtime();
    assert!(rt.install_throwing_probe("__ibex2_js_probe", false));
    rt.eval(
        "globalThis.settled = 'pending'; \
         Promise.resolve().then(() => __ibex2_js_probe()).catch((e) => { settled = e.message; });",
    )
    .unwrap();
    let started = Instant::now();
    assert_eq!(rt.pump(), Ok(0));
    assert_eq!(rt.eval("settled").unwrap(), "__ibex2_js_probe threw");
    rt.eval("Promise.resolve().then(() => __ibex2_js_probe());")
        .unwrap();
    rt.drain_microtasks().unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the checkpoint retried"
    );
}

/// The control: a program that behaves completes under the same deadline,
/// through every entrance, with the deadline nowhere in its results.
#[test]
fn a_well_behaved_program_completes_under_a_deadline() {
    let mut rt = runtime();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_secs(10));
    assert_eq!(rt.eval("40 + 2").unwrap(), "42");
    rt.eval("globalThis.n = 0; function step() { if (++n < 1000) Promise.resolve().then(step); } step();")
        .unwrap();
    rt.drain_microtasks().unwrap();
    assert_eq!(rt.eval("String(n)").unwrap(), "1000");
    rt.eval("globalThis.fired = false; setTimeout(() => { fired = true; }, 1);")
        .unwrap();
    while rt.eval("String(fired)").unwrap() != "true" {
        rt.pump().unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the timer never fired"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    rt.clear_deadline();
    assert!(started.elapsed() < Duration::from_secs(10));
}

/// The deadline is one point in time, not a budget renewed at each
/// entrance: three spins that would each fit are stopped where the deadline
/// falls. The witness is the `Err` — a budget renewed for the third
/// entrance would have let it return `spun` — and the clock only says the
/// stop was neither early nor absurdly late.
#[test]
fn the_remaining_time_is_what_is_left_of_the_same_deadline() {
    let mut rt = runtime();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(600));
    assert_eq!(rt.eval(&spin(250)).unwrap(), "spun");
    assert_eq!(rt.eval(&spin(250)).unwrap(), "spun");
    assert_eq!(
        rt.eval(&spin(250)),
        Err(JsError::Deadline),
        "the third entrance was given a fresh budget"
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(600),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "stopped late: {elapsed:?}"
    );
}

/// Arming again replaces the deadline: the later, nearer one is what holds
/// the next entrance, not the ten seconds first armed.
#[test]
fn a_deadline_armed_again_replaces_the_one_before_it() {
    let mut rt = runtime();
    rt.set_deadline(Instant::now() + Duration::from_secs(10));
    assert_eq!(rt.eval("1").unwrap(), "1");
    let replaced = Instant::now();
    rt.set_deadline(replaced + Duration::from_millis(50));
    assert_eq!(rt.eval("while (true) {}"), Err(JsError::Deadline));
    let elapsed = replaced.elapsed();
    assert!(
        elapsed >= Duration::from_millis(50),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "the first deadline held: {elapsed:?}"
    );
}

/// Clearing a deadline that was never armed is nothing, twice over.
#[test]
fn clearing_a_deadline_that_was_never_armed_is_harmless() {
    let mut rt = runtime();
    rt.clear_deadline();
    rt.clear_deadline();
    assert_eq!(rt.eval("1 + 1").unwrap(), "2");
}

/// The deadline is one runtime's own: armed on one, it neither stops nor
/// refuses another in the same process, before or after it fires.
#[test]
fn the_deadline_is_one_runtime_s_own() {
    let mut held = runtime();
    let mut free = runtime();
    held.set_deadline(Instant::now() + Duration::from_millis(50));
    assert_eq!(free.eval("40 + 2").unwrap(), "42");
    assert_eq!(held.eval("while (true) {}"), Err(JsError::Deadline));
    assert_eq!(free.eval(&spin(100)).unwrap(), "spun");
    assert_eq!(held.eval("40 + 2"), Err(JsError::Deadline));
}

/// Past the deadline an entrance is refused at the door — immediately, and
/// without running what it was given.
#[test]
fn a_deadline_already_passed_refuses_the_entrance_without_running_it() {
    let mut rt = runtime();
    rt.set_deadline(Instant::now());
    std::thread::sleep(Duration::from_millis(2));
    let started = Instant::now();
    assert_eq!(rt.eval("globalThis.ran = true"), Err(JsError::Deadline));
    assert_eq!(rt.drain_microtasks(), Err(JsError::Deadline));
    assert_eq!(rt.pump(), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(100),
        "a refusal waited: {elapsed:?}"
    );
    rt.clear_deadline();
    assert_eq!(rt.eval("String(globalThis.ran)").unwrap(), "undefined");
}

/// The same door for the two entrances that take prepared code: bytecode
/// and an entry module are refused past the deadline without running, and
/// run once it is cleared — so the refusal was the deadline's, not the
/// fixtures'.
#[cfg(feature = "loader")]
#[test]
fn a_deadline_already_passed_refuses_bytecode_and_an_entry_module_at_the_door() {
    let (compiler, cache) = compiler("door");
    let bytes = compiler
        .compile("globalThis.ran = 'bytecode';")
        .expect("compile");
    let project = Project::with_index("door", "globalThis.ran = 'module';");
    let mut rt = runtime();
    project.load_into(&mut rt);
    rt.set_deadline(Instant::now());
    std::thread::sleep(Duration::from_millis(2));
    let started = Instant::now();
    assert_eq!(rt.eval_bytes(&bytes), Err(JsError::Deadline));
    assert_eq!(rt.run_entry("./index.js"), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(100),
        "a refusal waited: {elapsed:?}"
    );
    rt.clear_deadline();
    assert_eq!(rt.eval("String(globalThis.ran)").unwrap(), "undefined");
    rt.eval_bytes(&bytes).unwrap();
    assert_eq!(rt.eval("ran").unwrap(), "bytecode");
    rt.run_entry("./index.js").unwrap();
    assert_eq!(rt.eval("ran").unwrap(), "module");
    let _ = std::fs::remove_dir_all(cache);
}

/// An entry module that never returns, loaded as source through the
/// loader, is stopped near the deadline like any evaluated source.
#[cfg(feature = "loader")]
#[test]
fn a_spinning_entry_module_is_stopped_at_the_deadline() {
    let project = Project::with_index("spin", "globalThis.entered = true; while (true) {}");
    let mut rt = runtime();
    project.load_into(&mut rt);
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    assert_eq!(rt.run_entry("./index.js"), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "stopped late: {elapsed:?}"
    );
    rt.clear_deadline();
    assert_eq!(rt.eval("String(entered)").unwrap(), "true");
}

/// The microtasks an entry module queues are drained through the same
/// checkpoint as `pump` and `drain_microtasks`, and so held to the same
/// deadline: a module whose body returns but whose chain never ends is
/// stopped in the drain, after the body ran.
#[cfg(feature = "loader")]
#[test]
fn an_entry_module_s_microtask_chain_is_stopped_at_the_deadline() {
    let project = Project::with_index(
        "chain",
        "globalThis.steps = 0; \
         function again() { steps++; Promise.resolve().then(again); } again();",
    );
    let mut rt = runtime();
    project.load_into(&mut rt);
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(150));
    assert_eq!(rt.run_entry("./index.js"), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "stopped early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "stopped late: {elapsed:?}"
    );
    rt.clear_deadline();
    assert_eq!(
        rt.eval("String(steps > 0)").unwrap(),
        "true",
        "the body's chain never ran"
    );
}

/// The helpers that block between entrances never wait past the armed
/// deadline: a timer an hour away is no reason for `pump_until` to sit out
/// its own thirty seconds once the deadline has passed.
#[test]
fn pump_until_returns_when_the_armed_deadline_passes() {
    let mut rt = runtime();
    rt.eval("setTimeout(() => {}, 3600000);").unwrap();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(100));
    assert_eq!(rt.pump_until(1), 0);
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(100),
        "returned before the deadline: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "waited out the helper's own budget: {elapsed:?}"
    );
}

/// The same for `run_to_quiescence`, whose wait would otherwise be the
/// lesser of the far timer and its thirty-second budget.
#[test]
fn run_to_quiescence_returns_when_the_armed_deadline_passes() {
    let mut rt = runtime();
    rt.eval("setTimeout(() => {}, 3600000);").unwrap();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(100));
    rt.run_to_quiescence(Duration::from_secs(30));
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(100),
        "returned before the deadline: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "waited out the helper's own budget: {elapsed:?}"
    );
}

/// The interruption is a kind, not a string: a throw that forges the
/// engine's own timeout text is `Thrown`, and the real stop needs no text.
#[test]
fn the_interruption_is_reported_by_kind_not_by_message_text() {
    let mut rt = runtime();
    rt.set_deadline(Instant::now() + Duration::from_secs(10));
    let forged = rt
        .eval("throw new Error('Javascript execution has timed out.')")
        .unwrap_err();
    assert!(
        matches!(forged, JsError::Thrown(ref text) if text.contains("timed out")),
        "{forged:?}"
    );
    rt.set_deadline(Instant::now() + Duration::from_millis(50));
    assert_eq!(rt.eval("while (true) {}"), Err(JsError::Deadline));
}

/// Bytecode carries only the checks it was compiled with, and hermesc emits
/// none by default: such a program is not stopped mid-way, but it is held
/// to the deadline on the way out, and the request the monitor left pending
/// while it ran is flushed rather than thrown at the next entrance.
#[test]
fn uncheckable_bytecode_is_reported_at_exit_and_its_pending_break_is_flushed() {
    let (compiler, cache) = compiler("flush");
    let bytes = compiler.compile(&spin(200)).expect("compile");
    let mut rt = runtime();
    let started = Instant::now();
    rt.set_deadline(started + Duration::from_millis(50));
    assert_eq!(rt.eval_bytes(&bytes), Err(JsError::Deadline));
    let elapsed = started.elapsed();
    // Its own end, give or take the rounding of the clock it spun on —
    // against a 50 ms deadline.
    assert!(
        elapsed >= Duration::from_millis(190),
        "check-free bytecode was stopped mid-way: {elapsed:?}"
    );
    // Without the flush this would throw the engine's timeout for a
    // deadline that is no longer armed.
    rt.clear_deadline();
    assert_eq!(rt.eval("1 + 1").unwrap(), "2");
    let _ = std::fs::remove_dir_all(cache);
}

/// Teardown with a deadline armed, and with one that fired, is clean.
#[test]
fn a_runtime_is_dropped_cleanly_with_its_deadline_armed_or_fired() {
    let mut armed = runtime();
    armed.set_deadline(Instant::now() + Duration::from_secs(10));
    drop(armed);
    let mut fired = runtime();
    fired.set_deadline(Instant::now() + Duration::from_millis(50));
    assert_eq!(fired.eval("while (true) {}"), Err(JsError::Deadline));
    drop(fired);
}
