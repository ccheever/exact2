# Queue

- **Bounded resource answers** (Charlie, 2026-09-18, [LLP 1027.004](llp/1027.004-bounded-resource-answers.plan.md) r2): slices 1–2 are implemented. `list virtualized=true` has `reachstart`/`reachend`, Messages stress has a bounded mode, and the Messages conversation answer is at most 200 rows around a cursor. Still owed: the Grok code review (balance exhausted), a macOS/AppKit drive of the stress bounded mode, and the declared edge limitation below. Shared views/buffers/heap and generic whole-value reconciliation remain deferred (LLP 1027.003 §9).
- **Messages durable writes are O(total records)** (found by LLP 1027.004 r2 D5): every mutation JSON-clones the whole model (`snapshot()`) and `persist` diffs every key; startup and each sync page `restore` everything; Snapback's 512-record edit cap refuses deleting or recovering a conversation over 512 messages. Belongs with the in-progress Snapback migration: persist changed records only, and chunk or raise the cap for bulk deletes.
- **Linux Messages transcript scroll commands** (S2 drive, 2026-09-18; pre-existing at ccc5367): after 215 UI sends, authored `scrollTop=1000221` / `scrollFollowEnd=true` leave the headless presenter at `sy=0`, both with the base’s 225 rows and S2’s bounded 200. Repair the Linux property path and re-drive latest/send plus earlier/later shifts. The reply drive also fails at the base: the presenter loses its held contact, and the Linux carrier refuses context-menu input. Evidence: `target/bounded-answers/s2/round2/base-classification.json` and the base drive captures beside it.
- **Edge re-arming from provisional heights** (LLP 1027.004 D5, 2026-09-18): with the whole window visible and rows far below the 32 px estimate, re-measuring a replaced window counts the edge row as outside, re-arms it, and earlier/later loading can oscillate without scrolling. Re-arm only on a measured exit or after a reader scroll; add a 200-row, 1 px test that settles through measured feedback. Found by Astra's round-4 review of `e08e25a` (`llp/reviews/code-2026-09-18-bounded-answers.astra.md`).
- **Messages native test fixtures keep the production 100 ms executor budget** (2026-09-18): `snapback_tests` failed the blocking suite at `e08e25a` with `conversation took 105.5 ms, over the 100 ms budget` at a 5–15-minute load average of about 25. The same binary passed three quiet reruns in about 0.2 s. Functional fixtures should disable the wall-clock budget as `apps/messages/apple/tests/window.rs` does. That file is also edited by the paused Snapback migration, so land it with that migration.
- **Virtualize the Messages transcript** (LLP 1010 §6 consumer sweep; LLP 1027.004 D5): its horizontal timestamp-reveal wrapper is outside the collection's supported shape. Once virtualized, `reachstart`/`reachend` replace the explicit earlier/later rows with no source change.

What would make sense to do next, in rough order. This is not a spec and decides
nothing: `rules/RULES.md` §Scope says a spec without an implementer and a date isn't
written, so a line here is one to three lines — the thing, why, what it needs. When
someone picks one up it becomes an LLP with a name and a date (archive a link when
the working set is full); when it lands, delete the line. Anyone, agents
included, may add or remove a line. `rules/NOT-DOING.md` still binds: an entry that
sits on that list carries the trade it would take.

## Next, in order (2026-08-29)

