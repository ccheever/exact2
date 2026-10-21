# Round 1
Found three defects in `4c0109074` (HEAD advanced during review):

1. **P2 — Swift compiler tests remain in the gate.** [development.rs:156](/Users/admin/projects/exact2-wt-gate/host/apple/tests/it/development.rs:156) and [development.rs:245](/Users/admin/projects/exact2-wt-gate/host/apple/tests/it/development.rs:245) run `swift -e` without an ignore annotation. Adding `host/apple` therefore adds Apple toolchain execution to normal Cargo tests. Using the existing test binary, I reproduced the latter failing when `swift` was absent from PATH. This also contradicts the documentation’s “in-process” characterization and implication that Swift testing stays in XCTests.

2. **P2 — A remaining stream test has a scheduling race.** [executor_stream_tests.rs:232](/Users/admin/projects/exact2-wt-gate/host/apple/src/executor_stream_tests.rs:232) sleeps twice for 50 ms, then requires message `"6"` with five coalesced predecessors. Nothing synchronizes consumption of the sixth message; a delayed worker lets `next()` return an earlier message and fail correct code. Replace the sleeps with synchronization or move this test async. Other explicit wall-clock assertions remain at [executor_timeout_tests.rs:29](/Users/admin/projects/exact2-wt-gate/host/apple/src/executor_timeout_tests.rs:29) and [executor_tests.rs:372](/Users/admin/projects/exact2-wt-gate/host/apple/src/executor_tests.rs:372).

3. **P2 — An unguarded integration test breaks on Windows/MSVC.** [inherited.rs:387](/Users/admin/projects/exact2-wt-gate/host/apple/tests/it/inherited.rs:387) builds native static libraries, then selects only filenames ending in `.a`. MSVC emits `.lib`—confirmed from rustc’s target specification—so `library` becomes undefined and the following `readFileSync` fails. This test now runs in the default gate.

The seven socket tests themselves are correctly selected: executing the async lane’s grep/regex and passing its filters to the existing Apple binary listed all seven ignored tests. Linux inherits the annotations through `#[path]`; the workspace async invocation selects both copies.

I found no additional real-network or simulator tests, or a Linux compilation blocker in the inspected source. Apple framework compilation is platform-gated. Fresh builds and cross-platform execution were not possible in the read-only environment. No files modified.
# Round 2
Two remaining defects:

- **[P2] Wall-clock assertions still run in the gate.** [executor_timeout_tests.rs:29](/Users/admin/projects/exact2-wt-gate/host/apple/src/executor_timeout_tests.rs:29) requires completion within three seconds; [executor_tests.rs:372](/Users/admin/projects/exact2-wt-gate/host/apple/src/executor_tests.rs:372) requires retirement within one second. Neither is ignored. The timeout test passed normally but failed after a controlled 3.2-second scheduling pause. This contradicts the documented exclusion of wall-clock tests.

- **[P2] Seven unignored tests still launch subprocesses.** They re-execute the test binary through `Command::new(current_exe())`: [abi_tests.rs:139](/Users/admin/projects/exact2-wt-gate/host/apple/src/abi_tests.rs:139), [abi_tests.rs:1037](/Users/admin/projects/exact2-wt-gate/host/apple/src/abi_tests.rs:1037), [collection_tests.rs:107](/Users/admin/projects/exact2-wt-gate/host/apple/src/collection_tests.rs:107), [executor_order_tests.rs:66](/Users/admin/projects/exact2-wt-gate/host/apple/src/executor_order_tests.rs:66), [storage_tests.rs:46](/Users/admin/projects/exact2-wt-gate/host/apple/src/storage_tests.rs:46), [storage_tests.rs:123](/Users/admin/projects/exact2-wt-gate/host/apple/src/storage_tests.rs:123), and the inline test at [store.rs:495](/Users/admin/projects/exact2-wt-gate/host/apple/src/store.rs:495). Consequently, the “in-process” description in [reference.md:907](/Users/admin/projects/exact2-wt-gate/docs/reference.md:907) remains inaccurate.

No additional unignored socket-binding or Apple-tool launches found. Verification used the existing test binary; no files modified.
# Disposition

Round 1: the wall-clock coalescing test and the six Bun/Swift/nested-cargo tests moved to the async lane; the Cargo.toml comment updated. The MSVC '.a' filename (inherited.rs) is moot on the gate, which runs on macOS, and that test is now async lane. Round 2: the fixture's hold is a minute, so the two remaining bounds (3 s, 1 s) are 30 s and still tell an aborted request from a held one; the reference says a test may re-run its own binary. Not re-reviewed after round 2.
