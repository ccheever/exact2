# Queue

What would make sense to do next, in rough order. This is not a spec and decides
nothing: `rules/RULES.md` §Scope says a spec without an implementer and a date isn't
written, so a line here is one to three lines — the thing, why, what it needs. When
someone picks one up it becomes an LLP with a name and a date (the working set is at
15 of 15, so a link gets archived); when it lands, delete the line. Anyone, agents
included, may add or remove a line. `rules/NOT-DOING.md` still binds: an entry that
sits on that list carries the trade it would take.

## Next, in order (2026-08-29)

- **LLP 1035 follow-ups** (2026-09-09): landed the same day — inherited rows
  (1035.000 slice 1), `layout <node>` (1035.002 slice 1), a contact's phases on
  the web and macOS carriers (1035.003). Next, in the umbrella's order: the
  Simulator contact gate (1035.003 §3; the backend is Charlie's call), 1035.001
  slice 1 (XCTest units for the navigation/modal rules, journal lines for
  refused intents, a session-scoped blur in the two-session host), 1035.004's
  six symbol roles, 1035.005's `contract fmt` with the continuation rule;
  `dynamic` as a `layout <node>` source once a constant binding can be told
  from an expression; `metrics.mjs` printing the inherited-invalidation cost on
  Messages; the reply tags on `tap`/`type`/`clock`/`screenshot`.

- **Percentage width inflates an auto-height flex column in native layout**
  (2026-09-08): LLP 1032's blocks ended at 30,145 points while Taffy sized
  their `width: 100%; max-width: 720px` column to 67,373, leaving a white
  phantom scroll range. The readers use the CSS-equivalent definite preferred
  width plus `max-width: 100%` meanwhile; reduce this to a kernel fixture and
  make the original CSS shape agree with the browser.

- **Two concurrent app builds ship each other's binaries** (2026-09-08):
  `host/apple/build.mjs` links every app's `ExactMac` into one product path
  (`host/apple/.build/<triple>/release/ExactMac`, staged through one
  `.build/composition-<composition>`) and then copies it into that app's
  bundle. Two builds of different apps at once — two terminals, two agents —
  race, and the loser's `.app` has one app's Info.plist over another app's
  executable. Seen for real: `LLP.app` launched as Weatherlight, and
  `scripts/agent.mjs` (which drives that same shared path, not the bundle)
  drove Weatherlight for `--app llp`. `scripts/exact.mjs` now refuses a
  bundle whose executable carries another app's id, but that is a smoke
  alarm, not the fix: the product and scratch paths want to be per-app.

- **`line-height` is a length, not CSS's ratio** (2026-09-08): the kernel's
  `line_height` row is absolute points and a unitless `1.62` draws every line
  on top of the last. CSS's unitless value is a multiple of the font size,
  and `rules/RULES.md` §Scope says a semantic that could follow CSS follows
  it. Either implement the ratio or declare the deviation in LLP 1001 §1.
  Found writing the Markdown readers (LLP 1033).

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
   geometry still need dedicated fixtures. The long-document pass brings scrolling
   to ~8 ms, but narrow-width full reflow remains ~28 ms on LLP 0566; lazy block
   measurement with stable scroll anchoring is the next performance candidate.
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

- **Rust development reload** — **LLP 1029.000** (Draft RFC, 2026-09-09): native-first Rust
  logic, native shared-library swaps, browser Wasm, Pulley on physical iOS;
  whole modules first, phone/native-loader measurements owed. Production Rust
  delivery and per-source digest analysis move behind the proposed dev loop.
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

- **Windowed `List`** — when a list misses 60 fps. Needs a logical total extent and window-origin compensation
  (LLP 1010 §5); scroll-offset events now reach the runner on web and Apple. `List` and `ScrollView` are the same thing on
  every host today.
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
  phone was asleep on landing day — `node host/apple/build.mjs --device --run`, then
  4-finger tap → Open Project… → the LAN URL dev.mjs printed; ATS and signing already
  proven on the built bundle); the Linux display loop's live SSE half (fetch.rs boots
  once per run today — subscribe-and-swap needs a Linux display to verify on); a
  native pre-download `kernelSchema` compare (needs the client's own digest exported —
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
  (delivery), stage 7 (the engine crate + Linux); owed from stage 3: the pure tier from
  ibex2 (URL, TextEncoder, base64). LLP 1027.000 landed 2026-09-04: time and seeds
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

- **Messages iPhone parity** (`apps/messages`): the local chat example now runs on
  web and iOS. Finish native back/Tapback/reply gestures, timestamp motion matching (recognition threshold and release curve; resisted/capped travel and stationary labels now work; extent/rate/paging probes rejected, a direct UIKit spring is closer but not integrated),
  anchor-removal fallback and typing/insertion motion, then compare actual touch
  behavior and screenshots with Messages.
  `apps/messages/README.md` records the current implementation gaps.
  The focused reply surface, swipe entry, and reply indicator are in place; finish thread positioning,
  and connecting lines. Transparent curved tails now work over its material.

- Border parity: an explicit edge width/color paints on iOS but remains invisible on the web without a border style; define and implement CSS `border-style` consistently before relying on border geometry for icons. Observed in Messages back-chevron rendering.

- Messages navigation: after three integration fix rounds, native header back-swipes cancel/complete with drafts, but the timestamp horizontal scroll still consumes rightward gestures over the transcript. An app-only directional `touch-action` probe enabled edge Back but not gutter Back; reverted because the fresh simulated Messages reference ignored the corresponding navigation gestures despite working timestamp input and button Back. Establish a reference that demonstrates the gesture before calling that change parity; no engine gesture arena.

- Messages reactions: per-person blue/gray badges, independent add/remove, inner-edge anchors and grouped participants now pass clean physical main/focused/group checks with the named Simulator verified foreground. The reproduced input miss was a browser covering the Simulator, not a delivered UIKit touch (`/tmp/messages-touch-phases/`). The participant/menu region now prevents keyboard-open overlap with the measured 24-point gap. Source anchoring through keyboard dismissal and transcript displacement behind a crowded preview now pass iOS/web checks (`/tmp/messages-context-anchor/`), including restoration and the focused thread's retained clip. Measure badge travel with a real native iMessage reply gesture, then match it; the SMS fixture only supplied drag-and-drop (`/tmp/messages-reply-badges/`). Finish native artwork. The participant popover's rectangular shadow cutoff is repaired by separating its glass from its scrolling content (`/tmp/messages-popover-shadow/`). Preview entry modes now match the incoming/outgoing native evidence: badge and double tap keep the original size; long press magnifies, including the picker (`/tmp/messages-preview-modes/`). Native focused-thread scale is still unmeasured; Exact applies the same entry policy there. Compare focused-thread Close's keyboard policy (current physical Close blurs the composer).

- Messages compose: finish native recipient token editing, invalid-address presentation/international formatting, native sheet motion and existing-history presentation before send. UIKit modal presentation and empty-draft gesture dismissal are now in place; the live source view follows appearance changes while a sheet is open; finish the small dark corner-edge difference and complete transition timing. Multiple fixture recipients now create/reuse local groups on send. The dedicated fixture recipient sheet replaces the old inbox-search shortcut.

- Investigate intermittent Linux update test `a_refused_initial_layout_releases_no_network_requests`: the workspace run observed one request after refused layout; isolated and full-workspace reruns passed (Messages compose validation, 2026-09-09).

- Messages inbox gestures: finish trailing full-swipe confirmation, Recently Deleted/recovery, exact glyph size/artwork and transition comparison. Explicit UIKit swipe rows now pass leading full Read/Unread, reversal, partial actions, Mute/Unmute, Delete, row switching, first-tap dismissal, draft/history retention and parent vertical scrolling (`/tmp/messages-swipe-activation/`). Callback eligibility follows the original authored ancestors, excluding UIKit’s temporarily disabled cell. Native action inspection and agent activation pass; the browser retains its authored controls. First-child scale/rotation capture and the pinned light/dark action colors are corrected (`/tmp/messages-swipe-glyphs/`); exact symbol paths remain owed. Open actions still retain their earlier appearance: two adaptive-image attempts passed action checks but corrupted surrounding avatar lettering and were removed at the repair limit. Isolate the rendering failure before another adaptive capture attempt. Real iOS gesture input still uses the temporary foreground-guarded helper.

- Messages contact details: finish call/video/email, contact editing/blocking, group/shared-content sections, and switch motion. Third-route navigation, draft/focus and reading-position restoration, and shared Hide Alerts now work; ordinary-launch button Back and cancelled/completed native swipes preserve the transcript with the keyboard restored.

- Messages balloons: match multiline intrinsic sizing (native sample 262 points wide, Exact 277 with the same three lines), fractional bubble rasterization, and sender-name repetition after pauses. Tail contours now measure about 0.10/0.11 points of outgoing/incoming edge error after normalizing the bubble bottom; complete pixel equality remains unverified. Time-based bubble runs now follow the measured 30/59/61-second cases; the exact 60-second boundary remains unverified. Minimum size, padding, line spacing, transcript insets, and group preview indentation now follow native samples.

- Messages date headings: establish same-day grouping from a stronger native fixture before adding a cutoff. Native Messages and the current web/iPhone app retain one heading across a 301-second gap followed by a 299-second gap; subsequent 601- and 901-second native pauses also keep that heading. Five-, ten-, and fifteen-minute inactivity cutoffs would disagree with this reference; the bounded pause study has not established a threshold. Matched captures and recorded input times are in `/tmp/messages-date-headings/`.

- Messages Tapbacks: Copy now writes the full message and restores the draft/focus; the menu has leading icons and measured row spacing (`/tmp/messages-copy-action/`). Finish Translate and text Select, native symbols, emoji/sticker picker and attached button, exact artwork/compositing (including system-keyboard dimming), preview pixel rounding/clipping and animation. The native outgoing samples in `/tmp/messages-reply-motion/` grow about 15% for short bubbles and about 26 points in width for wider ones; iOS/web previews now use this bounded scale without reflow, preserving source-edge alignment and space for receipts/actions. Final UIKit rounding/clipping remains. A public UIKit target preview reproduces this growth (`/tmp/messages-context-study/`), but a reaction button inside its custom preview receives no tap: UIKit commits/dismisses instead. Preserve interactive reactions before adopting that presentation. The duplicated source balloon and receipt now stop painting while their preview is active, preserving geometry and restoring on close; compare native lift/return timing and treatment of surrounding badges/avatars. Palette/action-card glass is in place; strip dimensions and sampled light-mode dimming colors match the reference; dark-mode native comparison remains. Long-press now hides the keyboard and adds actions; double-tap shows only reactions with the keyboard retained. Physical iPhone drives verify both paths and prior-focus restoration. Isolate a normal-mode far-right outside-dismiss tap miss: the agent drive passed that point, and left-side normal dismissal passed.

- Messages emoji picker: integrated public UITextField emoji mode, native search/selection, attached smile control and Close; browser/iOS driver cases and physical Simulator software-keyboard search/selection plus long/double Close pass (`/tmp/messages-emoji-integration/`). Draft and entry-focus policy are retained separately. Still owed: joined thought-bubble/glass artwork, animation, stickers, native incoming large-control geometry (currently mirrored), and removal of the public keyboard's ABC/dictation differences if a supported native path exists. Integrated hardware keystrokes did not populate Search Emoji; software taps did. Per-person reaction storage and grouped participants now pass the clean foreground-verified physical checks; final artwork and the transcript displacement behind crowded panels remain owed.
- Messages selection: finish selection/deletion motion and scroll anchoring. More now preselects the pressed message, supports multi-selection and count-specific confirmed deletion, and preserves the draft with the keyboard closed. Forwarding now opens a populated UIKit recipient sheet and sends the selected text locally; populated drafts resist downward dismissal and empty drafts permit it. Finish the small dark corner-edge difference and transition timing; the duplicate light-mode corner seam is fixed. Sheet-header swipes now keep the source route, draft, and keyboard in place. Match the remaining menu actions and reaction-participant presentation to native Messages.

- Native modal startup: verify a plan whose initially selected route is a sheet. The temporary bootstrap source-stack adjustment dispatched Back while installing the source route and was removed; normal Messages starts at the inbox. The initial-sheet source background remains unverified.

- Native text padding: a padded Text node allocated the padding but painted its glyphs at the outer origin in the inbox-title experiment. Verify the CoreText draw bounds against a padded DOM text element; ordinary container padding positions the title correctly.

- Messages inbox title: initial title/row spacing now follows native measurements. The remaining travel difference is native navigation-bar inset ownership: a standalone large-title table reproduces native travel while bare UIKit scrolling matches Exact. Integrate title and content-scroll intent in the navigation layer, with a coherent browser projection (1035.001). The public scroll-edge interaction works around UILabel in a fixture, but not around custom-painted text (1035.004); the current app header still uses a uniform material. An isolated real-Messages prototype with the authored title removed and a paired cold generation still initialized compact (inset/first-row y=116, expected expanded y=168); it stopped after three integration rounds. Its action-dispatch navigation retained the draft. The next bounded approach must address controller/scroll initialization, not another title-position constant.

- Apple baseline precision: the kernel preserves fractional frames and `TextEngine` preserves authored fractional line heights, but intrinsic widths, normal paragraph heights and painted baselines still round to logical points. A same-font iOS 26.5 UILabel/CoreText/WebKit fixture found that removing that rounding improves some native labels but does not consistently match either UIKit or WebKit. Resolve line-box/baseline placement with a targeted comparison before changing painting globally; Removing all measurement ceilings changed Caltrain’s established scroll extent and canvas readback; investigate those intrinsic metrics separately. Authored line-height precision alone does not establish glyph fidelity.

- iOS agent wheel bounds: `AgentIOS.scroll` ignores `adjustedContentInset`, so a native-inset scroll view can have a reachable negative offset but receive no wheel movement when content size equals bounds. Reproduced in the isolated Messages navigation-header prototype. This is a driver limitation, not evidence that a finger cannot collapse the header.

- Dynamic style diagnostics: a conditional `top` branch containing `"0px"` compiled but poisoned the runner with `WrongKind` on activation (`/tmp/messages-panel-placement/late-failure-web.json`). Accept the CSS length or diagnose the unsupported value before dispatch; the Messages branch currently uses the supported `"0%"`.
