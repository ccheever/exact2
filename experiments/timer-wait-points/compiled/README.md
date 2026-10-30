# A precise timer during a fling, in compiled Messages

This measures whether Messages' `task network mount` / `every(250, poll)` runs while its transcript is flung, on an iOS simulator, at commit `b0ce559698ebafc8a2b91c47e6efcc83979670a0`. The simulator was an iPhone Duo on the iOS 27.1 runtime under Xcode 27.1, created for the run and deleted after it. It presented frames at 60 Hz (period 16.67 ms). The Mac's load average was between 150 and 700 during the runs, so frame lateness here is indicative only (LLP 1144 D1); the commit and per-site counts are what hold.

## Under the agent, the timer does not run

`scripts/fling.mjs` launches the app on the agent's socket. Under the agent (`EXACT_AGENT=1`), even with platform timing, the driver owns the runner's clock: `SessionClock.scheduleClock` arms no wake, and `fireClock`, `advanceIfDue` and the display link's timer advance all return under `ExactEnv.agentMode`. `agent-probe.mjs` shows it: the runner's clock stayed at 12,719 ms, and the `network` task's next deadline at 12,959.69 ms, across 5.4 s of wall time that included a real fling of the transcript (`results/agent-probe.json`, `states`). Two `fling.mjs` runs of six flings each (`results/fling-agent-2.json`, `results/fling-agent-3.txt`) report no rows created, no handler dispatches and no work, and still 21 late frames each, with 187 to 216 ms waiting on the runtime and 708 to 923 ms unattributed: that lateness has no timer in it. A third run (`results/fling-agent-1-failed.txt`) stopped when a real touch landed on another node than its aim. So the fling report cannot show a timer's batches today.

## Without the agent, every beat lands in a frame of the fling

`fling-free.mjs` launches the development build with `simctl launch` and no `EXACT_AGENT`, opens the thread "demo", plays the same flings through the XCTest touch runner at the points `agent-probe.mjs` recorded, and saves a trace by `SIGUSR1` before the flings and 1.5 s after each one. The runner's accessibility snapshot after each fling shows the transcript moving about 47 messages each way (`fling-free-log.json`, `positions`; a simulator screenshot of a Duo is black). `analyze-free.mjs` reads the frames in the 1.5 s after each touch and the per-site work between traces; `analyze-late.py` relates every late frame in a run to the frame before it.

Three runs gave 22 flings and 1,970 sampled frames in those windows (`free-1/`, `free-2/`, `free-3/`, each with `analysis.txt` and `analysis.json`). Every frame applied one batch, because Messages' `every(frame, animate)` commits once a frame. Every 250 ms, on every 15th frame, that batch held two commits: the `poll` beat and the frame task's. That is six beats per fling, 132 in all, and all 132 came in the display link's batch (one batch, two commits), not as a batch of their own from `SessionClockTimer`'s run-loop timer.

Each beat re-evaluated the `MessageRow` site at `bubble.contract:76` once for every message in the thread (153 per beat in `free-1`, 453 in `free-2` and `free-3`) and the sites at `app.contract:309`, `316` and `317` once, `app.contract:354` three times and `bubble.contract:66` and `87` twice. Almost none of it changed anything: of 134,249 evaluations between the traces, 2 changed a value (`bubble.contract:87`, once each in the first fling of `free-1` and `free-2`), and every frame's `apply` was 0 ms.

Five frames in the windows were late. Four of them were the frame right after a beat's frame, where about 7 % of frames follow one, and they waited 14.6, 28.6, 49.4 and 86.7 ms on the runtime. Over the whole runs, set-up and rest included and trace saves (intervals over 300 ms) left out, 30 of 46 late frames followed a beat's frame, with runtime waits from 3.4 to 137.7 ms (`results/late-frames.txt`). The beat's own frame was on time; the wait came in the next frame's synchronous runner call, which is consistent with the beat's follow-on work (its `heartbeat` send and the `data` refresh it causes) still running on the runtime thread, though nothing here traces that.

## Running it

