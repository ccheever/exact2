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

- **Mixed by default, and the wasm executor on wasmtime** — LLP 1029 (Draft RFC,
  2026-09-03) over LLP 1028 (the measurements: wasmtime runtime +0.49 MB / +Pulley
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

- **Delivery, unified** — LLP 1030 r2 (Draft RFC, 2026-09-03: every layer's artifact,
  identity, and carrier in dev and production; the per-platform compatibility id and its
  streams; the classifier per platform and stream with dependency computed from the graph;
  the two axes — an update store or not, the executors linked; propagation at its real
  strength; the GPU sub-layers with shaders binary-coupled until packaged; the runtime
  upgrade — freeze only, the store-carry test; assets hot in dev, icons per platform) and
  LLP 1030.000 r2 (the bundle slice: `dev.mjs` classifies on every edit and never
  publishes, `exact deploy` in its own process — snapshot, bake, classify, dry-run table,
  conditional-put publish per stream; the policy table; the binary lanes, landing, reach,
  brownfield deferred to a later number). Both folded a two-round grok 4.6 xhigh + codex
  sol ultra panel (`llp/reviews/1030-delivery-unified.{grok,sol}.md`). Ruled 2026-09-03:
  continuous everywhere (`release: automatic`, phased by the store); the update economy
  off NOT-DOING in its static form. Every §5 question ruled 2026-09-03 (shader packaging in stage 1; the conditional put required). Landed: stage 1 (the asset row, 2026-09-04), stage 2 (the manifest, 2026-09-03), and of stages 3 and 4 the compatibility id, the `delivery` resource with `state.delivery`, the update store as a crate, and — 2026-09-04 — `exact deploy` (`scripts/deploy.mjs`: snapshot, bake into `target/deploy/<release>`, the per-platform, per-stream table with the origin row as the default dry run, `--yes` through the directory origin adapter `scripts/origin.mjs` with the lock-file conditional put, blobs read back, the signed head, the release record; `keygen`; `node scripts/smoke.mjs deploy` drives it; `update/tests/publisher.rs` is the cross-language proof over a head it wrote). Owed: `--watch`; an object-store adapter (https is read-only); the artifact graph with dependency computed from it and the icon roster refusal (the classifier compares digests against heads and treats an unchanged compatibility id as the cohort check); `dev.mjs` importing the classifier for its table; `Store::check` fetching `.exact/<compatibilityId>/` while the origin is laid out `.exact/<channel>/<compatibilityId>/` (the store should take the channel from `compat.json`'s `delivery.channel`); `host/web/serve.mjs` serving `.exact/` (it refuses every dot path but `.well-known`); bake writing the stream card and signature into the embedded `exact.json`; the hosts opening the store and checking after first pixel; the dev policy folded onto the store; the Linux asset row; iOS driven for the asset row and `state.delivery`; a client driven to show the sunset card.

- **The update store is built** (`update/`, `exact-update`, 2026-09-03) — LLP 1026 D9–D12
  and LLP 1030 D3a/D9 as store logic alone, no host wiring: the envelope with its stream
  and its signature, the canonical bytes a Node publisher has to reproduce, entries
  whole-or-absent under their envelope digest, selection at launch with entry zero as the
  fallback, the crash counter, the anti-rollback and wrong-cohort and wrong-app refusals,
  files reused from another entry by digest, and `staged`/`activate`/`status` for LLP 1030
  D7's two acts. Twenty tests over a temp directory and an in-memory origin. Owed before
  it delivers anything: bake writing `stream`, `release`, `signature`, and the embedded
  bundle's own digest into `exact.json` (the stream heads on the origin carry all three
  since `exact deploy` landed 2026-09-04 — `update/tests/publisher.rs` reads one; the
  bake's own embedded `exact.json` still carries none); a host
  calling `open` and the two boot marks and running `check` after first pixel on its own
  executor, on Apple, Linux, and the web; the `delivery` resource and commands with
  `state.delivery` beside them; and the dev policy folded onto the same store (1026 D12),
  which is what retires the file poll and the two URL loaders.

- **LLP 1031 owed after the 2026-09-03 landing** (the handle, `ExactKit`, the macOS
  sample host, the manifest stage; the iOS sample host landed 2026-09-04 — each pane its
  own navigation controller, `smoke.mjs host-ios`): **content-height containment** (D3's
  guard is written; bounded only today); **the request and store crossings** (D4's
  contract; triggered by an adopter whose client cannot be wrapped); **packaging**
  (`build.mjs --embed` and the link delta against `floor.swift` in `metrics.mjs --long`);
  a request in flight under a destroy (needs a fetching app — Weird Castle — as the
  fixture); and the host smoke's last step, which takes ~4 s after remounting a
  destroyed session's view (the `clock settle` that follows) — find out why.
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

- weird-castle declares its identity: `fn app_id(&self) -> &str { "com.exact.weird-castle" }`
  on `Castle`'s DataSource impl — one line, waiting only because data/src/lib.rs was
  mid-edit in another session on landing night (LLP 1023 Stage 2, 96ed314). Until then
  its plans bake unnamed and boot everywhere; after it, a weird-castle plan served to a
  caltrain binary is a refusal naming both apps.

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
  ibex2 (URL, TextEncoder, base64), a `Date.now` shadow onto the runner's clock. The
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
