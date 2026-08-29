# LLP 1012: Agent API v1 — eight operations, the clock in the agent's hands, as built

**Type:** Spec
**Status:** Draft (r2, unreviewed. r1 was reviewed by two families 2026-08-29 — `llp/reviews/1012-agent-api-v1.{codex,grok}.md`, both NOT READY — and the code by the same two — `llp/reviews/code-2026-08-29-agent-api.{codex,grok}.md`; r2 folds both, and the code they describe changed under them: §8.)
**Systems:** Runner (`agent.rs`, the journal, `advance_timed`), Web host (`exact_agent`, `at` markers, `glue.js` agent mode), Apple host (`exact_agent`, timed motion sync, `Agent.swift`), Tooling (`scripts/agent.mjs`, `scripts/smoke.mjs`)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Revised:** 2026-08-29 (r2: after the round-1 reviews — the clock made true on both hosts, the contract a host implements, the smoke walked back to what it checks, the private messages named, the numbers dated)
**Implementer:** Claude (Fable 5); landed 2026-08-29 (this document transcribes it)
**Related:** `rules/NOT-DOING.md` §Agent API (the eight; `clock` replaces `wait`), LLP 1002 D3 (the clock is a seek), LLP 1005 §6 (`dispatch`, `advance`; `act` is for tests), LLP 1007 §3 (the web glue), LLP 1008 §4 (the C ABI this adds one call to), LLP 1010 §5 (the scrolling the smoke holds), LLP 1011 (the image whose box it holds); research: exact1 `llp/0495-acto-on-the-substrate.rfc.md` §4.1

## Summary

An agent drives the app through **eight operations** — `tree · screenshot ·
tap · type · state · layout · logs · clock` — the same on every host, with
**time in its hands**: between two operations nothing the runner or the
motion system owns moves, and instead of waiting the agent seeks the clock.
`tree`, `state`, and `logs` are answered once in the runner behind one export
on each ABI; `layout`, `screenshot`, `tap`, `type`, and `clock` are the
host's, because they are about what it renders, its input path, and its
clocks. One driver carries both hosts (headless Chrome over the DevTools
protocol on a pipe; the macOS app over stdio); the smoke is a script of the
operations that runs unchanged on both. Where this document and the code
disagree, the code and its tests are the authority.

## 1. The operations

Public: what `scripts/agent.mjs` exposes as a session (`open({host, plan,
size})`), and what the CLI runs one per argument. A target is a `testId`
(first in preorder) or a view id; the driver resolves it through `tree`, so a
host only ever sees a view id.