```sh
export DEVELOPER_DIR=/Applications/Xcode-27.1.app/Contents/Developer EXACT_SIM=<udid>
bun host/apple/build.mjs --ios messages-apple --sim $EXACT_SIM
bun experiments/timer-wait-points/compiled/agent-probe.mjs experiments/timer-wait-points/compiled/results/agent-probe.json
bun scripts/fling.mjs ios messages --target transcript --to "tap conversation-demo" --to "wait 1500" --to "tap add" --to "wait 800" --to "tap add-fifty" --to "wait 1500" --flings 6
bun experiments/timer-wait-points/compiled/fling-free.mjs --sim $EXACT_SIM --aims experiments/timer-wait-points/compiled/results/agent-probe.json --out experiments/timer-wait-points/compiled/results/free-N --flings 6 --positions
bun experiments/timer-wait-points/compiled/analyze-free.mjs experiments/timer-wait-points/compiled/results/free-N
python3 -I experiments/timer-wait-points/compiled/analyze-late.py experiments/timer-wait-points/compiled/results/free-N
```

The `fling.mjs` runs added the Add panel's 50 messages three times; `fling-free.mjs` does too unless given `--fifty 0`. The app keeps its messages between launches, which is why the thread grew from 153 to 453 messages between runs.

## Minimal physical-device fixture

`timer-test.contract` uses Exact's empty Rust data source: one fixed 300-row list and
one counter, with no networking or app storage. Two 250 ms timers start 25 ms apart.
The controls choose precise timers, 40 ms native tolerance, `wait=scrollend`, or
`wait=update`. "Hide count" keeps the displayed value constant while the timers
continue, so a power comparison adds no counter-driven UI updates. "Save trace"
ends the current frame recording. A screen wake lock prevents auto-lock during a
measurement; it lasts only while this test view is present.

The device build uses the `hello-apple` shell with this Contract and a separate
`dev.exact.timerqualification` bundle identifier. The Hello and Messages sources are
restored after building; the measurement app carries its own baked plan. XCTest
and Instruments stay outside the app. Physical gestures require the iPhone's
Settings → Developer → Enable UI Automation permission.

For a quiet power run, launch with `EXACT_SAMPLE_FRAMES=0` to disable development
builds' automatic frame-sampling display link. Explicit `startTrace_EXPERIMENTAL()`
recordings still work and release their sampler when stopped; signal-triggered
diagnostic snapshots require normal sampling. Compile two copies
of the fixture with `quiet = true`, keeping `policy = 0` for precise timers and
setting `policy = 1` for tolerant timers. Load each with `EXACT_PLAN` pointing to its
device-local absolute path. Query the app's data-container path after installation
and verify each loaded plan's hash with a separate, normally sampled trace before
profiling: reinstalling can change the container path, and an unreadable plan path
falls back to the embedded plan.

### iPhone process wakeups

Measured on 2026-10-28 with an iPhone 15 Pro (`iPhone16,1`, iOS 27.0.1 build
24A446), Xcode 27.1, and the optimized development build at `492d8c0a48` plus this
fixture and the sampler opt-out. The phone stayed USB-connected and the screen
awake. Four Activity Monitor recordings used 30-second limits in precise,
tolerant, tolerant, precise order, each after a three-second warmup, with automatic
frame sampling disabled. The loaded plans' hashes were checked before these runs;
there was no intervening installation.

| Native tolerance | Sample span | Process idle wakeups | Wakeups/s | CPU time |
|---|---|---|---|---|
| 0 ms | 29.499 s | 243 | 8.237 | 0.105 s |
| 40 ms | 28.650 s | 117 | 4.084 | 0.109 s |
| 40 ms | 29.425 s | 121 | 4.112 | 0.107 s |
| 0 ms | 29.410 s | 240 | 8.161 | 0.106 s |

The exported `activity-monitor-process-live` table supplies cumulative counters.
Each delta runs from the first sample to the penultimate sample, over the sum of
the intervening row durations; the final partial row is excluded. The sanitized
[results](results/iphone15pro-native-timers.json) retain the counters, sample spans,
plan hashes and command forms. The combined wakeup rate is 8.199/s precise and
4.098/s tolerant, about 50% lower. These are process wakeups in this quiet workload;
callback counts and battery energy were not collected. Separate Power Profiler
attempts failed during finalization, and their recoverable traces supplied no
power-metric rows. The physical fling comparison below measures frames separately.

### iPhone flings

On the same iPhone and app build, the physical XCTest
`DeviceFlingTests/testTimerPoliciesDuringRealFlings` passes. It performs three warmup
swipes, then selects precise, `scrollend`, `update`, `update`, `scrollend`, precise.
Each pass records three fast down/up swipe pairs. Each of the six saved recordings
contains six drag segments and six glide segments on `timer-list`, at a median
8.34 ms display period, with no recording drops. Low Power Mode is off. The counter
updates all 300 row labels; the agent and clock overrides are absent.