- **Text around shapes** (LLP 1043.000 §8, as built 2026-09-19): `wrap-flow` / `shape-outside` exclusions flowed on
  both sides per frame on Linux, Apple and web through one shared walker (`textflow/`), demo `apps/textflow` (six
  scenes); vendored Taffy is upstream 0.14 (patches 3, 4, 5 retained). Owed: **auto-height flow** — M8's probe found
  `BlockContext` offsets are provisional at measure time (sibling margin collapse y=100 → 90, auto margins x=0 → 50,
  descendant collapse y=110 → 150), so stable offsets and wrapping-context identity must be settled before measurement
  sees them, or bounded re-layout used instead (Charlie's call; LLP 1043.000 §8 "Stage 0"); the wasm cost is ~+77 KiB against the ruled ~64 KB;
  Euclidean `shape-margin` for ellipse/polygon; `justify` in fragments; iOS hit-testing and any iOS run at all;
  Safari/Firefox; `shape-outside: <image>`; the 2,000-paragraph worst case regressed 5 → 54 ms a pass in the fix round.

- **Apple text: keep the line-break boundaries in `TextShape`** (2026-09-18; LLP 1043 F7): under
  `overflow-wrap: normal` `TextEngine.layout` re-runs `CFStringTokenizer` over the whole paragraph at
  every width (25–31 µs on a 79–120 µs snapshot, probe only); the boundaries depend on the text alone.
  Confirm in a Markdown resize trace first.
- **Internal event/property ID collisions** (Charlie, 2026-09-19): investigate assignment and cross-branch collision detection after navigation and video independently claimed host dispatch kind 14 (merged as navigation 14, media 19).
  Audit the manually mirrored Rust/Swift/browser dispatch codes alongside generated kernel property IDs and plan event ordinals; determine how to keep assignments consistent and detect incompatible host/plan pairs without adding another declaration authority.

- **Native verification gaps** (2026-09-15): Messages debug native tests can
  exceed their 100 ms data-call budget during workspace validation (105–241 ms
  on the Air); investigate without weakening the limit. Interview's new Mac
  materials still need Reduce Transparency/Increase Contrast and older-OS pixels.

- **Web driver shutdown** (2026-09-14): Caltrain web smoke and Interview web
  drive print success after closing sessions but leave Bun alive; find the
  remaining handle. Explicit exit after the smoke returns completes cleanly.

- **List memory** (2026-09-14, Codex; LLP 1010 §6): baseline at 25/1,000/25,000
  rows, then shared runner/web windowing, Apple/Linux, raster budget and Messages
  acceptance. Memory and construction cost trigger it now; further Messages
  decorative artwork/material/timing polish waits. The baseline does not complete windowing.
- **Game-engine comparison remaining gaps** (2026-09-17; LLP 1041.003 §7): full Lanterns games and fleet trials landed. Babylon/PlayCanvas qualify; eight external edit passes include one PlayCanvas repair-budget violation. Godot proves human win/loss but its sequential large-step adapter fails; Three.js/Cannon collision remains broken after the separately authorized extra sleep/reset round.
  Next: repair those two specific baselines before their withheld trials; measure target GPU/phone and native exports, complete adapter/setup cost, and actual vendor automation routes. Actively track Babylon + PlayCanvas; retain Godot/native and Three/renderer controls.
  Retained fixtures include a held-out change and negative controls; their repeated execution is not repeated agent success. Diagnostic only, no blocking check; LLP 1041.000 §6a remains the full-game bar.

- **List memory** (2026-09-18, Codex; LLP 1010 §6): fixed and measured-height
  runner/web windows and Apple geometry are implemented; Markdown now uses them,
  with logical selection/copy on Mac and web. Finish workspace validation after
  the macOS executable-startup block, Linux geometry, the full native interaction
  sweep, raster budget and Messages acceptance. Memory and construction cost trigger it now; further Messages
  decorative artwork/material/timing polish waits. The fixed-height web slice does not complete the native/consumer sweep.

- **Markdown scrolling, on the 120 Hz machine** (2026-09-19): scrolling now measures
  ahead of Legend on an M4 Pro at 60 Hz with input synthesized in-process
  (`apps/markdown/README.md`: inputs committed later than one 120 Hz frame, of
  2,400 — 624 before, 6 against Legend's 84 in the same sitting, 2 since AppKit's
  persistence was switched off). Owed, all needing HID and Instruments
  permission: the same comparison at 120 Hz with real wheel and trackpad input
  under the Hitches instrument, which at 60 Hz no longer separates the two (0, 2, 1
  hitches against Legend's 1, 0, 0, three of the four being the scroll's first
  frame in either app); responsive scrolling for a contained list, which cannot be
  driven from inside the process and is what keeps Legend's scroll off its main
  thread. Repeat first-content and memory with this build.
- **A list paragraph is typeset twice** (2026-09-19): in black to be measured, then
  in its colours to be painted, because `TextShapeKey` carries the paint; only the
  line breaks are handed over. 0.7 ms at the median and 3 ms at worst, since
  2026-09-19 a fill unit of its own. A shape whose colour comes from the context at
  draw time would serve both, for a paragraph of one colour at least.
- **A row of many inline runs is the longest fill unit** (2026-09-19): 7 ms at the
  99th percentile against 2.6 at the median, about 0.5 ms in the runner and kernel
  and 0.35 ms in the presenter per created view, and every inline run is a
  `NodeView` that is never mounted.
- **Rationed list reports on iOS** (2026-09-19; LLP 1010 §6): UIKit also scrolls on
  the thread that lays out; `PresenterIOS` still reports a whole window inside the
  scroll callback and paints text when first seen. The runner and host halves are
  shared (`list_viewport_within`, one-call settle); the presenter half is not made.
- **A jump lands on rows built synchronously** (2026-09-19): a scroller drag or a
  250,000-point jump builds a scrollport of rows in one report, 30–55 ms. The rows
  are never blank; the frame is late. Paint them first and fill the rest, or show
  the estimate's geometry for a frame.
- **The macOS smoke fails on a Mac that shows legacy scrollers** (2026-09-19): on an
  M4 Pro mini with a mouse, `smoke.mjs macos` over the Markdown app at untouched
  `6214c47` failed its scroll fixture in 22 of 24 runs (the scroll node stops at
  669, not 652: a 17-point scroller) and its motion fixture in 8 of 24 (the box
  never leaves 50 wide); with the scrolling work, 24 and 9. Nothing else failed.
  The fixtures assume overlay scrollers, and something in the run is timing.
- **The Swift test target did not compile at `6214c47`** (2026-09-19):
  `TextGeometryTests` still tested `TextCache`, removed by `e095df53`. The one test
  was deleted; nothing replaced what it covered, if `TextResidency` needs it.

- **Apple text: cache the line-break boundaries per spec** (2026-09-18; LLP 1043 F7):
  under `overflow-wrap: normal` `TextEngine.layout` re-runs `CFStringTokenizer` over
  the whole paragraph at every width (25–31 µs on a 79–120 µs snapshot, probe only);
  the boundaries depend on the text alone. Keep them beside `typesetters`; confirm in
  the Markdown resize trace first. LLP 1043 §5 lists what would reopen an arithmetic breaker.
  Since 2026-09-19 one tokenizer is shared (`TextEngine.lineBoundaries`), which removed
  making one per paragraph, a tenth of a first measure; the re-run per width is as it was.

- **Text around shapes** (2026-09-18; LLP 1043.000, Draft RFC, not in `current/` — the set is full):
  `wrap-flow` / `shape-outside` exclusions a paragraph flows around on both sides, per frame; kernel
  resolves them after layout, a shared `exact-textflow` crate walks lines, Apple can start on CoreText
  (a screenful at 1.1 ms a frame, measured). Ruled 2026-09-18 (§6): CSS names, always linked, Taffy to upstream 0.14 for
  auto-height (stage 0, its own lane), `every(16, …)`; Claude builds stage 1 (definite-height flow, kernel + Apple) in `lane/textflow`.

- **Router and viewport follow-ups** (2026-09-15; LLP 1038/1039): core, Contract
  routes, host projections, browser history and native URL entry points are implemented.
  Owed: Interview's held-swipe/retained-data and iPad-wide sweep; iOS cover viewport
  before first settlement; Chrome's software-keyboard viewport drive. TypeScript
  route helpers remain deferred until their first data-source consumer (1038 slice 3).

- **LLP 1035 follow-ups** (2026-09-09): landed the same day — inherited rows
  (1035.000 slice 1), `layout <node>` (1035.002 slice 1), a contact's phases on
  the web and macOS carriers (1035.003). Next, in the umbrella's order: the
  Simulator contact backend (1035.003 §3; the backend is Charlie's call — the held-read gate now passes at two placements after common-mode agent scheduling, `/tmp/messages-held-inspection/`), 1035.001
  slice 1 (XCTest units for the navigation/modal rules, journal lines for
  refused intents, a session-scoped blur in the two-session host), 1035.004's
  six symbol roles, 1035.005's `contract fmt` with the continuation rule;
  `dynamic` as a `layout <node>` source once a constant binding can be told
  from an expression; `metrics.mjs` printing the inherited-invalidation cost on
  Messages; the reply tags on `tap`/`type`/`clock`/`screenshot`.

- **Two `.contract` files cannot be shared between apps** (2026-09-08): the
  `Blocks`/`Runs` renderer is copied between `apps/markdown` and `apps/llp`
  because `use … from` refuses a `..` segment and a path outside the entry
  file's directory. A third reader is the trigger to widen it; doing so has a
  compatibility-id consequence (the shared file is an input to the bake).

- **Intermittent macOS GPU smoke stall** (2026-09-07): a TS Caltrain run sampled
  in drawable acquisition, then passed in 2.1 s on rerun; final `host` smoke
  exceeded 60 s and was stopped. Investigate window/display-dependent progress.

- **iOS module measurements** (2026-09-07): physical guard execution and live URL
  replacement/refusal/recovery now pass with retained state and unchanged process
  and binary (1027 D6). Measure linked size/startup/calls systematically next.
- **Repaired iOS onboarding acceptance** (2026-09-07): Charlie accepted touches in
  regular Caltrain after moving session creation past UIKit startup (1012).
  Normal cold/warm OS URL delivery and live reload pass after the storage fix;
  Charlie also confirmed native opening from Safari. Explicit cold/warm Safari
  cases and four-finger menu recognition remain to be checked.
- **Safari initial scrolling, phone sweep** (2026-09-08): the web host now gates
  actions/editing without making the baked tree inert; Chrome verifies scrolling
  with the data module held or failed. Repeat the touch-scroll check on iPhone.
- **Dev program rebuild boundary** (2026-09-07): after the normal phone's live
  edit passed, a server rebuild reported web inputs changed / iOS inputs unchanged
  but the iPhone required a native relaunch. Check whether the shared program
  identity is invalidating unaffected native clients (1030.000).
- **ibex host SDK during iOS bake** (2026-09-07): its host `darwin_http.mm`
  compile inherits the iPhone SDK; target-specific macOS CXXFLAGS unblocked this run.
  Fix SDK selection in the sibling build script rather than relying on that override.
- **Resource carry source identity** (2026-09-07): carry checks name, arguments,
  value shape and module hash, but not a Contract edit redirecting that resource
  to another source in unchanged logic. Include the source in compatibility.

- **macOS cover viewport smoke** (2026-09-06): `smoke.mjs macos` reports
  420×853 for the cover fixture versus Caltrain's 420×821, a 32-point
  titlebar difference. Check the initial window sizing and fixture expectation.

- **External wasm reproducibility** (2026-09-05): relocated source captures put
  absolute Rust source paths in the app and GPU wasm, so unchanged sources can
  republish different bytes. Keep closed snapshots; investigate stable build paths.
- **External dev latency** (2026-09-05): Weird Castle's first captured edit took
  300 ms to DOM acceptance (9 ms plan), above the 100 ms budget. Use the existing
  metrics drive to separate startup/preflight work from repeated edit latency.

- **Sample-host no-op input delay** (2026-09-04): `smoke.mjs host` passed but its
  final repeated light-scheme tap after destroying session a waited 74–131 seconds.
  Trace and GPU drawable samples are in the filesystem run; reduce the carrier/frame acknowledgement path.

- **macOS offscreen capture** (2026-09-04): a cover-window app can return a
  fully transparent default screenshot while its window capture is correct.
  Reduce to a generic canvas fixture; `issues/20260904-macos-offscreen-capture-can-be-transparent.md`.

1. **Linux host follow-ups** (LLP 1015 landed 2026-08-29; r2 the same day: vello on
   the GPU is the main painter, tiny-skia the fallback and pixel oracle; green on
   `expo-build-1000` (CPU), the minisforum (Vulkan/llvmpipe), and headless on macOS
   (Metal)). The DRM path ran on the minisforum 2026-08-29 (LLP 1015 §6: 1920×1080 @ 60 Hz on
   RADV, boot to first flip ~160 ms), seen and driven over the host's own VNC server
   (`EXACT_VNC=1`) since the KVM was unplugged; evdev itself has still carried no real
   event. Owed, in order: **a KMS surface for the GPU**
   (`VK_KHR_display`) so the frame is presented, not read back (17–19 ms of latency
   per frame on Metal today); `canvas` on this painter's device; the Chrome
   comparison of a pixel fixture (the font is pinned since 2026-08-29 — LLP 1015
   §3 — and `smoke linux` is green on a Mac and on the builder); a font cache when a machine's scan (25 ms
   for 787 faces on a Mac) matters; lifting the shared ~120 lines of orchestration
   out of `host/apple` and `host/linux`; one wgpu when vello moves to 30.
2. **Events, the rest** (the minimal set landed 2026-08-30: `hover`, `focus`,
   `blur`, `key` on web, macOS, iOS — LLP 1005 §3). Left out on purpose: pointer
   coordinates and moves (a drag), `keyup`, double-click, wheel offsets reaching
   the runner (the windowed-List line below). Owed with them: the Linux carrier's
   `tap … hover` / `type … key` forms (`smoke.mjs` guards them; the Linux lane);
   `key` inside a text field on macOS/iOS sees editing commands only (LLP 1008
   §5's declared deviation); and an action cannot branch on a key or hover
   payload — actions have assignment and command statements, no `if` and no
   string `match` — so `key=` today can only record the key.
3. **Weird Castle's asks** (Charlie, 2026-08-30: the app lives in `~/projects/weird-castle`
   on exact2 by path; `scripts/app.mjs`). In order: **asynchronous data settlement** —
   LLP 1016 (RFC, **Accepted** 2026-08-30 after a three-model panel: a `mutation` slot +
   `send`, `refresh` for queries; built the same day end to end — runner, compiler, the
   browser and `ibex2::host` executors, the agent's `settle`, Weird Castle's real `loginV2`
   against api.castle.xyz on three hosts; owed: the Linux executor on ibex OQ2, the token
   across launches) for the runner's reserved shape (LLP 1005 §7): a data source
   answers now or hands back a request, the host executes it (`ibex2::host` on native,
   the browser's `fetch` on the web), one ABI call brings the answer in, a pending
   resource is `none` meanwhile, bake tolerates it — the first out-of-process resource
   is `loginV2` at api.castle.xyz; **password masking** and **Enter to submit** landed 2026-08-30 — the
   kernel rows are the web's `type` and `inputMode` (LLP 1017 P9's sweep spells them
   literally later), honored on web/macOS/iOS; `submit` is the seventh event, the web's
   implicit submission; a session across launches is LLP 1018 (another session's lane).
4. **Markdown viewer follow-ups** (LLP 1033, macOS milestone 2026-09-05):
   iOS selection and link gestures; iOS/web file-opening adapters; heading anchors,
   tables, and syntax highlighting. macOS drag selection across paragraphs and copy
   now use CoreText's existing lines. Keyboard selection extension and bidi selection
   geometry still need dedicated fixtures. Variable-height windowing and scroll
   anchoring now run; startup against Legend is still a tie, and scrolling is
   ahead on one 60 Hz machine (see `apps/markdown/README.md` and the 120 Hz item
   above). At a 420-point
   window width, the open folder pane leaves the text cramped and clips header
   controls; the reader needs a compact layout at that width.
   During verification the macOS timer-step smoke once read width 50 at t=1250
   instead of 75, then passed on repeat; investigate the intermittent clock fixture.
5. **View transitions** (LLP 1013, Draft RFC) — behind the webview/deck lane by
   LLP 1020 §6's decided take (Charlie, 2026-08-30): the trade that moved `webview`
   off NOT-DOING. Shared elements, heroes, and magic-move
   the web's way, not RN's: a `view_transition_name` row, an action keyword as the
   boundary, `startViewTransition` on the web, snapshot + host-owned ghost under
   `exact-motion` on macOS, one parity fixture. Identity is by name at the host, so
   `when`/`match` arms keep destroying and creating and nothing reparents in the plan
   model. Needs the agent API's `clock` (3) to seek the ghost, `transition` in the tag
   table (below), and two ghost-only size properties on the engine. Charlie to confirm
   the boundary (§Open questions).

- **Rust replacement follow-ups** — **LLP 1029.000**: native/browser/Wasmi replacement
  and dev/prod controls are implemented. Next: physical-phone size/latency and
  20-edit/50-replacement measurements, normalized native-baseline reuse, and
  custom out-of-tree host composition; independent multi-module routing stays later.
  Parent **LLP 1029** (Draft RFC, 2026-09-03), mixed by default and optional
  executors, builds on LLP 1028 (the measurements: wasmtime runtime +0.49 MB / +Pulley
  +0.58 / wasmi +1.0 / Cranelift +5.6, never shipped). Charlie's three rulings of
  2026-09-03 — wasmtime optional, never mandatory even for Rust logic, the default app
  mixed TS + Rust — are its Summary; §7 stages it (phone numbers first), §8 asks
  Pulley-or-wasmi and four more. Awaits Charlie; the 1026/1027 amendment notes land at
  acceptance. **LLP 1032** (Research, 2026-09-04) measured Charlie's "does wasm make
  Hermes unnecessary?": QuickJS inside a wasm module on the same twin and seam is
  1.5–3× the lean Hermes VM with precompiled desktop code, 40–134× under Pulley and
  17–64× under wasmi (an interpreter inside an interpreter), and 1.5–2.1 MB as a
  Pulley artifact against Hermes's 1.8 MB; 20/20 bytes on every engine. Feeds 1029 §8
  and stage 0's phone afternoon (the iOS Pulley host builds; the phone was not
  connected).

- **Delivery, unified** — LLP 1030 r2 and LLP 1030.000 r2. Landed: stage 1 (the
  asset row), stage 2 (the manifest), the compatibility id, the `delivery`
  resource, the update store, `exact deploy` (snapshot/bake/classify/publish
  through `scripts/origin.mjs`), and the macOS and Linux hosts opening the
  store. **2026-09-04 review** (LLP 1030.001): the landing is real and the
  signature seam holds; four structural properties the RFCs paid for are not
  what the code does — filesystem issues `issues/20260904-*.md` (anti-rollback
  floor is 0, stream files overwritten before the head, Linux next-launch
  ignores entry assets, snapshot is a git-status of the app dir, activation
  cannot represent two pinned session generations, and the remaining P1–P3
  findings, most on the dev loop). Owed as before: bake writing the update bundle beside the archive;
  the dev policy folded onto the store (1026 D12); iOS driven; `BGAppRefreshTask`;
  Linux font and deck overrides from an entry; the sunset card shown by the
  app; the web host, which has no store (`L = A` in its `compat.json` is a lie
  until it does, or the manifest says `0` for it); `--watch`; the object-store
  adapter; the binary lanes.

- **LLP 1031 owed after the 2026-09-03 landing** (the handle, `ExactKit`, the macOS
  sample host, the manifest stage; the iOS sample host landed 2026-09-04 — each pane its
  own navigation controller, `smoke.mjs host-ios`): **content-height containment** (D3's
  guard is written; bounded only today); **the request and store crossings** (D4's
  contract; triggered by an adopter whose client cannot be wrapped); a request in flight under a destroy (needs a fetching app — Weird Castle — as the
  fixture); and the host smoke's remount step, which took ~3 s in some runs: measured 2026-09-04 in
  isolation at 42 ms (`clock settle` after the pop) and 7 ms (a's layout) — the seconds
  appeared only while two agent lanes were compiling on the same Mac, so it reads as a
  settle bound under load, not a remount cost; watch for it on a quiet machine.
  Weird Castle is on today's exact2 (2026-09-04: the registry shape, `host!` with
  `COMPAT`, `app.json`), but its `tests/app.rs` does not compile since LLP 1027 stage 3 —
  six assertions want `Answer::Now(...)` around a `Value` — and the file carries another
  session's uncommitted edits; whoever owns them wraps them.

- **Tab on iOS with a hardware keyboard** (Weird Castle login): macOS now
  wires the key-view loop after each batch (fields and buttons, tree order,
  Space/Enter on a focused pressable). iOS still has no sequential-focus
  path for an external keyboard.

## From the 2026-09-01 review (LLP 1025)

LLP 1025 is the dated review snapshot, not a live defect list. The 2026-09-01
closure pass moved 69 findings to `issues/closed/`; the last two blockers closed
2026-09-02 (285be27) — the response ceiling ibex2 was missing (ac156bee3, and
Darwin turned out to have had none at all), and the sysroot clang actually reads
when SwiftPM leaves a host SDKROOT in the environment. Nothing from 1025 is open.

## Cheap, any time

- **The sky's capture on the phone** landed at the display's rate 2026-08-30 (LLP 1008
  §9): a shadow layer tree rendered by `CARenderer`, a nested canvas's readback cached,
  the texture handed to the module as it is (`gpu_texture_metal`, `gpu_sync`) — 119 fps
  on average scrolling with the sky on, a capture 7.1 ms, from 42–64 fps and 20–25 ms.
  Left: the deck's 48 per-child textures still cross as bytes (a `gpu_child_metal`
  would spare ~30 ms when the deck opens); the macOS presenter still captures on the
  CPU (`cacheDisplay`, 5–18 ms at 2×) — the same `Shadow` would serve it; the
  cross-queue wait (`gpu_sync`, ~3 ms of the 7.1) could be a shared Metal event if
  wgpu exposed one. Tried and declined on the way: capture at 2× (slower), `CARenderer`
  on the live overlay (crashes — a layer is in one tree only), `drawHierarchy` (no gain).
