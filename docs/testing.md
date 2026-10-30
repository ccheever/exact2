# Writing Exact's tests

How to write and change a test in this repository so that it stays trustworthy
and fast. The merge-blocking checks must run in under 65 seconds
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

## Whose failure it is, and what has run

- **`bun scripts/known.mjs -- <command>`** runs `cargo test …`, `bun test …` or
  `bun host/apple/build.mjs --test …` and, when it fails, sorts each failing
  test against the async lane's record of origin/main (LLP 1146 D1): **known**
  (it fails there too, with how often it failed in the lane's last ten runs),
  **new** (the lane ran it there and it passed), or **unknown** (no lane run
  there covers it). The exit status stays the command's. The lane attaches what
  each commit's checks found to that commit as a git note
  (`refs/notes/async`, tier 2's `async2`), which it fetches from origin.
- **`bun scripts/evidence.mjs --app <name>`** lists the drives, test runs and
  `verify` runs recorded against the app's builds (LLP 1146 D2,
  `<app>/.exact/runs.jsonl`): current while the build they ran on is still the
  host's and still fresh, else stale, saying what changed. It names the hosts
  the branch changed that no current run covers; `--draft` prints lines for a
  diary's Checkpoints or a PR's Test Plan. `evidence.mjs note <host> "<what>"
  [--device "<model, OS>"]` records something seen by hand. Web, macOS and
  iOS-simulator builds are identified; other hosts' runs are listed as
  unchecked.

## Waiting

- **Wait on events rather than sleeping for a duration.** A fixed sleep is too
  long on a fast machine and too short on a slow one. Some ways to wait for
  events are:
  - Rust: a channel, a condition variable, a count to observe.
  - The TypeScript executor (`exact-js`): `Module::set_turn_step` times each
    turn as one step of a clock that moves at no other time;
    `exact_render::expire_renders` brings render deadlines to now;
    `Module::hold_compressions` holds compressions until released, and a
    storage wait with only held work in flight runs out at once.
  - Swift: an expectation fulfilled by the callback, or a run-loop pass that
    returns once work has run: `RunLoop.main.run(mode: .default, before:)`.
    `RunLoop.main.run(until:)` waits out its whole slice; on a loaded Mac a
    10 ms slice takes about 80 ms.
  - Browsers: an event or a promise, or the agent's `clock settle` and virtual
    time.
  - A drive or an authored test: `clock settle` or `clock data` for what the
    app has in flight. A stream counts as landed at its first message (LLP
    1016.000 D5), so for state a protocol reaches later (a socket's READY after
    its HELLO) write `clock until state <path> == <literal> within <ms>`, not
    `clock +N real`: the clock stays frozen while it polls `state` on real
    time, and a miss fails naming the last value. A task due a commit later
    (`after(1, …)`) needs a `clock settle` before it.
- **Poll only when nothing notifies, and poll cheaply.** Every 50–100 ms, with a
  limit of tens of seconds that only stops hanging tests. A shorter interval can
  be considered for state that is cheap to read in the test's own process, when
  the poll significantly affects the test's speed. In a browser,
  `setTimeout(fn, 0)` is already throttled and coalesced: HTML clamps it to at
  least 4 ms once nested five deep, and browsers slow and batch the timers of a
  hidden page. A check that something did *not* happen should wait until the
  work it watches has ended (a count of operations in flight, a join) and then
  check.
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
- **A Bun file that mocks a module runs in a global of its own.** Bun's
  `mock.module` replaces a module for the whole process and does not relink a
  module already loaded, so files that mock or import the same module
  (`host/web-js/rt.js`) break each other in one process: a mock that does not
  take can leave a test spinning on the real module, past any test timeout.
  Run several such files with `bun test --isolate` (a fresh global and module
  registry per file) or `--parallel`, which implies it and is how the async
  lane's glue step runs them.
- **Platform availability belongs in Swift.** The iOS run includes every test
  compiled for its destination, including shared classes without an `IOSTests`
  suffix. Guard platform-specific assertions with `#if os(...)`.
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
- **An app's Rust tests are its data crate's.** An app's logic and its Rust tests
  live in `apps/<app>/data`; its `web`, `linux`, `apple` and `android` crates are
  host shells, tested mainly by driving the app (conformance, the smokes,
  `scripts/agent.mjs`). A workspace package without tests sets `test = false`
  and `bench = false` on its library and each binary target, so `cargo test
  --tests` and `cargo build --all-targets` build no empty test binary for it;
  remove both settings when adding the package's first test. `bun scripts/caps.mjs` checks this both ways from
  a scan of the package's sources, so a test that a macro or a generated file
  produces needs a reviewer's eye.

For an iOS simulator drive without a GUI, set `EXACT_SHOW_SIMULATOR=0`. The simulator
boots and runs the app, but the driver opens neither Simulator nor Device Hub.
Use a simulator you created when another person shares the Mac.

## UI tests

- **Programmatic input over gestures.** Typing text and programmatic presses are
  fast and exact. Use a gesture (a real touch, a drag, a swipe) when the gesture
  is what the test is about.
- **Observe the UI instead of waiting for it.** XCUITest's
  `waitForExistence(timeout:)` and its non-existence counterpart poll with a
  delay of about a second. Query the app's state (`tree`, `state`, an
  accessibility read) for the condition instead.
