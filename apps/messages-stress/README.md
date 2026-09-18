# Messages stress

LLP 1041's Messages workload uses the real Contract → Rust `DataSource` →
runner → existing web/Apple host path. It never reads Messages, accounts,
files, secrets, or network data. It declares no grants. Rust is binary-bound
(`rust.mode = off`); this is not a worker-placement or module-reload demo.

Workspace members:

- `apps/messages-stress/data` (`messages-stress-data`)
- `apps/messages-stress/web` (`messages-stress-web`)
- `apps/messages-stress/apple` (`messages-stress-apple`)
- `apps/messages-stress/linux` (`messages-stress-linux`)

```sh
export PATH="$HOME/.cargo/bin:$PATH"
EXACT_UPDATE_TRUST=development cargo test -p messages-stress-data
EXACT_UPDATE_TRUST=development cargo check -p messages-stress-web -p messages-stress-apple
EXACT_WEB_DIST=target/messages-stress-dist bun host/web/build.mjs messages-stress-web
bun host/web/serve.mjs --origin target/messages-stress-dist --loopback 4318
```

Open `http://127.0.0.1:4318/`. Use a separate output directory/port for the
other stress app; the default web output directory is shared across apps.
For live editing, use `EXACT_WEB_DIST=target/messages-stress-dev bun
host/web/dev.mjs --app messages-stress --port 8771 --loopback` instead.
The ordinary native wrapper is available with
`bun host/apple/build.mjs messages-stress-apple --run` (or `--ios --run`).

## What to try

1. Type in the composer and check the exact live text below it. `stress-input`
   and `stress-echo` are the shared browser sampler's test IDs.
2. Select 100 / 1,000 / 10,000 / 100,000 logical messages. The default remains
   a **manual 100-row page** showing the latest records. Earlier/Later moves
   by one page; scrolling only scrolls that page. It is not virtualization.
3. Choose 1 / 8 / 32 tail messages per revision; start the producer while
   typing and scrolling. Its existing root task requests a revision every
   250 ms and stops after 120 revisions. Start resets that finite run; Step
   does one revision, Pause stops future production, Reset restores 100 rows.
4. Opt into **Construct ALL rows (eager)** to supply the full selected history
   to `each`. Begin with 1,000. At large counts the main thread may stall;
   Pause cannot preempt a synchronous query or mounting already underway.
   Reload returns to the small default if the UI stops responding.
5. **Virtualized transcript** supplies the same full history to the shared
   `list virtualized=true` path. The runner mounts nearby variable-height rows
   and up to two focus/interaction pins. Scrolling covers the whole history;
   the record data and compact key/height index still grow with history size.
   Swipe a bubble right or use its Reply button; Cancel clears the reply target.
6. Local echo puts the draft in the transcript and clears the composer. Only
   one local echo is retained; the next replaces it. Nothing is sent anywhere.

Drafts are limited to 512 Unicode scalar values. Oversized edits are refused
and the previous draft remains. Histories only accept the four presets,
revision is 0..120, batch is 1/8/32, and page offsets are checked before any
allocation. Six deterministic text patterns create varied natural heights;
keys stay stable across streaming revisions and history sizes.

## What the numbers mean

- Logical history: requested synthetic cardinality, **not resident records** in
  manual-page mode. The page generator allocates only that page. Eager and
  windowed modes both supply the complete selected history.
- Supplied records: exact `DataSource` list length, including the optional
  local echo. This is not a measured DOM/native-view count.
- Revision: the returned resource's applied revision, not an arrival rate.
  The nominal 4 ticks/s is requested cadence, not measured throughput.
- Revised rows: rows in the current slice carrying that revision. Older manual
  pages do not include the updated tail and can show zero.
- UTF-8 body bytes: a sum of returned message bodies, not heap/RSS, record
  metadata, DOM cost, or decoded images. There are no image attachments.

The default source reconstructs its supplied list synchronously on each revision.
Eager data and UI are O(N); windowing bounds UI lifetime, not record generation
or key validation after a changed list. There is no worker placement or
preemption of that synchronous work in this fixture. Typing alone
only changes the draft and its live echo, not the history resource arguments.
No FPS, frame deadline or physical presentation result is asserted by this UI.

