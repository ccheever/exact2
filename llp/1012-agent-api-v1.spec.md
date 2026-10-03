# LLP 1012: Agent API v1 — nine operations, the clock in the agent's hands, as built

**Type:** Spec
**Status:** Draft (r2, unreviewed. r1 was reviewed by two families 2026-08-29 — `llp/reviews/1012-agent-api-v1.{codex,grok}.md`, both NOT READY — and the code by the same two — `llp/reviews/code-2026-08-29-agent-api.{codex,grok}.md`; r2 folds both, and the code they describe changed under them: §8.)
**Systems:** Runner (`agent.rs`, the journal, `advance_timed`), Web host (`exact_agent`, `at` markers, `glue.js` agent mode), Apple host (`exact_agent`, timed motion sync, `Agent.swift`), Tooling (`scripts/agent.mjs`, `scripts/smoke.mjs`)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Revised:** 2026-08-29 (r2: after the round-1 reviews — the clock made true on both hosts, the contract a host implements, the smoke walked back to what it checks, the private messages named, the numbers dated)
**Implementer:** Claude (Fable 5); landed 2026-08-29 (this document transcribes it)
**Related:** `rules/DEFERRED.md` §Agent API (the nine; `clock` replaces `wait`), LLP 1002 D3 (the clock is a seek), LLP 1005 §6 (`dispatch`, `advance`; `act` is for tests), LLP 1007 §3 (the web glue), LLP 1008 §4 (the C ABI this adds one call to), LLP 1010 §5 (the scrolling the smoke holds), LLP 1011 (the image whose box it holds); research: exact1 `llp/0495-acto-on-the-substrate.rfc.md` §4.1

## Summary

An agent drives the app through **nine operations** — `tree · screenshot ·
tap · type · state · layout · logs · clock · prefer` — the same on every host, with
**time in its hands**: between two operations nothing the runner or the
motion system owns moves, and instead of waiting the agent seeks the clock.
`tree`, `state`, and `logs` are answered once in the runner behind one export
on each ABI; `layout`, `screenshot`, `tap`, `type`, and `clock` are the
host's, because they are about what it renders, its input path, and its
clocks. One driver carries both hosts (headless Chrome over the DevTools
protocol on a pipe; the macOS app over stdio); the smoke is a script of the
operations that runs unchanged on both. Where this document and the code
disagree, the code and its tests are the authority.

**Physical iOS carrier (Codex, 2026-09-07):** `ios --device` / `open({host:
'ios', device:true})` selects the paired phone (`--phone` / `phone` chooses one).
The signed app is installed first by `host/apple/build.mjs --device`.
`devicectl --console` launches and captures diagnostics, but its stdin is EOF
on the tested phone. Instead, `EXACT_AGENT=1`, `EXACT_AGENT_CONNECT` and a random
per-launch `EXACT_AGENT_TOKEN` opt into an outbound connection after first pixel.
The driver binds a temporary port on the Mac's IPv4 LAN address (`EXACT_AGENT_HOST`
overrides selection), validates the token, then carries the same JSON lines.
No listener opens on the phone. This is a trusted-LAN developer carrier, not TLS
or an internet control API; allow local networking and keep the app foregrounded.
Agent startup disables UIKit's idle timer for that test process only, since
agent input does not reset the user-idle timer; normal launches are unchanged.
Host-local plan/assets paths refuse: use embedded artifacts or `--url`.
Only explicit launch environment crosses to the phone. Screenshots are written
in the app's temporary directory and copied back with its app-container file
service. The iPhone 17 Pro Max / iOS 26.6.1 passed the guard app drive, state/log
assertions and screenshot copy; a second run passed in 5.7 s. Physical live-URL
replacement, refusal and recovery now pass alongside the browser (1027 D6).
Two device crash reports identify UIKit's delayed-touch queue, not a locked
phone: the dev-menu recognizers now leave touch endings undelayed, and the full
guard proof passes with them enabled. Caltrain subsequently reproduced the same
crash despite that mitigation; its full URL proof passes with `EXACT_DEV_MENU=0`
(1027 D6). Follow-up: disabling touch delays/cancellation on Caltrain's hover
observers, plus direct-touch-only menu admission, passes two full Caltrain URL
proofs with the menu enabled, finally without tracing. Manual gesture recognition
is still owed. **Correction from real touch testing (2026-09-07):** Charlie
opened the client through Safari and reproduced the same nil-insertion crash on
first touch/scroll, including with `EXACT_DEV_MENU=0`. Those mitigations are not
a fix. The iOS agent invokes the app's input path without synthesizing UIKit
finger events, so its passing drives do not verify this gesture path. Root cause
remains unconfirmed; debugger attachment also failed. Other prior timeouts are
not all attributed.