| op | request to the host | reply | who answers |
|---|---|---|---|
| `tree` | `{"op":"tree"}` | `epoch`, `incarnation`, `clock`, `roots`, `nodes[]` in preorder: `id`, `parent`, `depth`, `type` (schema name), `props` by schema name, `handlers` (`press`/`change`), `children` | runner (`Kernel::rows` + props) |
| `state` | `{"op":"state"}` | `clock`; `slots`, `derives`, `resources` by declared name as typed JSON: records keyed by field name, `none`/unit `null` | runner (the plan's type table) |
| `logs` | `{"op":"logs","since":N}` | `next`, `from`, `lines[]` — the journal from `since` (§3); the driver adds `host[]` (page console / app stderr) and `dropped` | runner |
| `layout` | `{"op":"layout"}` | `clock`, `viewport{w,h}`, `nodes[]`: `id`, `x`, `y`, `w`, `h` (+ `sx`, `sy` on scroll containers); the driver adds `type` and `testId` | host |
| `tap` | `{"op":"tap","id":V}` / `{…,"wheel":[dx,dy]}` | `tapped`, `at`; the driver adds `target` | host input path |
| `type` | `{"op":"type","id":V,"text":…}` | `typed` (+ `value` on macOS); the driver adds `target` | host text path |
| `clock` | `{"op":"clock","to":ms}` / `{…,"settle":true}` | `clock` (where it landed), `settled` for `settle` | host, both clocks |
| `screenshot` | `{"op":"screenshot","path":…}` (+`"window":true` on macOS) | `screenshot`, `w`, `h` (viewport points / CSS px, not PNG pixels; `scale` for a window capture) | host |

Errors are `{"error":"…"}` on the wire; the session throws `"<op>: <message>"`.

**The contract a host implements** (macOS is the worked example, `Agent.swift`;
the Linux host implements this list, not that file):

- **Space.** `layout` boxes are in the viewport's space: origin top-left, y
  down, points / CSS pixels (never device pixels), every enclosing scroll
  offset **and** presentation transform folded in — the web's
  `getBoundingClientRect`; macOS converts the view's transformed bounds into
  the clip view and subtracts the clip's origin. Two decimals on both.
- **Membership and order.** Every view attached to the document (web:
  `isConnected`; macOS: in a window), whether or not it lies inside the
  viewport, in ascending id order. `sx`/`sy` present only on scroll
  containers.
- **`tap`** presses at the box's center through the platform's own hit-test
  and dispatch — CDP `Input.dispatchMouseEvent` mousePressed/Released on the
  web (Chrome synthesizes the click), `NSWindow.sendEvent` mouse down/up on
  macOS — never `Runner::dispatch`. A press on a node without a handler
  reaches its parent the way a DOM click bubbles. With `wheel: [dx, dy]`,
  `dy > 0` scrolls down on both hosts; the web sends CDP `mouseWheel` under
  `--disable-smooth-scrolling` (fractional deltas allowed), macOS a phase-less
  pixel-unit `CGEvent` (`wheel1 = −dy`, `wheel2 = −dx`, rounded to whole
  pixels, bounded, non-finite refused) to the hit view — `scrollWheel(with:)`
  on `hitTest(center)`, so the responder chain carries it up as a trackpad's
  would.
- **`type`** sets an input's whole text as a paste does: select all, insert.
  Web: `focus` (a page-side helper, §1 private) then CDP `Input.insertText`;
  macOS: first responder, the field editor's `selectAll` + `insertText`. One
  `change` with the whole value; a non-input target is refused on both.
- **`clock`** is monotonic (a backwards `to` is refused). It moves the
  runner's clock and the host's motion clock to one instant and **lands where
  the runner says** (`batch.clock`): a timer's refusal stops the advance at
  that timer's due time, the commits before it are shown, the refusal is the
  reply's error. `settle` is a fixed point: advance to when the last thing in
  flight ends — the motion engine's `settle` (a private read on the ABI), and
  on the web every `Animation`'s computed end — and if the timers crossed on
  the way started more, again; sixteen rounds, then `settled: false`. §2.
- **`screenshot`** is the rendered pixels: `Page.captureScreenshot`, or
  `cacheDisplay` of the viewport (Metal layers absent), or with `window`
  the window server's picture (`screencapture -l`, Metal included, needs
  screen-capture permission).