- **Safe-area tests fill the screen.** Size a UIKit test window to its screen's
  bounds and boot the session at that size. A fixed phone size may stop above
  another simulator's home indicator and correctly receive no bottom inset.
- **A settled UI is a condition.** After a transition or an animation, wait for
  the condition the product itself uses (`Agent.nativeInFlight()` on Apple, the
  agent's `clock settle`), not a hardcoded sleep timeout.

A real drag with `during` ops fails when the touch runner finishes before a
dispatch-log read sees the touch begin. The failure carries the aim, the injection
result and the log, and says whether the touch began and lifted. The log's `t` and
`dispatched` are seconds on the mach clock of `UITouch.timestamp`; the injection's
`end` is wall-clock milliseconds, so do not subtract one from the other. A dispatch-log
read, operation or clock step still pending a second after the runner finishes stops
the app's carrier, and the failure names it.

With an iOS session using `touch: 'platform'`, `await session.back({ native: true })`
locates UIKit's Back button through XCTest inside the active navigation container
and taps it. Embedded sessions target their own container. The driver refuses missing
or ambiguous containers or buttons, so it cannot choose another session's Back.
Settle the session and assert its resulting route or screen.
The ordinary `back()` invokes the platform's handler;
it does not establish touch delivery. XCTest can wait a minute for the app and another
minute for SpringBoard to idle before tapping, so the native call has a 150-second bound.

`ModalStackIOSTests` also drives UIKit's animated `popToViewController` to
check multi-entry Back in primary and presented stacks. It asserts the retained
controllers and the router's chosen visit, including return to the sheet root
without changing the background stack. It also checks that header search commits
and blurs once before dispatch and keeps the typed query, newer app navigation
wins during a pop (including navigation from blur), and an unmapped destination
restores the declared stack.
A test exercises UIKit's registered search-cancellation action and the controller
dismissal used by Duo's inner-display Close, including dismissal in a presented stack.
It checks that cancellation clears the live and committed query, commits once and
keeps the route. It repeats cancellation after the Search key dismisses the keyboard
and checks that no second blur is sent.
The runner tests distinguish visits with the same URL and reject stale or
cross-tab jumps. These tests use UIKit's public stack API; they do not establish
activation of its Back-history menu.

With `EXACT_SIM` naming an iPhone Duo, rebuild Duo Lab and the host fixture, then run
`bun scripts/smoke.mjs duo --only navigation,navigation-gestures,host-navigation`.
These legs cover native Back taps, completed and cancelled edge and content swipes,
and Back in independent embedded sessions. The gesture checks observe UIKit's held
interactive transition and check content clearance within the moving route. After
release they assert the route, draft, focus and scroll position. UIKit owns the
[content swipe recognizer](https://developer.apple.com/documentation/uikit/uinavigationcontroller/interactivecontentpopgesturerecognizer).

`--only secondary-navigation` checks an unfolded Duo's centered UIKit sheet
with its own navigation stack over a background stack that also has Back.
It covers native Back taps, completed and cancelled edge and content swipes,
the secondary editor's focus and draft, destination scroll restoration, and
horizontal clearance during the gesture. The sheet's root refuses a native
Back tap rather than targeting the stack behind it.

The runtime refuses a host call made from a callback it waits on
(`Owner.shared.callMain` inside `Owner.shared.sync`). To test a report made there,
change the native state inside that pair, then wait for the runner to apply it
(`testACoverFromAServedCallbackReachesTheRunner`, `testAReportWaitsForTheServedCallbackToReturn`).

## Browsers

- **Launch Chromium through `chromium()`, never by a path.** It
  (`scripts/agent-launch.mjs`) answers `CHROME` when it is set (a browser that
  is not there is reported as unavailable), else the Chromium that Playwright
  pins in `package.json`: by default its headless shell, which on an M5 Pro
  launched in 74 ms against the full build's 185 ms and used 263 MB of memory
  against 939 MB (the medians of 7 launches, every process of the browser
  counted). Install both builds with `bunx playwright@<version> install
  chromium chromium-headless-shell`.
- **Name a need when the shell lacks what the test uses.**
  `chromium({ needs })` answers the full build (Chrome for Testing, run with
  `--headless=new`) for `gpu` (WebGPU and WebGL on the GPU: the shell has only
  SwiftShader, whose fallback adapter a GPU app takes and then draws wrong),
  `pdf` (the PDF viewer), `hevc`, `bfcache` (the shell restores no page from the
  back/forward cache) and `perf` (timings that should match a person's
  browser). A web drive of an app with a GPU module asks for `gpu` itself
  (`webNeeds`).
- **A Playwright test takes `suiteBrowser()`.** It shares one browser in the
  process for each engine, build and set of launch options. `newPage` and
  `newContext` each make a browser context that belongs to the suite; `close`
  closes those, not the browser. A test that speaks CDP itself takes
  `cdpBrowser()` on the same terms: `page(url)` opens a target in the suite's
  context. A test about the browser process (it kills the browser, or needs a
  kept profile, a headed window or flags of its own) launches its own.
- **Wait for the event, not for its first sign.** The shell is faster than the
  full build, so a test that reads a state while the browser still changes it
  fails there first. `document.fullscreenElement` is set before the
  display-mode media changes are sent; `fullscreenchange` comes after them.

## Windows: the NucBox

Windows's oracle and its consumers' drives run on the NucBox, `nucbox-evo-x2`
on the tailnet (LLP 1115.004 D13). As recorded on 2026-10-23:

- **The machine.** Windows 11 Pro 25H2, build 26200, x64; an AMD Ryzen AI
  Max+ 395 with Radeon 8060S graphics. It has Rust's `x86_64-pc-windows-msvc`,
  Bun 1.4.2, and the .NET SDKs 9.0.313 and 10.0.203.
- **The display and input.** One display, a Samsung C49RG9x at 5120×1440, with
  no display scale or text scale set. It has no precision touchpad; D8 needs
  one added.
- **Reaching it.** SSH as `charlie cheever`, whose shell is PowerShell:
  `ssh -l "charlie cheever" nucbox-evo-x2`. `scp` refuses a space in
  `user@host`, so name the user in a config file (`User "charlie cheever"`)
  and pass it with `-F`. An SSH session has no GitHub credentials, so send
  commits as a bundle: `git bundle create`, copy it, then
  `git fetch <bundle> main:refs/remotes/origin/main` and `git merge --ff-only`.
- **SSH is not the desktop.** An SSH session sees one 1024×768 stand-in
  display (`WinDisc`), so a window it opens is never shown. Headless drives
  work (`bun scripts/agent.mjs windows --app shelf --storage <name> …`); a
  real window must open in the signed-in console session, as
  `host/windows/reference/draw.mjs` opens WinUI's, with a one-off scheduled
  task whose principal logs on interactively. WinUI launched from SSH exits
  with `0xC000027B` before its first frame.
- **The WinUI reference** pins Windows App SDK 1.8.260921001
  (`host/windows/reference/Reference.csproj` and its `packages.lock.json`) and
  builds with the .NET 9 SDK: `bun host/windows/reference/draw.mjs` draws each
  specimen of `specimens.json` in light and dark, at the display's 100%.
- **The drive's font.** The agent pins DejaVu Sans on Windows as on Linux
  (`scripts/agent-stdio.mjs`), so a drive draws the fixture font, not Segoe UI
  Variable. Set `EXACT_FONT` to a family to draw another, or to a name that
  is not installed (`EXACT_FONT=unpinned`) to draw the system's own. In
  PowerShell, `$env:EXACT_FONT = ""` removes the variable, and the pin
  returns.
- **Confirm focus independently when reports disagree.** Sky's focused-element
  field can remain on Excord's composer after Tab moves to a button. Windows SDK
  Inspect reports the button's UI Automation focus correctly, and Enter activates
  it. Check the visible focus, action and an independent accessibility client
  before changing the provider (`target/windows-chat-comparison/focus-inspect-qualification.json`).

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
- **Measure with a pruned `target/`.** Cargo never deletes old object files
  from `target/debug/deps`, and on macOS a test that touches the main bundle
  lists that directory, so hundreds of thousands of files there cost seconds
  of system time per test process. For URLSession startup stalls in
  `CFBundleCopyBundleLocalizations`, changing the working directory is
  insufficient: Foundation inspects the executable's directory. Copy the
  test executable into an empty temporary directory and run that copy with
  the same filters and deadlines to isolate directory accumulation.
- **Gatekeeper scans each newly built binary when it first runs.** After a
  kernel edit, the scans added about 95–130% to the wall time of rebuilding
  and running ten packages' Rust tests. Adding the program that starts the
  test processes (a terminal, say) to System Settings → Privacy & Security →
  Developer Tools skips them.
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
