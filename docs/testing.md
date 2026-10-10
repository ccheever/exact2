# Writing Exact's tests

How to write and change a test in this repository so that it stays trustworthy
and fast. The merge-blocking checks must run in under 60 seconds
(`rules/RULES.md`). The async lane runs everything else for every commit, so
its time counts too.

## Principles

- **Tests MUST be reliable.** Prioritize reliability over speed, although both
  are important. Optimizing a test to run faster must keep or strengthen the
  semantic correctness of the test.
- **Conscientiously identify the unit and the integrations.** Decide what a test
  covers: one function, one crate, a host over the runner, the whole app, etc.
  Test behaviors at the lowest level that exhibits them. Test an integration
  between subcomponents only where the test is about that integration.
- **Combine semantically related checks.** Combine semantically related tests to
  reduce the number of setups.
- **Deduplicate.** Delete tests that truly check the same thing.
- **Delete vacuous tests, with care.** A test that asserts nothing about this
  repository's code (a check of Foundation's `Timer`, say) or truly does not test
  anything meaningful is unnecessary. Read it first: an assertion can hide in a
  helper.
- **No external I/O unless it is the point.** External I/O is relatively slow
  and unreliable. A subprocess, a network call, a real file or a nested build
  costs time and adds ways to fail. Tests that use compiled fixtures should use a
  binary the lane has built (below).
- **Keep coverage.** A randomized test, a fuzz loop or a stress test keeps its
  seeds and iteration counts. Change compiler optimization levels only for a
  whole test suite or when the intent is to test a specific optimization level.

## Waiting

- **Wait on events rather than sleeping for a duration.** A fixed sleep is too
  long on a fast machine and too short on a slow one. Some ways to wait for
  events are:
  - Rust: a channel, a condition variable, a count to observe.
  - Swift: an expectation fulfilled by the callback, or a run-loop pass that
    returns once work has run: `RunLoop.main.run(mode: .default, before:)`.
    `RunLoop.main.run(until:)` waits out its whole slice; on a loaded Mac a
    10 ms slice takes about 80 ms.
  - Browsers: an event or a promise, or the agent's `clock settle` and virtual
    time.
- **Poll only when nothing notifies, and poll cheaply.** Every 50–100 ms, with a
  limit of tens of seconds that only stops hanging tests. A check that something
  did *not* happen should wait until the work it watches has ended (a count of
  operations in flight, a join) and then check.
- **Keep waits simple.** A wait reads as one condition and one bound. If
  observing an event needs a new hook, the hook is small and read-only (for
  example `Module::storage_in_flight`).

## Structure and isolation

- **Spread independent cases across the CPU cores.** When a test checks many
  generated inputs (random bit patterns for fuzzing against a reference
  implementation, the repository's documents), generate all the inputs first,
  in order, then check them on several threads (`num`'s `tests::par`,
  `std::thread::scope`). Generating first keeps the inputs the same however
  many threads check them. Independent fixtures (runners, kernels) each run on a
  thread of their own.
- **Each test sets up what it uses.** macOS XCTest classes run one per process
  (`host/apple/xctest.mjs`) and iOS classes one per simulator clone, so a class
  cannot rely on an earlier class. Install what it uses in `class func setUp`
  (`ExactSurfaces.install()` for a canvas) and create what it reads
  (`NSApplication.shared` before `NSApp`).
- **Mac-wide state is serial.** A class whose file uses the general pasteboard,
  app activation, the mouse location or a screen capture shares one process with
  the other such classes. A window's key status and first responder belong to
  its own process and need nothing.
- **Helpers are simple and correct under sharing.** A helper shared by tests or
  threads holds no mutable state, or holds it behind one obvious rule (a queue
  index read and advanced with no `await` between, a value drawn before the
  threads start). A browser shared by a file's tests gives each test a context
  of its own.
- **Build once, outside the test, where you can.** A test compiles a Contract
  fixture with the debug `contract` binary the lane's build step has built, run
  directly; not `cargo run --release`, and not a `cargo run` per fixture. The
  compiler's output does not depend on its build profile.

## UI tests

- **Programmatic input over gestures.** Typing text and programmatic presses are
  fast and exact. Use a gesture (a real touch, a drag, a swipe) when the gesture
  is what the test is about.
- **Observe the UI instead of waiting for it.** XCUITest's
  `waitForExistence(timeout:)` and its non-existence counterpart poll with a
  delay of about a second. Query the app's state (`tree`, `state`, an
  accessibility read) for the condition instead.
- **A settled UI is a condition.** After a transition or an animation, wait for
  the condition the product itself uses (`Agent.nativeInFlight()` on Apple, the
  agent's `clock settle`), not a hardcoded sleep timeout.

## Pixels

- **Downsample and crop.** Compare at 1x or lower, cropped to the region the
  test is about. When higher resolution is needed, like for font rasterization
  and hairline borders, render them small or crop them.
- **Capture once.** Lay several cases out in one window, capture it once and
  crop each case so that we wait only once for the window server.

## Measuring to optimize tests

- **Measure with interactive QoS.** A process started by a background service
  (an agent's session, a launchd job without `ProcessType Interactive`) can run
  at background QoS, where the kernel coalesces timers: a 10 ms sleep takes
  60–90 ms and every wait inflates. Measure from a terminal or an interactive
  launchd job.
- **Find the bottleneck first.** Time each binary and each test (`cargo test`
  binaries with `-Zunstable-options --report-time` under `RUSTC_BOOTSTRAP=1`;
  the XCTest per-case lines; `bun test`'s per-test times). A binary's wall time
  is at least its slowest test, and `cargo test`'s time is the sum over its
  binaries.
- **Sample a slow test before changing it.** `sample <pid>` shows whether the
  time is work, a lock, a timer or an idle wait. An idle wait is usually a
  missing notification, in the test or in the product.
- **Check the harness's own costs.** Xcode's Thread Performance Checker writes a
  symbolicated backtrace at every wait on a lower-priority thread, and its
  diagnostics collection can wait out a 600 s timeout; `build.mjs --test --ios`
  runs without both.