**UIKit reduction (Charlie approved; Codex, 2026-09-07):**
`node host/apple/touch.mjs --device --run` installs the separate
`com.exact.touch-repro` app; `--phone <id>` chooses the device, `--case 0..4`
chooses its starting case. Without `--device` it uses the simulator (`--sim`).
`ios/touch.swift` links UIKit alone: plain buttons in a scroll view, then reduced
Exact hit/press handling, hover observers, four-finger window recognizers, and
replacing rows during scrolling. Cases are cumulative and selected with Next
case. Console and container `Documents/touch.log` record cases, real touch phases,
tap callbacks and scroll starts. No private API, synthetic touch, Exact runtime,
GPU, WebKit, module or URL loader is present. This is an approved diagnostic,
not a sixth check or proof that the full Exact presenter matches the reduction.
Physical build/signature/install/launch and simulator build/launch pass; the
simulator screenshot was inspected. The physical plain-UIKit case recorded seven
row taps and 15 scroll starts without crashing (PID 34926; saved log
`/tmp/exact-native-crash.xvrqhW/touch-baseline.log`). Charlie subsequently tried
all five cases and reported no crashes, including row replacement during dragging;
the complete log also records four-finger menu recognition
(`/tmp/exact-native-crash.xvrqhW/touch-all-cases.log`). This rules out none of the
full presenter's interactions: the approximated input patterns alone did not
reproduce the fault.

