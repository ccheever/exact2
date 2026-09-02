# Queue

What would make sense to do next, in rough order. This is not a spec and decides
nothing: `rules/RULES.md` §Scope says a spec without an implementer and a date isn't
written, so a line here is one to three lines — the thing, why, what it needs. When
someone picks one up it becomes an LLP with a name and a date (the working set is at
15 of 15, so a link gets archived); when it lands, delete the line. Anyone, agents
included, may add or remove a line. `rules/NOT-DOING.md` still binds: an entry that
sits on that list carries the trade it would take.

## Next, in order (2026-08-29)

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
4. **Text selection.** Free on the web (nothing sets `user-select: none`); none on
   macOS (CoreText paints per node, LLP 1008 §7). The hard part is cross-node
   selection, the browser's document model. Host state, like scroll offset, never plan
   state. `Paragraph` keeps its `CTLine`s, so hit-to-index is available. After 6.
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
  snapping; a scroll offset or event reaching the runner.
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

- **Windowed `List`** — when a list misses 60 fps. Needs three things the seam lacks
  (LLP 1010 §5): a logical total extent, window-origin compensation, and a
  scroll-offset event to the runner. `List` and `ScrollView` are the same thing on
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
  under two loaders; design that first. Zero app JS until then.

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

- weird-castle declares its identity: `fn app_id(&self) -> &str { "com.exact.weird-castle" }`
  on `Castle`'s DataSource impl — one line, waiting only because data/src/lib.rs was
  mid-edit in another session on landing night (LLP 1023 Stage 2, 96ed314). Until then
  its plans bake unnamed and boot everywhere; after it, a weird-castle plan served to a
  caltrain binary is a refusal naming both apps.

- LLP 1024 (native modules) is r2 after the 2026-08-31 three-model panel — unanimous,
  design settled (one app artifact, roster-at-bake, versioned table ABI). What it
  waits on is Charlie: ratify §6's Q1 leaning, and name a consumer + implementer;
  D8 does not start without both.