The data crate also exports opt-in `ReusableMessagesStress` for comparing
allocation costs. It keeps one latest immutable result and reuses unchanged row
records: a 10,000-row update changing 32 bodies retains the other 9,968 records.
It uses the original generator for a temporary 100-row tail page, copies O(N)
row handles, and still incurs fresh-answer validation and positional scanning.
The Runner reuses keys for unchanged immutable records: the tested 10,000-row,
32-change update evaluates 32 keys, while the stateless control evaluates all
10,000. This stress app's shipped entries and bake still use that control;
Exact Live opts into row reuse.

Three native comparison pairs observed lower loaded advance/decode medians
with row reuse. Timer tails still exceed 8.33 ms, resize and input results are
mixed, and the repeat pairs have different timed revision cohorts. See
[LLP 1041 §8.42](../../llp/1041-graceful-overload.rfc.md#842-two-alternating-native-messages-repeat-pairs-2026-09-17)
for the measurements and limits; they do not establish physical 120 Hz.

Use the existing optional browser diagnostic, once the server is ready:

```sh
bun scripts/metrics.mjs --stress-url http://127.0.0.1:4318/ --seconds 10
bun scripts/metrics.mjs --stress-url http://127.0.0.1:4318/ --seconds 10 --tap history-1000 --tap toggle-eager --tap batch-32 --tap start
```

That sampler labels input event → matching echo DOM → following rAF; it does
not measure physical presentation or prove a 120 Hz display budget.

## Tests

`data/tests/generation.rs` was written before the generator and can also run
without Cargo while workspace builds are coordinated:

```sh
rustc --edition=2021 --test apps/messages-stress/data/tests/generation.rs -o /tmp/messages-stress-tests
/tmp/messages-stress-tests
```

It covers all four cardinalities, stable unique keys, deterministic mixed
content, exact changed-tail counts, page coverage, invalid controls and bounded
Unicode local echoes. `data/tests/runtime.rs` compiles and bakes this actual
Contract, boots the runner, dispatches controls, advances its deterministic
clock, and checks draft/control behavior and malformed data-seam inputs.
Virtual-clock assertions are correctness checks, not performance evidence.
`data/tests/windowed.rs` also covers the actual 10,000-record Contract's
bounded mounted rows, an active offscreen row retained until release, exact
typing, reply identity and the preserved eager/manual controls.
`data/tests/reuse.rs` compares complete canonical values and bytes with the
stateless source, checks immutable sharing and last-owner release, and runs the
real Contract through typing, width changes and ticks. The integrated data suite
has 26 passing tests and one ignored opt-in timing test; strict all-targets
Clippy passes. The original source produces nine behavioral failures in the
13-test reuse suite, retained under `target/messages-row-reuse-validation/`.

The final browser drive (`target/messages-windowed-web-final/`) passes five
endpoint round trips from 10,000 supplied records, exact typing, a streaming
step and actual browser pointer swipe commit/cancel. A single end jump reaches
the last logical message. The first refinement mounts 34 rows; later top/end
observations mount 7–8. These are functional checks, not twenty full traversals
or frame timings. That drive predates the dedicated gesture-takeover validation
below. The separate paired runner diagnostic in
[LLP 1010 §6.6](../../llp/1010-scrolling-v1.spec.md#66-paired-runner-evidence-2026-09-16)
covers twenty complete traversals, retained state and CPU work through 25,000 rows.

## Native hosts

The Linux wrapper is the `messages-stress-linux` workspace member. These commands
use the existing presenters and the same Contract and data source:

```sh
bun host/apple/build.mjs messages-stress-apple
bun apps/messages-stress/native-smoke.mjs macos --samples 12

EXACT_UPDATE_TRUST=development cargo build --release -p messages-stress-linux
bun apps/messages-stress/native-smoke.mjs linux --samples 12
```

The opt-in driver opens a fresh app for `idle100` and `eager1000`, types while
checking the exact echo, verifies revisions, scrolls, takes a screenshot, sends
a local echo and resets. `eager1000` uses 32 updated tail messages. It records
raw samples and p50/p95/p99/max in
`target/messages-stress-native/<host>-<operating-system>/report.json`, with a
screenshot per profile. `--out <directory>` selects another artifact directory.
The host binary must be built before invoking the driver.

This is a **native agent command acknowledgement diagnostic**. The agent clock
is seekable: each loaded sample queues one 250 ms advance followed immediately
by a type command. There is no claim of a continuous wall-clock producer. Input
acknowledgement includes IPC and any queued synchronous update; echo acknowledgement
additionally includes inspection of the full runner tree. Neither is physical
presentation, OS input latency, frame timing or a 120 FPS result. Runner node
counts are not native live-view counts. Percentiles of small runs are diagnostics,
not robust benchmark estimates.

`macos` uses actual AppKit and its native text/layout/presentation path. `linux`
uses the Linux host's headless CPU raster path; it can also run on macOS for
correctness, and the report records the actual OS. A Linux-host run on a Mac is
not evidence from a Linux OS. A Linux VM run is labelled a VM by its operator;
neither headless case measures a Linux desktop compositor or physical display.

The windowed AppKit drive in `target/messages-stress-native/collections/` passes
10,000 supplied records with at most twelve mounted message rows, twelve loaded
typing samples, held-contact retention/release, Reply/Cancel and three actual
window widths. One jump reaches the last row; appending follows the tail while
an older reader keeps its key and offset. Final executable SHA-256:
`427baeb90112edaf611067203b6dd7c02256f29dfeb0b72f707e360500ef3024`.
At that checkpoint AppKit lacked the swipe recognizer: held mouse contact and
Reply buttons did not establish native swipe continuity. Twelve standalone Swift assertion
bodies pass; full XCTest is unavailable with this machine's Command Line Tools.
iOS adapter source has not been compiled against its SDK or driven on a device.

Actual Ubuntu 24.04 ARM64 CPU-raster validation passes the windowed 10k drive:
seven endpoints, three resizes, Unicode typing, a stream step, Reply/Cancel,
local echo and teardown/remount. Every endpoint uses one wheel and two no-wheel
observation requests, with 5–8 numbered rows (plus the echo when present) and
97–127 kernel nodes. The headless carrier pumps on requests; no physical held
pointer or autonomous compositor timing is inferred. Native pin regressions are
separate, within 69 passing Linux tests and strict Clippy.

Evidence: `/tmp/exact2-linux-endfollow-final-6840b5e9/`; final executable SHA-256
`6810b9bda835ace10c5147622427cc5d19ffed5254aaff6159ba021d1cfe3547`.
The source manifest and Noto CJK/emoji font identities are retained there.
Earlier failed captures remain: an origin/range fix corrected real adapter
errors, then a package-scoped runner rebuild removed a stale cached artifact
without further source changes. Those failed binaries are not final acceptance.

## Interruptible swipe, 2026-09-17

The shared token-hold implementation now drives browser, AppKit and Linux swipe
input. Grab a returning row, move left immediately, reverse and release; typing
or target/transition commits preserve the held presentation. Final actions run
once while ownership is live. Disabled or inactive retained views cancel, and
stale callbacks cannot act on replacement views. The browser still executes
springs through WAAPI; native hosts use the same seekable motion engine.

AppKit's 10k native-event drive and linked lifetime/reentry/eligibility probes are
in `target/messages-stress-native/holds/`, with exact source and executable
identities. Browser checks include actual compiled Wasm plus DOM pointer/WAAPI
takeover, style commits, delayed release, deletion and retained-route cancellation.
Logs are `/tmp/exact-web-motion-*`. These cover functional behavior; UIKit source
has not been compiled or driven against its SDK here.

`target/messages-stress-native/linux-holds-vkms/report.md` records the actual
Ubuntu DRM/KMS display-loop drive, using CPU rendering and a virtual 60 Hz
connector. Existing VNC pointer and ASCII key input exercises 10k rows, typing
while held, release/catch/reverse, autonomous settling and contact-disconnect
cleanup. Raw framebuffer checks verify position changes; source/binary identities
and the earlier control are retained. It does not measure a physical screen,
physical input latency, a desktop compositor or 120 fps. The separate headless
Unicode and viewport-resize results above remain distinct evidence.