**Private messages** are not operations: `focus` (web `type`'s first half),
`settle` (the engine's end time, read by `clock`), `quit` (macOS stdio), and
the `at` op inside a batch (§2). `exact.reload` / `EXACT_PLAN` boot a plan —
session setup, not a drive. `Runner::act` runs an action by name for **tests**
(LLP 1005 §6 now says so); an agent never takes it.

**Eight, and a wheel is a form of `tap`** (Charlie, 2026-08-29: "A"). A drag
would be another form. `rules/NOT-DOING.md` §Agent API binds the count and
the trade — a ninth operation replaces one of the eight, same PR — and this
document cites it rather than restating it as a second law.

## 2. The clock

Agent mode is opt-in per launch: `?agent=1` on the page (only then does
`globalThis.exact` carry `agent` and `now`), `EXACT_AGENT=1` for the macOS
app. In it the driver owns time: the page's `now()` is the last clock
value, the 250 ms ticker never starts, events carry that value, and every
animation the browser holds is **frozen** where the clock says.

**Timed commits.** `Runner::advance_timed(to)` fires each due timer at its own
due time and returns the commits with those times (`Timed { at_ms, receipt }`),
the clock it landed on, and the refusal that stopped it, if any — the commits
before a refusal are kept, because they are in the kernel. Both hosts use it:

- the web host writes an `{"op":"at","ms":…}` marker before each commit's
  ops and that commit's spring frames after them; the batch's trailer
  carries `"clock"` (the runner's clock afterwards);
- the Apple host seeks the motion engine to each commit's time before feeding
  it that commit, then to the landing time once.

So a transition a timer starts is born at the timer's due time on both
hosts, and one seek gives the bits sixty would (LLP 1002 D3). Known
deviation: two timers writing the same animatable row inside one seek are
seen through the receipts against the final kernel — the intermediate target
is not replayed.

**Freezing on the web.** After every batch (`applyBatch`): the clock moves to
`batch.clock` if that is later; every animation not yet seen is registered at
the clock it began; every animation is seeked — `finish()` past its end, else
`pause()` + `currentTime`. At an `at` marker: register what the ops before it
started at the clock so far, move the clock to the marker, seek. An animation
started by a `tap` therefore sits at local time 0 until a `clock` moves it;
`layout` or `screenshot` twice with no `clock` between give the same answer.
`document.getAnimations()` flushes style, so a transition the batch's
`cssText` started is seen in the same turn; CSS transitions and
`Element.animate` springs are handled alike. The GPU module renders with the
page's `now()` in agent mode and never reschedules itself; `clock` asks it
for a frame.

**What still moves on its own** — host I/O, not the clock: an image
finishing decoding (the kernel relays out; poll `layout`), the GPU module
loading after the first paint. The smoke polls for both and says so. On
macOS the display link keeps ticking under agent mode; every tick seeks the
engine and the canvases to the same agent clock, so it repaints the same
picture.

**Errors.** A batch's `error` from `exact_advance` is the `clock` reply's
error; the clock still reports where it landed. Non-agent launches are
unchanged except that a dispatch or advance batch now carries `clock` and
`at` markers, which the normal glue ignores.

## 3. The journal

`Runner::log` / `journal` / `journal_start`: a ring of the last
`JOURNAL_RING` = 4,096 lines (about an hour of a one-second timer), the
oldest dropped first, silently — `journal_start` counts them. Every line is
stamped with the runner's clock: `boot: 205 nodes, epoch 1` (`(carried)` on
a reload); each `dispatch`/`act` with its outcome — `press view 12
(openStations) → epoch 2 (+79 −187 ~1)`, `… refused: UnknownView(9999)`, or
`… poisoned the runner: …` on the transition into poison (later refusals
say `refused: Poisoned`); each command an action emitted, journaled once the
update committed (`command setScheme("dark")`); each advance that fired
timers (`advance → 60 timers fired, epoch 5`) and each timer that refused
(`timer 0 (tick) refused: …`). Hosts may append with `Runner::log`; none
does today.

`logs` replies `{"next":N,"from":M,"lines":[…]}` with `from = clamp(since,
journal_start, next)`: a reader whose cursor the ring has passed gets the
suffix and a `from` above its `since`. The driver surfaces it as `dropped =
from − since`, and the transcript form prints a marker line. A non-numeric
`since` is an error.

## 4. Where the code is

`runner/src/agent.rs` (`handle` → `tree`/`state`/`logs`; `typed_json`; a
one-pass top-level JSON field scanner — string tokens and nested objects are
stepped over, surrogate pairs decode, no serde); `runner/src/runner.rs`
(`Timed`, `Advanced`, `advance_timed`, the journal); `host/{web,apple}/src/host.rs`
(`Host::agent` — `settle` from its own engine, the rest delegated; timed
batches); `host/web/src/batch.rs` (`at`, `clock`); `host/apple/include/exact.h`
(`exact_agent(len)`); `host/web/glue.js` (agent mode: `applyBatch`, `register`,
`seek`, `settleCandidate`, `agent`); `host/web/gpu-glue.js` (the page's clock
for surfaces); `host/apple/macos/Sources/ExactMac/{Agent,Bridge,main}.swift`
(`EXACT_AGENT=1`: JSON lines on stdio answered in order on the main thread;
`ready` once the window is key or after a second; `wall()` for the startup
stamps, `now()` for the app); `scripts/agent.mjs` (the driver: a ~130-line
DevTools-protocol client over `--remote-debugging-pipe` with deadlines and
failure on a dead pipe, `Emulation.setDeviceMetricsOverride` for an exact
viewport, stdio for macOS, `--plan` on both; `render`, §7);
`scripts/smoke.mjs`; `scripts/fixtures/transcript.{json,txt}`. The two old
per-host smokes are gone; `EXACT_SMOKE=1` remains in the macOS app for the
startup stamps `scripts/metrics.mjs` reads.

## 5. What the smoke checks, and what was measured

`scripts/smoke.mjs <web|macos>` asserts, with `check(...)`, on both hosts:
the transcript fixture renders byte-equal (§7); the five landmarks and
"Mountain View"; the root's width equals the viewport's; the logo's box is
96×36 (polled, up to 2 s — image decode is host I/O); after `clock +60000`
**every** countdown still shown is one less, `state.clock` is 60 000,
`nowMs` moved by 60 000, and the journal shows `advance → 60 timers fired`;
`tap change-station` shows the stations screen; `type station-search Palo`
leaves the field's value and the `query` slot at `"Palo"` and exactly one
match, `station-paloalto`; tapping it makes the station "Palo Alto" and
shows home; a wheel of 300 over the content moves it by **exactly 300** and
exactly one scroll container took it; the GPU module loaded (web, polled up
to 3 s — module load is host I/O); the journal has its boot line, dropped
nothing, holds no refusal, and the host reported no error. Then the LLP 1010
fixture through `--plan`: a wheel of 100 over the scroll node scrolls it by
exactly 100 with the page unmoved; twelve wheels of 400 stop it at
**652** — a parity number pinned on both hosts, it moves if either host's
text metrics or padding do — with the page scrolled past its top. (A canvas
step, 6a and 9, belongs to LLP 1014.)

Measured 2026-08-29 on this machine, printed by the smoke and
`scripts/metrics.mjs`, not asserted: web smoke 3.6 s, boot 12.2 ms to the
first frame under agent mode (WebGPU on); macOS smoke 1.9 s, boot 100–160 ms
(the AppKit floor); `node scripts/agent.mjs macos tree` 0.46 s including
launch. The app wasm: 418 002 → 431 782 bytes for the r1 agent code (+13 780
B = 13.5 KiB; gzip 177 165 → 183 159), measured by building HEAD
(`aba1a62`) in a scratch worktree with the same `web` profile and
`wasm-opt -Oz`; the r2 code adds ~1 KiB. Whether the agent read operations
should be a second artifact rather than an export of every normal wasm is
open (§8).

## 6. Decisions

- **One delivery path.** Agent `tap`/`type` are real input on both hosts.
- **The driver owns time in agent mode; nothing the runner or motion owns
  moves between operations.** No snapshot tokens, no refs, no identity
  envelope: `tree` carries `epoch` and `incarnation`, a stale id is a typed
  refusal in the journal. Host I/O (§2) is the declared exception.
- **No tiers.** All eight on every host, or the host is incomplete.
- **`clock` replaces `wait`.** `settle` is a bounded fixed point, one call.
- **Reads from the substrate.** `tree`/`state`/`logs` from the runner and
  kernel; `layout` from what the host renders — its agreement across hosts
  is a parity check (652), its disagreement a finding. (Research: exact1's
  0495 §4.1 names the parallel reconstruction as the defect class.)
- **No views layer, no server, no generated skill.** One script and a
  paragraph in `AGENTS.md`.

## 7. The transcript form

The consumer is mostly a language model and it reads the driver's
rendering, so that rendering is part of the interface and there is exactly
one: `render(op, reply)` in `scripts/agent.mjs`, a pure function of the
reply, lossy on purpose (only `text`, `value`, and `label` ride along;
`placeholder` and the rest are in the JSON), **never parsed back** — a
review rule, since no check can hold it: the library returns objects, the
CLI splits argv, and nothing reads the outline in.

```
tree     epoch E · incarnation I · clock C ms · N nodes
         {"  " × depth}{Type}#{id} [{testId}] "{text}" value="…" label="…" ({handlers, ", "-joined})
layout   viewport W×H · clock C ms
         #{id} [{testId}] {Type} {x},{y} {w}×{h} scroll {sx},{sy}
logs     "(N earlier lines dropped by the journal ring)" when dropped > 0;
         the journal lines as they are; the host's lines indented two spaces;
         "(nothing new)" when there is nothing at all
state    the JSON, indented two spaces (JSON.stringify(reply, null, 2))
others   the JSON on one line
```

A part in `[brackets]` appears when its field is present — `!= null`, so an
empty string shows (`label=""`); `(handlers)` when the array is non-empty;
`scroll` when `sx` is present. `text`, `value`, and `label` are JSON-quoted;
`testId` and `Type` are not (they are identifiers). The fixture:
`scripts/fixtures/transcript.json` is an object of named samples, each
rendered as `--- name` + newline + the rendering, joined by blank lines,
with a final newline; a sample named `empty` or `dropped` is a `logs`
reply, the rest are named by their op. `scripts/smoke.mjs` checks it before
opening a host; `--record` rewrites it after a deliberate change. The input
grammar — `tap <target> [wheel <dx> <dy>]`, `type <target> <text…>`, `clock
<ms|+ms|settle>`, `screenshot <png> [window]` — is the whole of it.

## 8. Review record, not in v1, open

**Folded from the round-1 reviews** (both families, both code and spec):
animations frozen at every batch, not only at `clock`; timer-started motion
attributed to the timer's time on both hosts (`advance_timed`, `at`
markers, per-commit engine seeks); the clock landing where the runner says,
refusal and all; `settle` as a fixed point; the GPU module on the agent's
clock; `exact.agent` only in agent mode; the top-level JSON scanner;
`since` validated; `from` clamped and surfaced with `dropped`; unknown
actions journaled; poison labeled once; commands journaled after commit;
wheel deltas bounded; `layout` with transforms, sorted, rounded; `ready`
once the window is key; the file server's prefix check; the DevTools pipe
failing pending calls, with deadlines; `openMac` refusing a boot error; the
smoke's claims made exact and the wall-clock retry removed; the ring test at
the runner; this document.

**Not in v1:** a contract fixture with a `transition` under `clock` (the
freeze and the seek are held by the parity harness and by the batch tests,
not yet by the smoke); a drag form of `tap`; keys beyond `type`'s value
replacement; the macOS app taking focus while a script runs; `screenshot`
on macOS seeing Metal without `window`; the display link idling under agent
mode; replaying two timers' writes to one animatable row within a seek;
attaching to an already running app (a session is a process; the dev loop
wants the same resident channel).

**Decided (Charlie, 2026-08-29):** the agent read operations stay in every
normal wasm — a read of the runner's own memory has no second artifact to
live in, and a second build of the app wasm would be the build matrix the
rules forbid. Baseline to watch: at `e0a69bb` the agent code is 13,780 B of a
431,782 B wasm (3.2%; 6 KiB of 181 KiB gzip); if it passes ~5% or ~25 KiB,
revisit. Measured by building the previous commit in a scratch worktree with
the same `web` profile and `wasm-opt -Oz`; `scripts/metrics.mjs` prints the
total. A second review round on r2 was skipped for now (Charlie, 2026-08-29:
"skip it for now"); the Linux host's author is r2's next reader.