| Policy | Glide frames, by pass | Missed frames, by pass | Glide hitch ms/s, by pass |
|---|---|---|---|
| Precise | 1,634; 1,623 | 81; 87 | 47.10; 51.82 |
| `wait=scrollend` | 1,715; 1,711 | 14; 14 | 8.54; 8.37 |
| `wait=update` | 1,261; 1,256 | 495; 506 | 279.30; 284.92 |

The [results](results/iphone15pro-flings.json) retain every pass's phase totals,
recording identity and raw-trace hash. The combined glide hitch rates are 49.46,
8.46 and 282.11 ms/s respectively. `scrollend` moves work out of the glide;
`update` produces substantially more hitching in both passes. In the second
`update` pass, the retained late frames include 3.88 s of the host's runtime phase,
which includes waiting on the runner and kernel layout; the second precise pass
has 0.020 s. The recordings identify the cost, but do not establish its cause.

The test asserts element existence and executes gestures, with no frame-lateness
threshold. Its accessibility snapshots and policy presses add work. The table
compares glide phases rather than whole recordings, which also include rest and
policy changes. Hitch time is a display-link estimate that omits render-server-only
hitches. The first `update` recording retains the worst 200 of 214 late frames;
the other recordings retain every late frame. Per-firing admission timestamps and
per-site runtime work counters are not captured in this baseline. These short
repeated passes do not establish a general performance benefit for every wait policy.

### Owner-queue admission and timing evidence

The follow-up keeps the same fixture, phone and six-pass XCTest protocol. Native
UI admission now posts the runtime job after the current main-thread turn instead
of synchronously waiting for action evaluation and layout inside `UIUpdateLink`.
Exact's own frame callbacks keep their ordering barrier. A continuation retains its
original cutoff and samples observations again after each published batch;
takeover and session replacement invalidate queued submissions and release their
busy state. Ineligible held firings skip owner work.

| Policy | Glide hitch ms/s, by pass | Combined glide hitch ms/s |
|---|---|---|
| Precise | 42.67; 44.74 | 43.70 |
| `wait=scrollend` | 0; 0 | 0 |
| `wait=update` | 50.84; 47.98 | 49.41 |

Each recording again has six drag and six glide segments with no recording drops.
The measured `update` rate falls from the baseline's 282.11 to 49.41 ms/s; this
removes most of its excess hitching in this workload. These measurements do not
establish that `update` always improves on precise timers. In the two `update`
passes, retained late frames contain 72.67 and 93.43 ms of runtime-phase work, versus the baseline's
3.90 and 3.88 seconds. The first precise pass and both `update` passes each execute
148 measured timer firings and evaluate the row-text site 44,700 times, including
the policy change. Their action work is comparable; `scrollend` intentionally runs
fewer firings while a scroll holds it.

Full recordings now include `work.before` and `work.after`: per-site `perf` counters
and the runner journal. While measuring, each timer attempt records its row,
admission kind, nominal deadline, host cutoff and clock after the action's last
commit. The clock after a commit is not wall-clock completion time. All measured
`update` firings have kind 2, with no cap fallback or duplicate nominal firing. The
[results](results/iphone15pro-owner-queue.json) retain timing distributions, counter
deltas, recording summaries and raw-trace hashes. Counter windows include policy
changes and are not per-frame attribution. The baseline's measurement limits still
apply. Actual energy-savings measurements and GPU qualification remain deferred.

### Windows JavaScript timer qualification

The Windows 11 Pro ARM64 VM, build 26300, runs an MSVC x86-64 integration binary
under Windows' x64 emulation, using the pinned real Hermes lean engine. The timer
fixture exercises precise timing, tolerance and all four waits; held firings are
cancelled through their gate and restarted before refreshing a real bytecode-backed
resource. The lean-engine identity check and explicit-input/async-interleaving test
also pass. Microsoft's C++ runtime is deployed beside the test executable.
The [results](results/windows-hermes-timers.json) record the engine pin, binary and
runtime hashes, test output and limits. This closes the timer-specific JavaScript
qualification gap; it makes no native ARM64 Hermes or GPU qualification claim.