- **Canvas children** → LLP 1014 (RFC, Accepted) — built 2026-08-29, LLP 1014.000
  transcribes it: kernel, corpus, the web wrapper, the macOS overlay and capture with
  D4's four sources, the smoke's steps 6a, 9, and 10 with the readback fixture (native:
  `caltrain-gpu` `tests/children.rs`; per host: `scripts/fixtures/canvas-sky.*.png`,
  cross-host mean 1.48/255 as measured — a first number for 1009 §4.1); the line map's
  stations as children (the second use); the render tail measured and closed (0.5 ms
  steady; occluded windows now skip rendering); Accepted 2026-08-29, the §5 take applied
  (the three gradient rows are out — 1009's proposal for the same door). 2026-08-30:
  the app inside the sky (glass/ink/crt materials, the previous-children crossfade,
  nested canvases by readback) and D5 built — per-child textures, placements with
  depth, hit-testing and accessibility through them, the card deck (LLP 1014.000 §1a,
  §1b); two code reviews folded the same day (§1c: placements are the only place a
  placed child is; `clock`/`tap`/`type` settle the canvases before replying; a resize
  shows at once; the ABI checks its pointers). Left: Chrome's flag as the oracle for
  children *through* a surface; a hermetic test of the glass crossfade; LLP 1013 on
  this machinery. Delete this line then.


- **Menus** → LLP 1021 (RFC, Draft 2026-08-30): the Popover API by its HTML names
  (`popover`, `popovertarget`, `id`, `aria-checked`, `hr`), open state host state like
  scroll, the top layer outside every canvas capture; a menu-shaped popover may present
  as UIMenu/NSMenu with selection dispatching by view id, kernel-painted under
  `EXACT_AGENT`. First consumer: Weird Castle's account switcher (today a hand-rolled
  overlay with no light dismiss). M1 web+kernel · M2 Apple/Linux · M3 the switcher.
  Messages Add now preserves focus; its rich native attachment popover remains owed.
  Three UIKit probes stop short of keyboard-overlaid painting; matching the box
  alone passed falsely (`/tmp/messages-attachment-menu/probe/`, LLP 1021 D2).
- `fitDocument` reading the root's `content` so the page's extent includes overflow
  past the root, as `scrollHeight` does (LLP 1010 §3's declared deviation).
- The untested-but-built list in LLP 1010 §5: horizontal scroll on macOS, `overflow:
  hidden` on the web, `hidden`/`visible` on a `ScrollView` on macOS — each a fixture.

## Open decisions (Charlie's)

- **LLP 1009** is `Review` while the canvas is built. Its §5 trades: the three
  gradient rows leave `schema.json` (done under LLP 1014 §5); amend NOT-DOING's "compiles no shaders at
  runtime" to "compiles nothing on the boot path"; the refine-loop take. Then the
  1009.000 spec transcribes the landing.
- **Linux painter: decided 2026-08-29 — vello on the GPU** (Charlie), with tiny-skia
  kept as the CPU fallback and the pixel oracle (LLP 1015 §7). What it owes: the
  NOT-DOING wording "compiles no shaders at runtime" now needs the clause "the GPU
  painter compiles its shaders on the first launch on a machine and reads them from
  the pipeline cache after" beside LLP 1009 §5's — Charlie's to write.
- **The app's name** (`rules/NOT-DOING.md` opens with it; the recommendation is
  Caltrain and Caltrain is what exists).