**Production-view reduction:** `node host/apple/touch.mjs --device --presenter
--case 5 --run` compiles the actual ExactKit Swift sources into this same
diagnostic executable and links the already-built client archive (`--archive`
overrides the platform's `libcaltrain_ts_apple.a`). Case 6/7 (CLI case 5) creates
the real Presenter, PlainView, NodeView and nested ScrollViews without creating
an ExactApp/session/runtime; its row captions are UILabels. Case 7/7 (CLI case 6)
creates a full ExactSession/ExactView and boots the bundled, compiled
`scroll.contract`. Neither has menu gestures, a URL connection, an update-store
adapter, GPU canvases or web nodes. The archive is linked in both cases, so
process-level native initializers are not excluded. Cases 1–5 remain available
as same-binary controls. The starting case is baked into the diagnostic plist
as well as passed at launch, so opening from Search/Home no longer changes the
starting experiment. No production source modifications or core feature flags.
Fixture boot failures display an error and never mount ExactView (which would
otherwise fall back to the baked Caltrain plan). First simulator trial exposed
a stale archive/schema mismatch; that is not a gesture result. Rebuilding the
simulator archive with `IPHONEOS_DEPLOYMENT_TARGET=17.0` produced a clean link and
case 7/7 booted with `views=84 error=none` (PID 27595); both simulator screenshots
were inspected. Physical case 6/7 installed and launched as PID 35216; real touch
phases were recorded without an exception so far, but manual outcomes are pending.
That initial phone build predates the fixture-refusal guard: require the case 7/7
console's 84-view/error-none confirmation before attributing its result. Five repo
checks passed, plus default UIKit-only Swift typechecking and presenter compilation.

**Physical full-session control passed (Charlie, 2026-09-07):** a direct launch
at 16:56:43 opened case 7/7 as PID 35523. Console confirmed `views=84 error=none`,
recorded real touches on both the nested rows and outer page, and recorded cycling
to the next case afterward. Charlie reported no crashes; saved container log:
`/tmp/exact-native-crash.xvrqhW/full-session-phone.log`. This makes startup timing
a concrete next comparison: ExactIOS (and ExactHostIOS) create their sessions
before UIApplicationMain, whereas the diagnostic created its session in
viewDidLoad. Session initialization creates the presenter's UIKit scroll view.

`touch.mjs --device --presenter --case 6 --early-session --run` moves only session
creation ahead of UIApplicationMain; the fixture still boots in viewDidLoad.
Its title is **Early session · scroll plan**; Next case is disabled because
cycling would not repeat the early initialization. The choice is baked into the
diagnostic plist for Search/Home launches; `TOUCH_EARLY_SESSION=0` overrides it
for a same-binary late-start control. Device PID 35540 and simulator PID 79313
logged early creation before entry to UIApplicationMain, then 84 views/error none.
**Timing result:** early-start PID 35567 crashed on its first touch with the
same `_delayTouchesForEvent:inPhase:` nil insertion as Caltrain (saved
`TouchRepro-170357.ips`, launch 17:03:21, crash 17:03:56). The exact same installed
binary relaunched with `TOUCH_EARLY_SESSION=0` as PID 35577, booted 84 views/error
none, and accepted repeated inner/outer scrolling; Charlie confirmed scrolling
the gray list. Saved log: `timing-ab-phone.log` in the evidence directory.
This isolates pre-UIApplicationMain session construction as a reproducing cause,
not the menu, URL, GPU or TypeScript execution alone.

**Targeted fix (2026-09-07):** ExactIOS and ExactHostIOS now create sessions in
`application(_:didFinishLaunchingWithOptions:)`, before their views boot but
after UIKit startup. Agent clocks are still set before boot, and the sample's
session list is populated before either pane mounts. No gesture changes or
private API. Both simulator products build; the standalone phone build was
installed and launched normally with menu/graphics enabled. Five repo checks
passed. Simulator host-ios smoke passed in 3.6 s (independent sessions, native
navigation, refusal, destruction); the full TS Caltrain app drive passed in 8.3 s
with all three Contract tests and an inspected screenshot
(`fixed-caltrain-sim.png` in the evidence directory). Charlie then reported
"seemed ok" after the request to scroll and try Change station in the rebuilt
regular Caltrain client (2026-09-07). This is real-app touch acceptance, separate
from the diagnostic control and scripted drives; four-finger menu recognition
and cold/warm URL usability on the repaired client remain separate checks.

Next reduction uses the installed Caltrain TS binary with the existing
`contract/corpus/scroll.contract` compiled and copied to its Documents directory,
then launched normally with `EXACT_PLAN` pointing to the phone-local file and
`EXACT_DEV_MENU=0`. No agent mode, live connection, GPU canvas or web node; the
linked TypeScript runtime remains. Screen: Above, a gray numbered-row scroll,
then Below labels. This reuses the production presenter and existing file-plan
fixture path without changing Caltrain's source or installed binary.
**Result:** the console attached to this launch recorded the same delayed-touch
nil-insertion exception. Saved `ExactIOS-160257.ips` in the evidence directory
identifies PID 35083, launched at 15:54:26 and crashed at 16:02:56, matching the
file-plan launch rather than a later Search relaunch. A fresh `EXACT_SMOKE=1`
launch at 16:07:34 printed `84 views; root 440x1372; error none`, confirming the
intended fixture loaded, then hit the same exception before the smoke completed.
Charlie also reported crashes after opening through Search; those launches alone
do not identify the file-plan fixture, since launch environment is not persistent.
The real presenter can reproduce without Caltrain's canvas, web nodes, URL or menu;
its scroll hierarchy and linked runtime remain to be isolated. Not a fix.

Device requests fail after 45 s with recent native diagnostics and close the
connection; the carrier remembers failure so subsequent reads reject immediately.
Malformed,
oversized and wrong-token handshake peers refuse. No ninth operation or automatic
phone fallback. Simulator and macOS carriers retain their existing transports.

**Apple request scheduling (Codex, 2026-09-07):** the reader waits for each
request to finish on the main run loop, using `CFRunLoopPerformBlock`, rather than
entering a synchronous main-dispatch-queue block. The latter starves main-queue
WebKit snapshot completions during nested run-loop waits. Requests remain serial
and main-thread-owned. Full TS Caltrain drives pass on macOS and the physical
iPhone, including guest pixels in the copied screenshot (1020 D4).
The block is scheduled in **common modes** (2026-09-10): default-mode-only
requests stalled while UIKit tracked a held sheet drag and answered only after
release. The reader still serializes requests and the main thread still owns
them. A physical compose-sheet drag now permits `layout`, `state`, editor
inspection and `screenshot` before reversal/release, with a visible software
keyboard, at two Simulator window positions. Screen geometry reports the held
sheet; a viewport-only screenshot retains its existing crop. This fixes reads
during native tracking, not the iOS carrier's still-unsupported synthetic contact
phases (LLP 1035.003). Evidence: `/tmp/messages-held-inspection/`.
An OS-opened macOS bundle also exposed a startup ordering bug: becoming key can
synchronously announce agent readiness before the later global initializer reset
its guard, starting two stdin readers. The guard now initializes before ordering
the window. Cold/warm OS opening passes with one ready announcement and serial
responses; the public operations are unchanged.

## 1. The operations

Public: what `scripts/agent.mjs` exposes as a session (`open({host, plan,
size})`), and what the CLI runs one per argument. A target is a `testId`
(first in preorder) or a view id; the driver resolves it through `tree`, so a
host input path only ever sees a view id. LLP 1038 D5/D11: on native, `--url` with an
app scheme or path supplies the cold launch location; HTTP(S) retains the
development-plan locator form only and never supplies a launch location. Apple uses the
same pre-boot fact as the OS callbacks; Linux receives the URL as argv.

`tree <target>` (library `tree(target)`) returns the target and its descendants.
The wire request adds `target`, a numeric view id or string `testId`; repeated
`testId`s select the first node in live structural preorder. Missing, retired,
and malformed targets are refused. `roots` names the selected node; node fields,
including the real parent and absolute tree depth, match the full response.
The text renderer removes only the common leading indentation. Without a target,
`tree` still returns every live node. For a target, wire `shallow: true`
(library `tree(target, {shallow: true})`) includes only the selected node's
record. Its real parent, absolute depth and child ids remain unchanged; the
descendants' records are omitted. A shallow read requires a target, and the
flag must be boolean. `find`, and therefore input and targeted-layout lookup,
use this read without exporting the descendants. Input still sends the resolved
view id through the existing host input path.

Each multi-node `tree` response gathers live handler declarations in one
instance-tree walk; a single-node response uses point lookup. Handler order and
empty lists are unchanged; no handler map survives the response, so branch and
collection changes are observed afresh.

| op | request to the host | reply | who answers |
|---|---|---|---|
| `tree` | `{"op":"tree"[,"target":V or "testId"][,"shallow":true]}` | `epoch`, `incarnation`, `clock`, `roots`, `nodes[]` in preorder: `id`, `parent`, `depth`, `type` (schema name), `props` by schema name, `handlers` (`press`/`change`/`hover`/`focus`/`blur`/`key`), `children` | runner (kernel topology + props) |
| `state` | `{"op":"state"}` | `epoch`, `incarnation`, `clock`; `slots`, `derives`, `resources` by declared name as typed JSON: records keyed by field name, `none`/unit `null`. **Then the host's three sections (2026-09-10, LLP 1035.002 D2)** — observations of its view tree, never a second model: `focus{logical, editor, responder, pending}` (the node holding the platform's focus, the editor when it is one, the responder's class, a focus a sheet is still holding for its presentation), `keyboard{visible, overlap, top?, guide?, policy, interactive}` (the software keyboard's overlap with the viewport, its top edge and the layout guide in the viewport's space where the host has them, the `interactiveWidget` policy, a drag dismissing it), `navigation{url, route, stack[], presentation, closedby, transition{interactive, phase}}` (the last `router` op's canonical URL or null when absent (LLP 1038 D11), the route the root names, the platform's stack by key, `modal` or null, the close policy, and `idle`｜`in-progress`｜`cancelled`｜`completed`). UIKit reports its first responder, keyboard and navigation controller; AppKit its first responder, no keyboard, the stack as the rule's prefix; the page `activeElement`, `visualViewport` and the DOM's routes; Linux each section as `{"unavailable": true}`, present so "no keyboard" reads apart from "no report" | runner (the plan's type table) + host (the sections) |
| `logs` | `{"op":"logs","since":N}` | `next`, `from`, `lines[]` — the journal from `since` (§3); the driver adds `host[]` (page console / app stderr) and `dropped` | runner |
| `layout` | `{"op":"layout"}` / `{…,"id":V}` | `clock`, `viewport{w,h}`, `env{…}` (2026-08-30: the page's environment by the web's `env()` names — `safe-area-inset-top/right/bottom/left`, the insets the host gave the kernel under `viewport-fit=cover`, and `keyboard-inset-height`, a software keyboard's overlap with the screen's viewport; under `interactive-widget="resizes-content"` `viewport` itself shrinks to the keyboard's top, as Chrome's `innerHeight` does; zeros on macOS and Linux, the browser's own on the web; **and the fold (2026-10-02, LLP 1078 D7)**: `device-posture` (`continuous`｜`folded`), `horizontal-viewport-segments` and `vertical-viewport-segments` (integers from 1), and `viewport-segments` (`[x, y, w, h]` per segment in viewport points, row-major, empty for one segment) — UIKit 27.1's division regions and hinge on iOS, the browser's Device Posture API and `viewport.segments` on the web, `continuous` 1×1 on macOS, Linux and older iOS unless `prefer posture`/`prefer segments` said otherwise), `nodes[]`: `id`, `x`, `y`, `w`, `h` (+ `sx`, `sy` on scroll containers); the driver adds `type` and `testId`. **With `id` (2026-09-09, LLP 1035.002 D1, `layout <target>` on the CLI): `node{…}` explains that one node** — the runner's half (`epoch`, `incarnation`, `site`, `instance`, every row it sets or inherits as `{value, source: authored | inherited (+from) | initial}`, `props`, the kernel's `frame` in the parent and `absolute`) merged with the host's (`space{viewport, local, window?, screen?, capture{scale}}`, the `scroll` and `clip` chains outermost first, `visible{hidden, inert, inViewport, clipped}`, `native{…}` — what was mounted; the web adds `browser{…}`, its own computed values for the inherited rows). A space a host cannot observe is absent; a stale id is refused by name | runner (the node's rows and sources, the private `node` message) + host (the spaces) |
| `tap` | `{"op":"tap","id":V}` / `{…,"wheel":[dx,dy]}` / `{…,"hover":true}` / `{…,"history":n}` on the navigation root (LLP 1038 D11; CLI `tap <root> history -1` or `tap <root> {"history":-1}`) / **a contact's phases** (2026-09-09, LLP 1035.003 D1): `{…,"phase":"down"[,"x","y"]}`, then `{"op":"tap","phase":"move","x","y"｜"dx","dy"[,"ms"]}`, `"hold"[,"ms"]`, `"up"`, `"cancel"` — CLI `tap <target> down [at <x> <y>]`, `tap move <x> <y>｜by <dx> <dy> [over <ms>]`, `tap hold [<ms>]`, `tap up`, `tap cancel`, the phase words read as phases only while a contact is down | `tapped`, `at` (+ `hover`); a phase replies `phase`, `at`; **every input reply carries `delivery`** — `platform` (a real input event through the platform's path: CDP mouse/touch, `NSWindow.sendEvent`), `recognized` (an already-recognized event injected: iOS `contextmenu`/`dblclick`/`hover`), `activation` (a hit-test and a direct call: iOS `tap`), or `unsupported` — plus `carrier` and `mode`, added by the driver from the host's answer and its own table (D2); the driver adds `target`. On iOS (2026-08-30) a tap also does what a touch up does first: the nearest node that takes the focus takes it, and when none does the field being edited is blurred (LLP 1008 §9); a phase on iOS or Linux is `unsupported`, never an activation dressed as a finger | host input path (the web: CDP touch events under touch emulation, switched on by the first contact; macOS: the mouse button held across requests, a timed move as dragged events with the run loop turning between them, `cancel` unsupported because AppKit has none; the simulator (2026-09-10): a real mouse on the Mac's desktop posted into the Simulator's window by `host/apple/pointer.swift`, the mapping calibrated by hovering at two desktop points and reading `layout.pointer`, refused with the covering app's name when the Simulator's window is not topmost at the point; a phone: unsupported) |
| `type` | `{"op":"type","id":V,"text":…}` / `{…,"key":"Enter"}` | `typed` (+ `value` on macOS) / `key`; the driver adds `target`. On the navigation root, text is a location: `type <navigation root> "/post/42"` dispatches `navigate` once, with `delivery: recognized`, without focusing an editor (LLP 1038 D11); the same target resolution applies | host text or navigation event path |
| `clock` | `{"op":"clock","to":ms}` / `{…,"settle":true}` | `clock` (where it landed), `settled` for `settle`; under `EXACT_AGENT_TIMING=platform` (2026-09-10, LLP 1035.003 D5 — `open({timing:'platform'})`, `--timing platform`: UIKit's push/pop, sheet and keyboard animations keep their natural timing while the driver still owns the runner's clock) `settle` also waits, bounded at two seconds, for the iOS navigation and modal hosts to leave a transition, and replies `settled: false, reason: "transition"` past the bound | host, both clocks |
| `screenshot` | `{"op":"screenshot","path":…}` (+`"window":true` on macOS) | `screenshot`, `w`, `h` (viewport points / CSS px, not PNG pixels; `scale` for a window capture) | host |
| `prefer` | `{"op":"prefer","media":{"prefers-reduced-motion":"reduce"}}` — CSS's media feature names: `prefers-reduced-motion` and `prefers-reduced-transparency` (`reduce`/`no-preference`), `prefers-color-scheme` (`dark`/`light`); an unnamed feature stays; CLI `prefer <feature> <value> […]` (2026-09-27, LLP 1061 D5); **the fold (2026-10-02, LLP 1078 D7)**: `{…,"fold":{"posture":"folded","cols":2,"rows":1,"gap":40}}`, CLI `prefer posture folded｜continuous`, `prefer segments <cols>x<rows> [gap <points>]` — a host without a fold splits its viewport evenly with the gap centred on each divider; a host with a real fold (a Duo on 27.1) refuses: the device decides; `0x1`, `1x0` and a gap wider than the viewport are refused by name | `media{…}`: all three as the host now reports them; `fold{…}`: the four `layout.env` names | host (the web: `Emulation.setEmulatedMedia`; Apple: the accessibility settings replaced for the process, the window scene's style or, on macOS while the app follows the system, `NSApp.appearance`; Linux: the runner and the painter's system scheme). An unknown feature is refused and nothing applies |

Errors are `{"error":"…"}` on the wire; the session throws `"<op>: <message>"`.

**Every reply but `logs` is tagged** (2026-09-10, LLP 1035.002 D3): `epoch`,
`incarnation` and `clock` — the runner's own replies at their source, a
host's (`layout`, `tap`, `type`, `clock`, `screenshot`) read after the
operation through the runner's `{"op":"tags"}` message, so the tags name
the world the reply left behind; a `clock` reply keeps its own `clock`. The
driver stamps the web carrier's CDP-delivered input and captures the same
way (`tagged` on the session). `logs` is the journal, whose lines carry
their own `t=` clocks; an error carries no tags. A targeted read of an id
that is not live in the current incarnation is refused by name.

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
  containers. On iOS, `ox`/`oy` measure elastic displacement beyond the
  scroll view's adjusted inset bounds. A valid negative resting offset is not
  overscroll. Wheel routing uses those same bounds, including reachable top,
  left, bottom and right insets; an exhausted child chains to its ancestor.
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
  would. With `hover: true` (2026-08-30), the pointer moves onto the box's
  center and stays there — CDP `mouseMoved` on the web (the browser fires
  the enter/leave pair); on macOS and iOS the presenter's own hover path from
  the hit view up to the first node with a `hover` handler, leaving whatever
  was hovered (no public pointer synthesis on iOS; on macOS the same for
  symmetry — a tracking area needs the window's real cursor). CLI:
  `tap X hover`. Browser Back/Forward (LLP 1038 D11) is `tap <navigation
  root> {"history":-1}` / `{"history":1}` (CLI also `history -1` / `history 1`).
  The web calls `history.go(n)` and handles its real `popstate`; `n` must be a
  nonzero integer. All other host sessions reply `delivery: "unsupported"`
  without dispatching a control. No ninth operation is added.
- **`type`** sets an input's whole text as a paste does: select all, insert.
  Web: `focus` (a page-side helper, §1 private) then CDP `Input.insertText`;
  macOS: first responder, the field editor's `selectAll` + `insertText`. One
  `change` with the whole value; a non-input target is refused on both.
  Key presses preserve the editor’s existing cursor/selection; only whole-value
  replacement selects all first. On iOS, Backspace calls the actual field’s
  `deleteBackward`, including its authored key event and normal text deletion.
  With `key: "Enter"` (2026-08-30), one key down (and up) on the target by
  the web's name: web CDP `Input.dispatchKeyEvent` after focusing the target;
  macOS a synthesized `NSEvent` key-down/up through the window with the
  target first responder; iOS direct delivery (§9 of LLP 1008). CLI: `type X
  key Enter` — which, at an input with a `submit` handler, is the web's
  implicit submission on every host. Not on the Linux carrier yet (its lane).
- **`clock`** is monotonic (a backwards `to` is refused). It moves the
  runner's clock and the host's motion clock to one instant and **lands where
  the runner says** (`batch.clock`): a timer's refusal stops the advance at
  that timer's due time, the commits before it are shown, the refusal is the
  reply's error. `settle` is a fixed point: advance to when the last thing in
  flight ends — the motion engine's `settle` (a private read on the ABI), and
  on the web every `Animation`'s computed end — and if the timers crossed on
  the way started more, again; sixteen rounds, then `settled: false`. §2.
- **`screenshot`** is the rendered pixels: `Page.captureScreenshot`, or
  `cacheDisplay` of the viewport with every canvas painting its readback
  picture (as a capture does; a Metal layer alone would be blank — since
  2026-08-30; a pending capture is rendered at the agent's clock first, as
  `clock` leaves it — since 2026-10-03), or with `window` the window server's picture
  (`screencapture -l`, needs screen-capture permission and a display that
  is on).

**Private messages** are not operations: `focus` (web `type`'s first half),
`settle` (the engine's end time, read by `clock`), `node` (the runner's half
of `layout <target>`, 2026-09-09), `quit` (macOS stdio), and the `at` op
inside a batch (§2). **`exact_log`** (2026-09-10, LLP 1035.001 D6) is the
ABI entry beside `exact_agent` through which a host appends a line to the
journal (§3): what it refused and why. `exact.reload` / `EXACT_PLAN` boot a plan —
session setup, not a drive. `Runner::act` runs an action by name for **tests**
(LLP 1005 §6 now says so); an agent never takes it.

**Eight, and a wheel is a form of `tap`** (Charlie, 2026-08-29: "A"). A drag
would be another form. `rules/DEFERRED.md` §Agent API binds the count and
the trade — a ninth operation replaces one of the eight, same PR — and this
document cites it rather than restating it as a second law. **`prefer`
(2026-09-27, LLP 1061 D5) is the ninth** (Charlie, 2026-09-27: "relax the
rule and allow a 9th button"): a display preference is no input, so it is
not a form of `tap` or `type`. It is the operation that sets every device
fact (LLP 1069.007 §3 P1). **`perf` (2026-10-02, LLP 1079) is the tenth**, under
Charlie's waiver: a subtree's work by plan site and a host's presented frames.
An eleventh replaces one of the ten.

## 2. The clock

**Requests in flight (LLP 1016; 2026-08-30).** `clock settle` waits for
every request the runner has out before it measures the fixed point — the
reply commits, and may start motion or ask for more — and reports `settled:
false` if one is still out at the bound (twenty seconds on the native
hosts), with `reason: "requests"`. The bound is one deadline for the whole
call, not one per round (2026-09-24: per round, a request that never answered
held `clock settle` for minutes). `state` gains `pending: [{name, ticket}]`, the requests in flight by
the resource's or mutation's name. `clock` with a time waits only where a
real clock would have let a reply land first (2026-09-26): the runner keeps
one request per target (LLP 1016 D5), so in a jump a tick's send would drop
the reply of the tick before it. Before a timer due within the jump fires,
what is in flight lands; the runner stops the jump after each timer whose
commit hands out a request (`Runner::advance_until_request`), and the host
waits for the reply under the call's deadline and advances on. A jump whose
timers send nothing is one advance; past the deadline, or 4096 stops, the
rest is one advance; a request with no timer after it in the jump lands when
it lands, as a fetch does under a real clock.

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
finishing decoding (the kernel relays out; poll `layout`; `screenshot` on the
web, macOS and iOS, and the Apple `clock`, first wait up to 3 s for images on
screen, and a screenshot that stopped waiting says `imagesPending: n`), the GPU module
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
(`timer 0 (tick) refused: …`). Hosts append with `Runner::log` through
`exact_log` (2026-09-10, LLP 1035.001 D6): the iOS presenter journals a
`focus("id")` it could not deliver with its reason (`focus "to" refused:
not mounted | disabled | zero size | hidden ancestor | inert ancestor | no
live node with that id`), a root `navigationKey` that names no route (once
per key), a back gesture refused for want of an enabled `navigationBack`
control, and a modal route refused because the owner already presents
(once per route); the web page journals a `focus` whose element is missing.
Nothing a host refuses is silent.

`logs` replies `{"next":N,"from":M,"lines":[…]}` with `from = clamp(since,
journal_start, next)`: a reader whose cursor the ring has passed gets the
suffix and a `from` above its `since`. The driver surfaces it as `dropped =
from − since`, and the transcript form prints a marker line. A non-numeric
`since` is an error.

## 4. Where the code is

`--app <name>` (2026-08-30) names the app the native carriers open — its
assets, its Linux binary, its bundle id — through `scripts/app.mjs`
(`resolveApp`); with `EXACT_APP_DIR` set it is an app outside this repo
(weird-castle's `node exact.mjs agent …`). The web carrier needs nothing: `dist/`
holds whatever was built last.

Carrier deadlines are cancelled when their operation settles. A completed
startup, frame wait or shutdown leaves no timeout keeping the driver alive;
an operation that does not settle still takes its existing bounded fallback.

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
`nowMs` moved by 60 000, and the journal shows that seek as one advance
firing sixty timers (a jump stops only at a timer that sends, §2);
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
text metrics or padding do — with the page scrolled past its top. Then the motion fixture
(`contract/corpus/motion.contract`: three boxes scaling 1 → 2 — a 250 ms
linear transition and a `spring(180, 12, 1)` on a press, a 500 ms linear
transition when a timer fires at 1000): after the press, two `layout`s with
a 300 ms wall pause between them are identical (50×50 — frozen at local
time 0); `clock +125` gives the linear box **75** wide and the spring box
**86.55** on both hosts; `clock +125` gives 100; `clock settle` is a
two-round fixed point landing at exactly **1500** (the spring settles at
1295.8 ms on both hosts, the seek there crosses the timer, whose transition
ends at 1500), after which both boxes are 100; and in fresh sessions, one
seek to 1250 then 1500 across the timer gives 75 then 100, the same as
stepping 1000, 1250, 1500 (50, 75, 100). (Canvas steps — 6a, 9, 10 —
belong to LLP 1014.)

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
- **No tiers.** All nine on every host, or the host is incomplete.
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
layout   viewport W×H [· safe-area T R B L · keyboard K, when any is not 0] [· posture P · segments C×R [x,y w×h]…, when not flat] · clock C ms
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
grammar — `tap <target> [wheel <dx> <dy> | hover | contextmenu | dblclick |
down [at <x> <y>]]`, `tap move <x> <y>｜by <dx> <dy> [over <ms>]`, `tap
hold [<ms>]`, `tap up`, `tap cancel` (the four phase words only while a
contact is down), `type <target> <text…>`, `clock <ms|+ms|settle>`,
`screenshot <png> [window]`, `screenshot <png|apng> over <ms> every <ms>` (film on
the clock, LLP 1012.001.000 D2), `layout [<target>]` — is the whole of it. `layout <target>` (2026-09-09, LLP 1035.002) renders its
`node` as a block under the listing — `node #id [testId] Type · site ·
instance · epoch · incarnation`, one `row = value (source)` line per row,
then `space`, `scroll`, `clip`, `visible`, `native` and, on the web,
`browser` lines, each present only when the host reported it (`renderNode`
in `scripts/agent.mjs`; the fixture's `layout` sample carries one).

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
the runner; this document. Then the motion fixture (§5), which found one
more: the web registered animations a timer's ops started only after the
clock had moved to the batch's landing time, so a seek across the timer
still bore them at the destination — `applyBatch` registers before it
moves the clock.

**Not in v1:** ~~a drag form of `tap`~~ (the contact phases landed 2026-09-09
for the web and macOS carriers, LLP 1035.003; iOS and Linux answer
`unsupported` until a backend exists); keys beyond `type`'s value
replacement; the display link idling under agent mode; replaying two timers' writes to one animatable row within a seek;
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

## Amended (LLP 1018, 2026-08-30)

`state` gains `"store":[…]` — the names the runner's store holds a value for,
never the values (a token is not the agent's to see); the journal carries
`store <name>` / `forget <name>` lines once a commit stands. Agent mode on
every host boots with an empty store and persists nothing (`?agent=1` on the
web; `EXACT_AGENT=1` on Apple, unless `EXACT_STORE=real`), so a drive is
deterministic and a smoke against real credentials leaves nothing behind.

## Amended (LLP 1069.007 first slice, 2026-09-27, as built)

- **Only where agent mode is admitted.** A build baked with
  `EXACT_UPDATE_TRUST=production` never enters agent mode (ruled). Native:
  the binary's own `compat.json` `inputs.trust`
  (`exact_runner::delivery::production`); Linux drops `EXACT_AGENT` and every
  `EXACT_AGENT_*` variable in `Config::from_env`, Apple in
  `ExactEnv.environment`, each before anything reads one, and says so on
  stderr. Web: every host file that reads `?agent` declares
  `AGENT_ADMITTED`, which `host/web/build.mjs` ships `false` in a
  production build (and refuses a file that reads `?agent` without it).
- **Held device requests.** A capability's arm under the agent holds its
  request with `Runner::hold` (a ticket from the runner's own counter);
  `state.pending` lists it after the network's as `{name, ticket,
  device:{capability, args}}`, `args` the capability's inspection summary.
  A hold is not I/O: it is outside `has_pending`, the web's `inflight` and
  Apple's `pendingCount`, so `clock +N` never waits on one. `clock settle`
  still waits for the network, then replies `settled: false, reason:
  "device", tickets: [...]` (every remaining ticket) while a hold remains.
- **`tap @N <choice>` / `type @N <value>`** (wire `{"op":"tap","ticket":N,
  "choice":"cancel"}`, `{"op":"type","ticket":N,"text":"…"}`) answer hold `N`,
  resolved by the host before any view: consumed once, a stale or retired
  ticket refused as `not pending: @N`, a network ticket refused by name. The
  reply carries `delivery: "substituted"`, a fifth value beside §1's (the
  driver also emits `presenter` and `browser-viewport`). A typed value is
  never echoed or journalled. The journal says `device <cap> <N> held
  (agent)`, `… answered: <choice>|a value`, `… cancelled`, `… retired` (its
  node removed). No capability is admitted yet: the proof is a synthetic
  hold in `host/linux/src/agent.rs`'s tests.
- **Launch facts.** Display preferences are the agent's from launch
  (`no-preference`, `no-preference`, `light`, and on the web `prefers-contrast:
  no-preference`), not the machine's until the first `prefer`; the UTC offset
  is told again after every `clock`, so a move across a DST change re-answers
  `exactTime()` (a timer fired inside that jump still reads the old offset).