- **Contract, started over** — LLP 1017 (RFC, **Accepted** 2026-08-30 evening; **all nine landed on main the same night: P1, P9, P2, P6, P8, P4a/b, then P5 `fn`, P7 `test` beside the app, P4c per-instance state** — LLP 1017.000 transcribes each; owed with P8: the dev loop watches the app file only, so an edit to a `use`d file needs a save of the app file — §8: literal CSS names, commands stay the framework's, all nine proposals in one lane in the order P1+P9 → P2 → P6+P8 → P4 → P5 → P7, no instance reload carry, Coterie rerun deferred; in `llp/current/`;
  sub-LLPs 1017.001 grok and 1017.002 codex answered the same brief blind; a one-round panel — `llp/reviews/1017-contract-restart.{codex,grok}.md`, §10 — settled every fork but the CSS spelling, which is Charlie's): the seven Coterie diaries and exact1's record read
  for what consistently cost authors time, and nine proposals in order of leverage — kernel
  rows as the compiler's value checker plus a layout lint at bake, `if`/`match` in actions,
  `send`/`refresh` as LLP 1016 decided, children reading the root by name and a
  `children` slot, `pure name(params): type` implemented in the app's Rust crate, named styles,
  the `contract` block deleted for the agent script, `use` across files, the RN-word sweep.
  §8's six questions are Charlie's; if one lands first, P1.

## Declared gaps, by system (each spec's "Not in v1")

- **Kernel / layout** (LLP 1001, 1010): `position: static`; `text_align: start`;
  presentation transforms in scrollable overflow; `overscroll-behavior`; scroll
  snapping beyond the admitted horizontal subset; Linux scroll events.
- **Motion** (LLP 1003 §9): layout transitions (gated on an incremental-relayout
  number); `@keyframes`; a Core Animation executor (measured question); reduced
  motion is the producer's `transition: none`.
- **Durable state** (LLP 1018 §5, built 2026-08-30 for Weird Castle's token): the
  `plain` tier — persisted slots, with Caltrain's "last station"; `me`; the iOS
  first-launch keychain wipe; the data-protection keychain on macOS (a bundle and a
  profile); the Linux file store when that host links ibex2.
- **Plan / runner** (LLP 1005 §8): a deps table and dirty-set sweep; per-instance
  derives or resources inside `each`; asynchronous data settlement.
- **Contract** (LLP 1006 §8): inlining budget; `contract` blocks as assertions;
  `cursor`; per-instance state; LSP and formatter; `linear()`.
- **Web host** (LLP 1007 §9): a text-measurement bridge (the kernel's layout does not
  run on the web — and nothing today checks the kernel's layout against the
  browser's; a layout parity corpus would be the analog of `parity.mjs`); gradients,
  grid, `line_clamp`, `font_family`; scroll position and focus across a reload;
  `prefers-reduced-motion`; a spring interrupted by an easing.
- **Apple host** (LLP 1008 §7): images; toggles; accessibility beyond `testId` and
  `accessibilityLabel`; justified text; per-corner radii; rubber-banding on inner
  scroll nodes; a generated header. **iOS** (LLP 1008 §9): the agent API on a
  phone (`build.mjs --device --run` installs over USB or Wi-Fi, but nothing drives
  the app there); a
  synthesized touch for the agent's `tap` (UIKit's hit-test and the responder-chain
  rule today); a pan chaining out of a nested scroll view at its edge (UIKit's own;
  the agent's wheel chains).
- **The keyboard's blink on a field hand-off** (iOS): UIKit rebuilds the accessory bar
  when first responder moves between two `UITextField`s — one bar-less frame (308 →
  335), reproduced in a from-scratch two-field app with any traits and by re-traiting
  one field in place; a phone shows it faintly. Safari does not, so WebKit hides it
  somewhere below the responder (its `UITextInput`-conforming content view and its own
  input-assistant handling). Worth finding out how, if a form ever needs it gone
  (LLP 1008 §9).
- **Linux host** (LLP 1015 §7): evdev has carried no real event yet; `canvas`; libinput and
  xkbcommon (acceleration, touchpad gestures, hotplug, non-US keymaps); Wayland/X11
  windows; selection, IME, a caret blink; `text_decoration`, `font_family`, RTL;
  shadows, gradients, grid; JPEG, image URLs; accessibility; a font cache; pixel
  fixtures against Chrome (needs a pinned font).
- **GPU** (LLP 1009 §4): readback bands for blending and MSAA; a generated module ABI
  once there is a second consumer; the `Custom(u16)` animatable-property extension
  when a surface needs it.

## Later

- **Windows, Android** — `rules/NOT-DOING.md` §Surfaces; after the loop is proven.
- **ibex2** (`~/projects/ibex/crates/ibex2`; ibex LLP 0057 §5.2 targets Exact 2). Two
  things, two triggers (Charlie, 2026-08-29). `ibex2::host` — the Rust standard library
  (`fetch`/`fs`/`env` behind grants), `default-features = false`, no engine linked, no
  boot-graph change (ibex LLP 0068) — at the first `resource` that needs bytes from
  outside the process: image by URL, a real Caltrain feed. Build it together with the
  runner's asynchronous data settlement (LLP 1005 §8), which it needs anyway; ibex OQ2
  (a TLS transport off Apple) is owed before Linux consumes it. The engine (`hermes`
  feature) only at a measured call site, after v1, as another `DataSource` loaded on
  demand after first pixel (the GPU-module shape, LLP 1009 D2) — the plan, kernel, and
  Contract do not change. On the web the browser is the executor, so one module runs
  under two loaders; design that first. Zero app JS until then. **Superseded 2026-09-03** (Charlie: TS optional,
  the default paved path for substantial app logic) — LLP 1027 is that design; the
  `ibex2::host` half above stands as landed.

- The dev menu (Apple hosts) is on by default — EXACT_DEV_MENU=0 is the only off
  switch, and "Quit Exact"/"Exact" are hardcoded names. Before any build of
  weird-castle goes to someone who isn't developing it, decide the release gating
  (and take the app's name from the bundle).

- The serial runtime owner + env()/keyboard lane is parked on `parked/runtime-owner`
  (2583be3, base 17350d0 — pre-fonts) and the main worktree is clean of it. LLP 1022
  holds the measurements (fails the macOS smoke ~6 of 8; macOS boot +26 ms for a
  shell frame; iOS −27 ms from the prepare overlap), the four cherry-picks worth
  taking without the owner thread, and the conditions for reviving it. A revival
  rebases across 43b0c0c and lands only through a green smoke.

- LLP 1023 Stage 1 leftovers, each small: the physical-iPhone typed-URL run (the
  phone was asleep on landing day — `bun host/apple/build.mjs --device --run`, then
  4-finger tap → Open Project… → the LAN URL dev.mjs printed; ATS and signing already
  proven on the built bundle); a native pre-download `kernelSchema` compare (needs the client's own digest exported —
  the runner's boot gate catches a mismatch only after the download); `metrics.mjs`
  rows for cold URL→first-frame and hot seq→first-frame over the LAN (diagnostic,
  never a sixth check).

- LLP 1024 (native modules) is r2 after the 2026-08-31 three-model panel — unanimous,
  design settled (one app artifact, roster-at-bake, versioned table ABI). What it
  waits on is Charlie: ratify §6's Q1 leaning, and name a consumer + implementer;
  D8 does not start without both.

- LLP 1026 (dynamic delivery: the app over the wire from a cloud that builds it) is
  Draft r2 (2026-09-02), Charlie's exploration, no implementer. r2 is "both worlds": one
  bake emits the native archive the binary embeds and a signed static update bundle;
  a client keeps an on-disk store and selects at launch; native by digest identity, so
  the interpreter runs only in the update window (D10); wgpu moves to the host so the
  app's surfaces travel as wasm against WebGPU imports (D6); native modules stay in the
  binary (D7). Level A is plan + assets at zero binary cost; Level B adds the app module
  and +1.0 MB of wasmi. Measured: 72 KB module, ~1 ms to instantiate, byte-identical
  answers, 8–10× on microsecond calls. Waits on §8's trades (the update economy comes
  off NOT-DOING; the take is the file poll + two dev-only loaders, and Stage 3's DNS-SD)
  and §10's eight questions, the binary size first. 1025's link left `current/` for it.

- LLP 1027 (TypeScript data sources) is **Accepted** (Charlie, 2026-09-03, every
  recommendation; implementer Claude, stage 1 landed the same day: the `sources` table in
  the plan (format 3), `DataSource::bind`, the `exact-js` crate over the lean Hermes VM
  with Caltrain's TypeScript twin as its byte-equality fixture, Rolldown at the repo root).
  Stage 3 landed the same day: `fetch` over the host's ticket path (a prelude in
  bytecode; one host door with four ops for requests and the store), `parse` returning
  an `Answer` so one answer awaits two fetches in a row, eleven tests. Next: stage 2
  (bake: Rolldown → hermesc → bake under the VM, `app.d.ts`, `dev.mjs`) built against
  its first consumer, stage 5 (Weird Castle in TypeScript), then stage 4 (web), stage 6
  (delivery), stage 7 (the engine crate + Linux). LLP 1027.001 supplies the named
  Ibex URL, UTF-8 text and base64 bindings. LLP 1027.000 landed 2026-09-04: time and seeds
  are explicit inputs; ambient Date/Intl time and Math.random calls refuse. The
  boot question was ruled the same day ("ok let's do that"): **the kept answer** — the
  runner persists a store-reading resource's last answer beside the secrets, boots
  from it when the source is not ready, falls back to the bake's empty-store placeholder
  (plan format 4, `resources.reader`), and `data_ready` asks again once the engine is up;
  landed with a test through the runner. Written the day Charlie
  ruled "TS support optional, but the default paved path for anything with substantial
  app logic/business logic." Nothing above the data seam changes: `app.ts` exports
  `appId`/`grants`/`answer`/`parse`, values cross as JSON directed by the plan's own
  `fields` table, the module has no globals that reach out (a request is a value the
  host runs), bake compiles it with `hermesc` to bytecode for native and to JS for the
  web, and a new crate `exact-js` (the lean Hermes VM, ~100 lines of C++) runs it after
  first pixel; Rust stays behind the same seam for hot sources (D8). Measured: Caltrain
  ported to TS answers 20/20 cases byte-identical, 0.32 ms create + 0.008 ms load,
  1.2–41 µs a call (7.5–29× native, mostly JSON), the linked lean engine +1.81 MB
  stripped (+785 KB gz), TS→bytecode 20 ms. D9 answers "split ibex2 in two": it is three
  — `ibex2::host` (linked today), the engine build as a `-sys` crate (proposed), ibex2's
  JS runtime layer (never linked). Waits on §8's trades (D4's sentence, "HBC
  compilation" for the bake, 1026's wasm data module leaves its staging so a phone
  carries one interpreter) and §10's eight questions — 1.8 MB on iOS and the
  one-frame `pending` first. 1019's link (Accepted, landed) left `current/` for it.

- macOS text controls: align focus/blur notifications with first-responder changes; AppKit editing-began delegates currently wait for the first edit, so a focus command can move the caret before a Contract focus handler runs.

- **Stale `target/` cache from another checkout breaks every bake** (2026-09-08):
  its dep-info names `/Users/ccheever/projects/exact2-principles-metrics/target/…`, so
  `host/web/build.mjs` dies in `completeBuild` ("stale compiler dependency names missing
  input"); worked around with a private `CARGO_TARGET_DIR=target/dev-local`. Needs: decide
  whether the checkout's `target/` gets wiped/rebuilt once or the receipt step tolerates
  foreign dep-info.

**2026-09-14 priority:** the decorative artwork, material/pixel matching and
animation-timing items below are deferred behind LLP 1010 §6's list-memory
acceptance. Their evidence stays here; functional navigation/editing fixes continue.

- **Messages iPhone parity** (`apps/messages`): the local chat example now runs on
  web and iOS. Finish native back/Tapback/reply gestures, timestamp motion matching (recognition threshold and release curve; resisted/capped travel and stationary labels now work; extent/rate/paging probes rejected, a linked Exact-engine spring now measures 1.14-point RMS error versus 7.70 for scroll snap, with physical vertical/reversal checks; R4 reuses current editor exclusion and identity-bound Presenter.dragX: physical read-only selection moves rows 0 points versus 44, and Back-during-hold emits 0 gone-target writes versus 11. Of three additional attempts approved by Charlie on 2026-09-11, R5 fails compilation on unsupported inset() clip-path; R6 margins/overflow hide rest labels with 23 native rectangles unchanged, ten physical assertions passing, zero retired writes and browser geometry verified (`/tmp/messages-timestamp-ownership/r6/`). All three additional attempts are consumed; authored intent and browser ownership still need integration),
  anchor-removal fallback and typing/insertion motion, then compare actual touch
  behavior and screenshots with Messages.
  `apps/messages/README.md` records the current implementation gaps.
  The focused reply surface, swipe entry, and reply indicator are in place; finish thread positioning,
  and connecting lines. Transparent curved tails now work over its material.

- Messages fractional borders: compare device-pixel snapping at 1×/2×/3×. The shared `none`/`hidden`/`solid` semantics, default width and `currentColor` are implemented; Messages and the other existing bordered apps now declare solid explicitly. Native style/color switches and Fieldnotes web borders pass, and the Messages iOS comparison is pixel-identical (`/tmp/messages-border-semantics/`, LLP 1001 §1).

- Messages navigation: after three integration fix rounds, native header back-swipes cancel/complete with drafts, but the timestamp horizontal scroll still consumes rightward gestures over the transcript. An app-only directional `touch-action` probe enabled edge Back but not gutter Back; reverted because the fresh simulated Messages reference ignored the corresponding navigation gestures despite working timestamp input and button Back. Establish a reference that demonstrates the gesture before calling that change parity; no engine gesture arena.

- Messages reactions: per-person blue/gray badges, independent add/remove, inner-edge anchors and grouped participants now pass clean physical main/focused/group checks with the named Simulator verified foreground. The reproduced input miss was a browser covering the Simulator, not a delivered UIKit touch (`/tmp/messages-touch-phases/`). The participant/menu region now prevents keyboard-open overlap with the measured 24-point gap. Source anchoring through keyboard dismissal and transcript displacement behind a crowded preview now pass iOS/web checks (`/tmp/messages-context-anchor/`), including restoration and the focused thread's retained clip. Measure badge travel with a real native iMessage reply gesture, then match it; the SMS fixture only supplied drag-and-drop (`/tmp/messages-reply-badges/`). Finish native artwork. The participant popover's rectangular shadow cutoff is repaired by separating its glass from its scrolling content (`/tmp/messages-popover-shadow/`). Preview entry modes now match the incoming/outgoing native evidence: badge and double tap keep the original size; long press magnifies, including the picker (`/tmp/messages-preview-modes/`). Native focused-thread scale is still unmeasured; Exact applies the same entry policy there. Compare focused-thread Close's keyboard policy (current physical Close blurs the composer).

- Messages composer: available-height growth and 20-point line pitch replace the six-line cap; a terminal Return reserves its caret line without altering the value. Browser/iOS 1–32-line drives and 11 physical typing/scrolling cases pass (`/tmp/messages-composer-growth/`). Resolve Compose's two-point upper-boundary difference. Real Paste keeps the final line visible; the bulk-replacement capture was premature. The driver now observes UIKit’s reachable caret reveal, preserving user scrolling and cancelling stale editor observations (`/tmp/messages-editor-paste/`). The subsequent viewport repair observes session-scoped geometry animations through a native idle turn, including keyboard work queued by sheet completion. Production early-open and bulk-edit captures, two-session isolation and physical manual-scroll retention pass (`/tmp/messages-viewport-settle/`). Both three-candidate repairs stop with their final candidates integrated. The wrapping repair matches six native specimen line counts/heights with a 38×28 Send control and 252.67-point editor; the browser reset now preserves textarea long-word wrapping. Both composers pass 44 browser/iOS cases and eight physical typing/deletion/emoji cases (`/tmp/messages-wrapping/`). Compare glyph rasterization, more scripts/widths, rotation, keyboard variants and growth motion before claiming full composer parity. The three CSS implementation rounds stop with the third candidate integrated.
- Messages rotation: the active keyboard guide now corrects a four-point landscape composer offset; both landscape directions, return to portrait, retained editing, Back/sheet gestures and two-session destruction pass (`/tmp/messages-rotation/`). Individual software-key typing yields pixel-identical first-three-keyboard-row crops in both landscape directions; `typeText` had left Shift selected. Remaining: native landscape header (44-point avatar, hidden name pill, Back at x=38/y=24) and the handwriting control. The orientation-plist experiment has no measured benefit and is not integrated.
- Messages compose: physical first Send installs the conversation before sheet dismissal and focuses its composer, removing the inbox flash (`/tmp/messages-compose-handoff/`). Both Send controls now retain focus; ordinary Send keeps the same editor, keyboard and viewport. The subsequent native editor-retirement repair keeps its outgoing hierarchy mounted and delivers destination focus before dismissal; the uninstrumented first-Send drive has no hidden-keyboard samples or sampled vertical keyboard movement. Seven physical Back/sheet cases and two-session reload/unmount/destruction pass (`/tmp/messages-editor-retirement/`). Establish native first-Send motion parity; native Compose opens through the public `sms:` URL, but typed and URL-supplied fixture addresses remain `No Name, Searching`; first Send is still unestablished (`/tmp/messages-native-send-reference/`). Native dismissal retirement now survives until UIKit completion and across reload; current route intent then projects once, preventing a refused Close → Compose from remaining full-screen. Timed reopening, reload/pending destruction and nine physical sheet cases pass (`/tmp/messages-modal-retirement/`); destination focus remains independent of retirement. The editor-retirement repair closes the keyboard gap; complete native motion parity remains open. Typing now replaces the selected recipient while retaining other recipients, the draft and To editor identity/focus; eight-step browser and physical iPhone drives pass (`/tmp/messages-recipient-replacement/`). Finish token selection handles/caret placement, invalid-address presentation/international formatting, native sheet motion and existing-history presentation before send. UIKit modal presentation and empty-draft gesture dismissal are now in place; the live source view follows appearance changes while a sheet is open; finish the small dark corner-edge difference and complete transition timing. Multiple fixture recipients now create/reuse local groups on send. The dedicated fixture recipient sheet replaces the old inbox-search shortcut.

- Messages native ownership: authored `inert` now reaches the compiler and real DOM attribute; iOS subtree input/focus/accessibility, preserved geometry/drafts and restoration pass. Native confirmation now escapes inert ancestors and survives its invoker becoming inert; iOS/browser cases and four Messages cases per host pass. The third/final browser candidate passes explicit focus and inspection inside escaped modal dialogs, single confirmation dispatch, basic/active-route input exclusion and four rebuilt Messages cases. Forty browser identity/frame observations remain unchanged. AppKit/Linux, wider VoiceOver/IME/drag cases and implicit modal-inertness reporting remain (LLP 1035.001 D3; `/tmp/messages-inert-ownership/verification.json`). Back lookup now stays within the selected route; native pop/sheet permission and browser Escape checks pass, including refusal with retained sheet editing (`/tmp/messages-back-owner/`, LLP 1035.001 D1). Completed-sheet cleanup now releases the old modal-navigation slot after UIKit returns; retained/replacement routes and mid-gesture permission changes pass six physical cases (`/tmp/messages-dismissal-owner/`). Revocation after native commitment re-presents the selected route with its keyboard closed; continuous editing through that outcome remains unestablished. Unmount/destruction during a physical sheet drag now retire native owners without cancelling retained Compose; offscreen route replacement and queued focus survive remount (`/tmp/messages-unmounted-owner/`). Moving to a different UIKit controller in the same window now passes, including transfer during a physical sheet drag and queued focus after offscreen route replacement (`/tmp/messages-reparent-owner/`). Fullscreen/source ownership is now implemented under the approved LLP 1035.001 Messages exception; exact motion matching remains. A fresh native recording shows details zooming from the contact photo; a public UIKit over-full-screen zoom plus nested page sheet reproduces bar y=88 / field y=416 and passes cancellation, permission, completion and missing-source Back (`/tmp/messages-presentation-ancestry/`). R4/R5 pass the Messages flow, ten physical ownership cases, source replacement, nested teardown/reload and browser Escape (`/tmp/messages-fullscreen/`); one authorized continuation attempt remains unused.

- Investigate JavaScript storage unload/write ordering: the workspace run observed `cancel` where `unload_invalidates_continuations_and_configuration_survives_reload` expected `again` (`js/tests/storage.rs:210`, 2026-09-10). Its isolated six-test suite and complete `--no-fail-fast` workspace rerun passed; the cause remains unproven. Unload invalidates the continuation, but the underlying write may already be in flight; establish that ordering before changing its guarantee.

- Investigate intermittent Linux update test `a_refused_initial_layout_releases_no_network_requests`: the workspace run observed one request after refused layout; isolated and full-workspace reruns passed (Messages compose validation, 2026-09-09).

- Messages symbols (1035.004): twelve schema-generated roles now render through native image leaves and browser masks; Messages has 19 symbol occurrences, and Fieldnotes uses Add/Close. Copy, Select and More now use native 17/400 images matching the reference’s intrinsic sizes; six physical icon taps and six browser actions pass with unchanged action frames and retained drafts. Reference-mask overlap improves, but exact raster alignment and Reply artwork remain open (`/tmp/messages-action-symbols/verification.json`). iOS physical controls and both apps’ web controls pass; natural/fixed/live-source fixtures pass on Apple/web. Finish pinned Messages glyph sizing/weight and VoiceOver comparison, material support reporting, and AppKit pixels (its current captures are transparent). Linux symbols remain unsupported (`/tmp/messages-symbol-integration/verification.json`). A stock-glass-button prototype passes eight physical actions and supplies native foreground treatment, but its Close raster error is worse than production; three calibration rounds stopped, no projection landed. Resolve that rendering difference before extending the native control boundary (`/tmp/messages-glass-button-candidate/verification.json`).
- Messages inbox gestures: finish trailing full-swipe confirmation, recovery styling/filter coverage, exact glyph size/artwork and transition comparison. Recently Deleted now archives individual/whole-thread deletions and provides local recovery/permanent deletion through the existing menu and confirmation owners; data order/reaction/reply retention and browser/iOS activation pass, but ordinary native return to Messages fails with a loading menu after recovery (`/tmp/messages-recovery/`). The three-candidate application loop is stopped; the source remains an incomplete candidate. Recover’s native default action remains black/gray rather than the reference’s blue; the browser selection outline and agent-mode ordinary-menu presentation also remain mismatched. Native partial and full deletion both ask first and cancel on outside tap; a public UIAlertController fixture proves deferred swipe completion/cancellation but differs in anchoring. Three presentation rounds stopped (the third failed compilation; its captures ran the preceding binary), preserved at `/tmp/messages-thread-deletion/`. No confirmation change landed. Explicit UIKit swipe rows now pass leading full Read/Unread, reversal, partial actions, Mute/Unmute, Delete, row switching, first-tap dismissal, draft/history retention and parent vertical scrolling (`/tmp/messages-swipe-activation/`). Callback eligibility follows the original authored ancestors, excluding UIKit’s temporarily disabled cell. Native action inspection and agent activation pass; the browser retains its authored controls. First-child scale/rotation capture and the pinned light/dark action colors are corrected (`/tmp/messages-swipe-glyphs/`); exact symbol paths remain owed. Open actions still retain their earlier appearance: two adaptive-image attempts passed action checks but corrupted surrounding avatar lettering and were removed at the repair limit. The ordinary light-to-dark control has pixel-identical surrounding rows with and without icon scaling; that comparison clears the scale change, not the adaptive experiment. Isolate the rendering failure before another adaptive capture attempt. Named inbox controls now retain their accessibility names and activation through native swipe cells; public activation and stale filtered-cell refusal pass, alongside eight physical swipe cases (`/tmp/messages-swipe-accessibility/verification.json`). Full VoiceOver interaction remains unverified. Real iOS gesture input still uses the temporary foreground-guarded helper.

- Messages contact details: finish call/video/email, contact editing, group/shared-content sections, and switch motion. Native Block and New Contact discard confirmations now have measured public-UIKit prototypes: both rectangles match, Block has a pixel-identical crop over its saved reference backdrop, and an actual field retains its text/keyboard through outside cancellation (`/tmp/messages-contact-actions/`, LLP 1021 D2). Local Block/Unblock and the declared confirmation are now integrated: native activation and physical actions pass, cancellation preserves drafts, and the two-session fixture covers live editors, reload, unmount and destruction (`/tmp/messages-confirmation-integration/`). Explicit `destructive` intent now produces native red in confirmations, menus and swipe actions; adversarial role/colour and two 22-capture physical checks pass (`/tmp/messages-destructive-actions/`). Contact-row CSS sizes now remove the two-point Block anchor gap and restore full-row hit areas; browser geometry and 22 physical captures pass (`/tmp/messages-contact-geometry/`). Browser Escape now dismisses the New Contact confirmation without closing its protected sheet or losing editor focus (`/tmp/messages-popover-escape/`). Finish browser placement, window-capture glass fidelity (1.40/255 mean crop error versus simctl), and complete New Contact fidelity. The local form now supports names/company, one phone/email and notes, with retained discard confirmation, fresh reopening, local Save and draft restoration verified by iOS/browser activation and physical touches (`/tmp/messages-new-contact/`). Finish multiple addresses, photo/pronoun/tone controls and the remaining native fields. Add to Existing Contact remains disabled. Its native chooser → existing editor → discard-to-details sequence is captured in `/tmp/messages-existing-contact/reference/`; existing addresses remain and the incoming address is appended, including an observed duplicate. The three-candidate app attempt failed compilation before runtime and was restored; the attempted contact-identity/address-list model is unverified in `r3/`, with failures in `verification.json`. Its y=62 origin differs from native y=72: the public-UIKit probe reproduces y=72 only above an existing modal, while Exact pushes details. Charlie approved the fullscreen/source Messages exception and three further attempts after the first three were reverted. R4 fixes Compose’s stuck transition flag and supplies fullscreen details with a retained nested sheet; R5 repairs browser Escape from body focus without consuming keys in outside host inputs. The local flow, ten physical cases, source identity and nested host lifecycle pass (`/tmp/messages-fullscreen/r4/`, `r5/`); the third additional attempt is unused. Full native motion and remaining form fidelity are still owed. The second physical repeat passes with 13 XCTest idle timeouts. Confirmation dispatch now waits for the native callback stack to return; two fresh physical runs have no timeouts and pending-action reload/destruction checks pass (`/tmp/messages-confirmation-completion/`). The smaller UIKit comparison did not reproduce the original symptom; keep its cause unproven and finish motion parity. Third-route navigation, draft/focus and reading-position restoration, and shared Hide Alerts now work; ordinary-launch button Back and cancelled/completed native swipes preserve the transcript with the keyboard restored.

- Messages balloons: `overflow-wrap: break-word` now prevents unbroken text from overflowing a web bubble; iOS/browser three-line message and inherited policy checks are in `/tmp/messages-overflow-wrap/`. H/Hi now measure the native 48×40 after Taffy patch 6 removes double-counted parent padding (`/tmp/messages-short-width/`). Match multiline intrinsic sizing (native sample 262 points wide, Exact 277 with the same three lines; current Chrome rejects `max-content-sizing` even with its named prototype flag, `/tmp/messages-wrapped-sizing/`), fractional bubble rasterization, and sender-name repetition after pauses. Tail contours now measure about 0.10/0.11 points of outgoing/incoming edge error after normalizing the bubble bottom; complete pixel equality remains unverified. Time-based bubble runs now follow the measured 30/59/61-second cases; the exact 60-second boundary remains unverified. Minimum size, padding, line spacing, transcript insets, and group preview indentation now follow native samples.

- Messages date headings: establish same-day grouping from a stronger native fixture before adding a cutoff. Native Messages and the current web/iPhone app retain one heading across a 301-second gap followed by a 299-second gap; subsequent 601- and 901-second native pauses also keep that heading. Five-, ten-, and fifteen-minute inactivity cutoffs would disagree with this reference; the bounded pause study has not established a threshold. Matched captures and recorded input times are in `/tmp/messages-date-headings/`.


- Messages Tapbacks: Copy now writes the full message and restores the draft/focus; the menu has leading icons and measured row spacing (`/tmp/messages-copy-action/`). Text Select now restores the source and uses a read-only native editor; foreground-verified handle dragging copies a substring and outside dismissal retains the draft (`/tmp/messages-text-selection/`). Outgoing white selection handles now follow explicit CSS `caret-color`, retaining the incoming system tint in light/dark captures (`/tmp/messages-selection-tint/`). The iPhone editor now uses the bubble's authored line spacing (`/tmp/messages-selection-lineheight/`). A disconnected session `selectText` dispatch is repaired; a batch-to-editor regression and eight public-XCTest actions cover native handles, shortened and incoming Copy, and dismissal with the draft retained (`/tmp/messages-selection-geometry/`). The current 17/20-point specimen has no measured glyph shift through selection; broader native raster comparison remains. The six standard Tapback glyphs now use bundled native resting images across palette, badges and participant bubbles; light-reference comparison, web/iOS replacement/removal and physical badge checks are in `/tmp/messages-reaction-artwork/`. Selected choices now use the sampled native light/dark blue in a 44-point circle, with contiguous 49×64-point hit cells; ten browser/iOS captures, unchanged artwork frames and six physical edge/gap taps pass (`/tmp/messages-selected-tapback/`). Finish Translate, selected-text rasterization/return motion, Reply artwork and final symbol rasterization, emoji/sticker picker and attached button, animated artwork and compositing (including system-keyboard dimming), preview pixel rounding/clipping and animation. The native outgoing samples in `/tmp/messages-reply-motion/` grow about 15% for short bubbles and about 26 points in width for wider ones; iOS/web previews now use this bounded scale without reflow, preserving source-edge alignment and space for receipts/actions. Final UIKit rounding/clipping remains. A public UIKit target preview reproduces this growth (`/tmp/messages-context-study/`), but a reaction button inside its custom preview receives no tap: UIKit commits/dismisses instead. Preserve interactive reactions before adopting that presentation. The duplicated source balloon and receipt now stop painting while their preview is active, preserving geometry and restoring on close; compare native lift/return timing and treatment of surrounding badges/avatars. Palette/action-card glass is in place. Paired native captures now establish long-press dimming as the same 21% tinted layer in light/dark and double-tap as about 10%/50% black; twelve browser/iOS captures and seven physical cases pass (`/tmp/messages-tapback-compositing/`). Finish the remaining one-level channel difference, glass opacity/refraction and system-keyboard dimming. The public regular-glass probe approaches the native palette, while tint/underlay substitutions worsen it; identical fixtures on both Simulators rule out that device-setting explanation (`/tmp/messages-glass-palette/`). Isolate the action card’s backdrop/containment before changing the shared material. The three-round direct Exact fixture comparison stopped with the missing `viewport-fit="cover"` setting identified; no host or app material change landed. Long-press now hides the keyboard and adds actions; double-tap shows only reactions with the keyboard retained. Physical iPhone drives verify both paths and prior-focus restoration. The earlier far-right outside-dismiss miss is not reproduced by twelve current physical entry/dismissal pairs and eight ordinary-launch pairs in light/dark with prior focus and existing badges (`/tmp/messages-outside-dismiss/verification.json`); its historical cause remains unknown. A non-key-window probe at ordinary UIKit levels does not reproduce the measured keyboard dimming, despite retaining editor focus (`/tmp/messages-keyboard-dimming/verification.json`); no overlay integration follows.

- Messages emoji picker: integrated public UITextField emoji mode, native search/selection, attached smile control and Close; browser/iOS driver cases and physical Simulator software-keyboard search/selection plus long/double Close pass (`/tmp/messages-emoji-integration/`). Draft and entry-focus policy are retained separately. Still owed: joined thought-bubble/glass artwork, animation, stickers, native incoming large-control geometry (currently mirrored), and removal of the public keyboard's ABC/dictation differences if a supported native path exists. Integrated hardware keystrokes did not populate Search Emoji; software taps did. Per-person reaction storage and grouped participants now pass the clean foreground-verified physical checks; final artwork and the transcript displacement behind crowded panels remain owed.
- Messages selection: finish selection/deletion motion and scroll anchoring. Delete/Forward now use 22-point regular native images; four iOS Simulator touch cases and four browser cases pass with preserved control/transcript frames and drafts. Fixed-crop reference shape overlap improves to 89–93%; foreground/material and edge raster differences remain (`/tmp/messages-selection-symbols/verification.json`). The third/final selection candidate retains hidden receipt space and reserves ten points above its toolbar; the matched outgoing iPhone fixture now moves up 17⅔ points versus native 17, instead of down about ten. Incoming displacement is corrected to 40⅔ points, with a stationary selection-control column. Four physical iPhone and four browser individual/group light/dark cases pass; about ⅔ point of displacement error, off-bottom anchoring and complete motion remain (`/tmp/messages-selection-indent/verification.json`). More now preselects the pressed message, supports multi-selection and count-specific confirmed deletion, and preserves the draft with the keyboard closed. Forwarding now opens a populated UIKit recipient sheet and sends the selected text locally; populated drafts resist downward dismissal and empty drafts permit it. Finish the small dark corner-edge difference and transition timing; the duplicate light-mode corner seam is fixed. Sheet-header swipes now keep the source route, draft, and keyboard in place. Selected-message Delete now uses the shared HTML modal dialog and existing UIKit confirmation owner, removing its application open-state flag and cancellation overlay. Rebuilt iPhone physical one/two-message light/dark checks match the native action rectangle; browser outside dismissal preserves selection and first/second Escape close confirmation/selection respectively. The two-session dialog fixture passes retained editing, reload, unmount and destruction (`/tmp/messages-modal-confirmation/`, LLP 1021 D2). Finish native Close/Forward foreground dimming, browser visual polish and transition timing. Three standalone UIKit variants show that tint adjustment, glass-button configuration updates, and disabled controls do not reproduce the native dimming; do not infer app `disabled` from its pixels (`/tmp/messages-confirmation-dimming/`, LLP 1035.004 D6). Match the remaining menu actions and reaction-participant presentation to native Messages.

- Apple line-clamp: the missing ellipsis is repaired and visible on a long Messages quote. Remaining: the token takes the last visible run's style instead of the paragraph's (the mixed bold/red link fixture differs in both token appearance and retained characters); the browser fixture does not show its right-aligned token. Compare those boundaries before claiming full parity (`/tmp/messages-line-clamp/verification.json`). Padding itself now paints at the content origin and wraps at its content width.

- Messages Translate (1035.001 D4): implement the direct action’s message/conversation choice and system translation presentation. Native captures establish the 240×246⅓ choice and half-height first-use sheet; a public UIKit/SwiftUI adapter matches six native control rectangles and passes expansion/Close/reopen/drag dismissal. Existing Select → native edit-menu Translate in production Exact retains its editor and draft through Close and resumes composer typing. The first-use consent was cancelled; results, conversation translation and later focus behavior remain unverified. Apple’s sample excludes iOS Simulator translation execution; use an appropriate physical-device reference for those states. The one-action confirmation projection and one-large-detent route sheet do not cover the direct flow (`/tmp/messages-translation/verification.json`).
- Messages inbox title (1035.001 D9): integrate general native title/content-scroll intent, first-geometry ownership and a coherent browser projection. Early navigation installation starts the paired prototype expanded at 168; late installation starts compact at 116 independently of content extent (`/tmp/messages-first-layout/`). Separate presenting/sheet owners preserve expanded and compact states through Compose. Public XCTest now proves short-list held collapse and release expansion, long-list compact rest at 116 and real pull-to-top expansion to 168, and held/cancelled sheet owner preservation (`/tmp/messages-title-touch/results-r2.json`). The previous clamped wheel result does not establish a stuck title. The Apple batch now preserves natural content extent; native iOS/macOS growth/shrink/resize checks pass, and the title prototype uses it directly through real touches (`/tmp/messages-title-content/results-r2.json`). Keeping the authored header slot with automatic native insets disabled left a 52-point gap and was rejected. Remaining: title/button authoring must work for Caltrain too; header elision and available CSS space need a principled mapping; initial-owner installation and coalesced refits are now integrated with a shared route projection; production keyboard/Back and two-session reload/destruction pass, and the initial-modal trace exposed and repaired presentation before first draw (`/tmp/messages-owner-install/`); browser projection and arbitrary gesture reversal remain open. The semantic-header/content-origin prototype removes the gap and its third candidate passes isolated real-touch long-list collapse/expansion and sheet cancellation, but authored scroll assignments still disagree with the browser by the changing native inset (`/tmp/messages-header-origin/`, 1035.001 D9); all three implementation rounds are stopped. No title projection has shipped. Previous detach/reparent/zero-frame and desktop-pointer loops stopped after three rounds; the new XCTest path supplies the later bounded gesture evidence. Temporary testId selectors and actual-child extents are not production semantics.

- Apple baseline precision: the kernel preserves fractional frames and `TextEngine` preserves authored fractional line heights, but intrinsic widths, normal paragraph heights and painted baselines still round to logical points. A same-font iOS 26.5 UILabel/CoreText/WebKit fixture found that removing that rounding improves some native labels but does not consistently match either UIKit or WebKit. Resolve line-box/baseline placement with a targeted comparison before changing painting globally; Removing all measurement ceilings changed Caltrain’s established scroll extent and canvas readback; investigate those intrinsic metrics separately. Authored line-height precision alone does not establish glyph fidelity.


- Dynamic style diagnostics: a conditional `top` branch containing `"0px"` compiled but poisoned the runner with `WrongKind` on activation (`/tmp/messages-panel-placement/late-failure-web.json`). Accept the CSS length or diagnose the unsupported value before dispatch; the Messages branch currently uses the supported `"0%"`.

- Continuous release loop (LLP 1030.003 r2, 2026-09-12): release id + signed `published` and the `use`-able version screen (Caltrain first), one hosted Interview release on the directory origin behind its tunnel, `--watch`, iOS through TestFlight (the archive verb, upload, `--status`, version/build numbers), safe activation + the web page's own check, the EAS macOS-worker spike for the Apple lanes (D8, Charlie's 2026-09-12 ask), then the EAS Hosting spike. D6a install pages and D6b's Mac-local development build/install are landed; distributable native artifacts and providers remain pending. §6's six questions await Charlie; other slices await an implementer.

- Shared-target filesystem helper: the ada0b93 workspace sweep failed with `spawnSync .../exact-filesystem ENOENT`; the working checkout’s async reader also intermittently returned 404 when its helper exited after one request. Reproduce helper executable identity/lifecycle across worktrees before claiming a full green sweep; evidence in `/tmp/interview-snapback4-20260913/exact2-checks/` and `/tmp/interview-snapback4-20260913/ui/`.
  LLP 1039's web smoke passed every assertion but retained its `exact-filesystem --serve-reads` child after printing success; closing that recorded child let Bun exit 0. Close the helper at the end of a finite bake/smoke process (`/tmp/lane-router/1039/smoke-web.log`, `launched-pids.txt`).

- Diagnostic source capture: the September 14 optional suite passed 55/56 cases; its source-isolation/cleanup assertion failed. A focused reproduction was interrupted after walking `snapback-sb4/snapback4/target/debug/deps` as ignored source for minutes; the assertion's cause remains unconfirmed. Exclude actual Cargo output directories of local dependency workspaces, while preserving ignored real inputs: derive output roots from Cargo instead of excluding every directory named `target`.

- Native development URL latency: Charlie observes about one second for Update Lab Contract edits on the physical iPhone while localhost clients update almost immediately (2026-09-14). Notification already uses SSE. `PlanURL` now fetches up to four verified payloads concurrently after the envelope; it still creates a fresh URLSession per fetch and refetches unchanged payloads. A physical-phone sample measured envelope 56.5 ms, payload fetches 1,347.8 ms, and prepare/commit 32.1 ms (`target/update-lab-phone-timing.json`). Payload fetching dominates this sample; isolate connection setup versus transfer, then consider a shared session and verified reuse by digest. Preserve byte limits, complete-generation validation and failed-candidate retention.

- Update completion event (Charlie, 2026-09-14): expose one app lifecycle notification after an admitted generation commits and its executors are ready, covering Contract, TypeScript and Rust. Include which artifact digests changed; refused/superseded candidates emit nothing, and initialization stays after first pixel. Update Lab now uses its existing 250 ms task to show executed probe versions automatically; an event would remove that sampling delay without making real apps poll. Define the shared Contract lifecycle hook before adding separate host callbacks.

- Fieldnotes aggregate library size: the app admits 1,000 notes of up to 20,000 UTF-16 units each, but `library` reads every body in one SQLite result. Real Chrome with 1,000 emoji notes at the per-note limit returns `query result exceeds 16 MiB limit` and cannot open the notebook. Paginate/search in SQLite and load full bodies only for the selected note so allowed stored data remains readable; preserve ordering, filtering, IDs and draft guards. Backup now uses one size-bounded SQLite statement in both languages, preserves one read snapshot and returns the existing 4 MiB backup notice. A generic worker would not repair this result-size failure (LLP 1027.001).

- Worker placement follow-ups (LLP 1027.002, landed 2026-09-14): drive the placed Fieldnotes on an iOS simulator through the shared Apple host; a macOS frame-gap instrument (a display link's callback cadence in `state`) so §5 step 5 is judged on the device, not only in Chrome; a forgotten ticket's held call still takes its turn (D5 says it may be removed before it starts) — tell the composer when the runner forgets; Rust on a web Worker waits behind a consumer (the NOT-DOING take); the development producer refreshes a Rust-owned resource's first-frame value only at a Cargo bake, and refuses a Rust source shared by several resources until then.

- Rust replacement latency follow-up (2026-09-14, LLP 1029.000): native image preparation now runs off the UI thread and the bake helper stays resident. Six Rust edit/restore cycles reached web, macOS, simulator and iPhone with carried state; three distinct new libraries took 2.6–4.6 s save-to-observed, with macOS probe round trips ≤48 ms. First executable launches can still wait on macOS assessment (including the filesystem helper at dev-server startup). Measure the remaining compiler/producer overhead toward the subsecond warm-edit target; the 20-edit timing and 50-replacement retention probes remain owed. Evidence: `target/update-lab-distinct-repeat-result.json`, `target/update-lab-baker-reuse-check.log`. Later normal-app preparation took 17.6 s behind a ~31 s load. Isolated real-library loads reproduced 70.7 s ad-hoc and 39.1 s development-signed inside `dlopen`, versus <1 ms write/sync in the ad-hoc case. Gatekeeper assessment and concurrent 30 s `syspolicyd` QUIC timeouts are recorded in `target/native-bottleneck-security.log`; loader stage diagnostics now separate queue/write/sync/load. Signing alone is not a fix. The opt-in stateless `tiered` executor now runs the new Wasm while native mapping proceeds, then promotes without a second reload (LLP §4.1); this leaves the OS loading cause and producer timing as follow-ups. A fresh Rust save in the Launch Services app then published in 2.93 s but spent 103.9 s in `dlopen`; the main thread stayed in its event loop. Restore preparation reused the mapping in 19.8 ms, with a separate 36.2 s producer/publication delay still to attribute. The tiered live proof subsequently passed with state/TS retained: 25 ms candidate preparation on two saves, plus 36 ms preparation while a normal-app native load took 54.3 s. Web/simulator/iPhone save-and-restore also passed; the earlier JS-engine-not-loaded diagnostic did not recur in the native traces (`target/update-lab-tiered-result.json`, `target/update-lab-tiered-cross-host-result.json`). Cold helper launches and shared Cargo locks still made initial builds take minutes; this is separate from tiered activation.

- Messages native debug startup (2026-09-14, LLP 1027.001): three conversation tests exceed the 100 ms executor guard during synchronous native Snapback/SQLite initialization. Serial A/B on the same integrated runtime measured 132–151 ms with either the standard TextEncoder or the previous private UTF-8 counter; the helper replacement is not causal. All 13 optimized Messages tests pass. Profile native initialization and its scheduling separately; do not raise the guard or describe the full debug workspace suite as green. Evidence: `/tmp/exact2-language-parity/logs/de55b63-messages-current-serial.log` and the adjacent old-helper log.

- Worktree-contained test fixtures (2026-09-14, router chunk (c)): `exact-apple --test inherited` / `apple_artifacts_own_paths_locks_identity_and_failed_placement` creates standalone Cargo packages without their own workspace boundary. With `TMPDIR` inside this worktree, Cargo captures them into the repository workspace and refuses metadata. Three attempts stopped; make those generated packages explicitly standalone. Evidence: `target/router-test.log`, `target/router-test-retry2.log` in `exact2-wt-router`.

- Storage reload assertion (2026-09-14, router verification): the workspace sweep failed `exact-js --test storage` / `unload_invalidates_continuations_and_configuration_survives_reload` with `"cancel"` instead of `"again"`; the focused seven-test storage rerun passed. Reproduce the cancelled file operation/reload interaction before claiming the full workspace sweep green. Evidence: `target/router-test.log`, `target/router-test-retry.log` in `exact2-wt-router`.

- Web smoke teardown (2026-09-14, router verification): `bun scripts/smoke.mjs web` printed `web smoke: ok in 34.6 s` and all three Caltrain tests passed, but the Bun process remained alive afterward; the launched exec session was interrupted (exit 130). Identify the retained handle and make successful smoke runs exit naturally. Evidence: `target/router-smoke.log` in `exact2-wt-router`.

- Interview router row 4 (LLP 1038 F10): seed an isolated agent-mode replica for the own-data/scroll swipe drive; first finish the static fixture’s held swipes with Simulator unobscured (other desktop apps blocked contact).
- Interview iPad-wide iOS (LLP 1038): drive the rail on a real wide simulator viewport; the iPhone agent ignores `--size` and reports 402 × 874 for both requested sizes.
- Interview backend list arguments (LLP 1038): let `/state` accept question/person/post ID lists and return every stacked record; the pre-replica path currently sends only each list’s first ID.

- Messages macOS agent menus (2026-09-14, LLP 1038 slice 2b): `MenuHost.sync` leaves all popover subtrees painted under `EXACT_AGENT=1`; a conversation tap hits the topmost confirmation's Cancel text and dispatches no press. Reproduced with both the original and routes-based Messages Contract; the generic smoke is green. Make the existing agent menu projection driveable before claiming the full Messages interaction matrix on macOS. Evidence: `/tmp/lane-router/s2b/messages-macos-baseline.json`, `messages-macos-fixed2.json`, `messages-macos-hit.json`.
- Web smoke/agent exit (2026-09-14): successful Messages commands can leave Bun alive after their final success line. Inspect the resident filesystem reader's idle pipe references (`scripts/filesystem.mjs`, notably stdin). Evidence: `/tmp/lane-router/s2b/smoke-messages-web.log`, `messages-web-timer.log`; the smoke also runs to completion when the caller exits after the module's completed assertions.
# Graceful-overload consumers (2026-09-16)

LLP 1041 starts `messages-stress` and `completion-storm` as opt-in synthetic
workloads. Next: compare idle/loaded interaction samples on a real 120 Hz device,
then implement bounded completion pumping and admission/fairness where measured.
Keep module placement under LLP 1027.002; automatic windowing/image pressure and
physical-display native sweeps remain follow-ups; the first slice drives AppKit
and the Linux headless CPU renderer without claiming display frame timing.

- Linux textflow follow-up (LLP 1043.000 M3): carry resolved exclusions in immutable content-region worker requests and their accepted artifact identity. Ordinary definite-height text now flows; the opt-in content-region publication still owns its original unflowed geometry. Linux has no native text-selection UI, decoration painter or inline-link hit geometry yet; future consumers must use the accepted paragraph's fragments and original run metadata. Add discretionary-hyphen ink and clamp ellipsis to the flowed path (M3 clips at the band cap); justify currently uses start.

- Apple textflow follow-up (LLP 1043.000 M4): the shared walker now paints native paragraphs on macOS/iOS. A resolved flowed leaf retires an opt-in opaque content-region raster and returns the whole registration to native views, until a new registration. Carry exclusions and fragment geometry through the worker artifact before retaining region rastering for moving shapes; the current fallback trades that worker scheduling for correct fresh ink.

- Web textflow size review (LLP 1043.000 §6.2, M5): optimized textflow-web grew from the observed pre-M5 834,019 bytes to 905,398 bytes (+71,379), above the approximate 64 KB revisit threshold. Caltrain is currently 1,022,149 bytes; the orchestrator has the pre-program comparison. Review code/data size while preserving the ruling that the shared crate stays linked. Evidence: `target/textflow-scratch/m5/size-baseline.json`, `wasm-probe.json`.
- Public UI coverage (LLP 1035.006): select the first settings slice (labels, buttons, switches, checkboxes, radio groups, and text entry), choose each platform's disposition, then implement and hand-check the concrete examples. The ordered catalog distinguishes accepted support from reviewed deferrals; drafting it does not admit every excluded capability.

- Tooling follow-up (2026-09-19): `bun scripts/caps.test.mjs` still passes 59/61 after the Snapback 0.2.30 recovery. The absent-root watcher misses deletion (`created=true, deleted=false, recreated=true`), and diagnostic source/output isolation and failure cleanup fails. These match the saved 2026-09-17 findings; Apple packaging checks pass.
- Video (LLP 1042): verify PiP/AirPlay on physical devices; complete the designed track/controller and app audio-session ownership APIs before promising captions, background playback or DRM. Linux carries an explicitly unavailable video box.


- Hermes incremental rebuild inputs (2026-09-19): `js/build.rs` captures macOS/Linux engine archives in OUT_DIR but only watches iOS archives; replacing the external compiler/VM can leave a warm native build stale. Add upgrade invalidation together with `scripts/app.mjs` receipt identity support for external SDK inputs; simply adding external rerun-if-changed paths currently makes bake receipt capture refuse them. Until then, clean exact-js when provisioning a new compiler/engine.

- Linux baseline failures found during dependency verification (2026-09-19): `exact-gpu` references Apple-only `SurfaceTargetUnsafe::CoreAnimationLayer` on Linux; Hermes ICU `DateTimeFormat.formatToParts` returns timestamp text (same with old 6badada and new e3371863); Ibex loader-case fixture expects a case-insensitive filesystem. Repair separately from the dependency upgrade; current Linux full-workspace checks are not green.
