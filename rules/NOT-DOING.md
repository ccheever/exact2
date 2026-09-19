# What Exact v1 Does Not Do

This is the most valuable file in the repo. It is longer than the doing-list on purpose.

## The bar that makes this list derivable

v1 is done when **one real application** — not a demo — runs from a single Contract
source on web, macOS, iOS, and Linux, hitting the time budgets in `RULES.md`.

**Decided (Charlie, 2026-08-30):** the Caltrain app is the one that defines v1 — it
already exists in Contract, it has real lists, navigation, search, text, and theming, and
"does it still work" is answerable in seconds — **and Weird Castle's wordmark is in v1
beside it**, which puts one bundled brand face in scope and nothing else about that app.
Everything not required by those two does not exist. What the wordmark unblocks: a
declared font, LLP 1019. Fonts were never on this list, so nothing comes off for them.

**Expanded (Charlie, 2026-09-05):** the Markdown viewer (LLP 1033) is a third
consumer: macOS file opening and comfortable reading first, shared with iOS and
web. Take: native-module implementation (LLP 1024, awaiting a consumer) moves
behind the reader. Blog/CMS and publishing remain out.

**Expanded (Charlie, 2026-09-07: "make a real app that uses them"):** Fieldnotes
is the storage consumer: notes in SQLite, backups in app-scoped files, and a
multiline editor on web and Apple. Take: further general-purpose API expansion
waits behind proving these shipped bindings in the app; Snapback2 stays deferred.

**Expanded (Charlie, 2026-09-12: "Include the Messages conversion"):**
Messages is the Snapback4 consumer: the separately linked Rust device on
Apple/Linux, its TypeScript replica on the web, and the existing host SQLite
and network capabilities. This unblocks durable conversations and offline
writes across devices. Take: generalized native-view module loading remains
behind proving this data-only consumer; Snapback2 stays deferred.

**Expanded (Charlie, 2026-09-14):** language parity for existing app operations
(LLP 1027.001): shared standard text/URL helpers, portable storage requests,
reusable mixed composition, and Fieldnotes backup in either language. Unblocks
moving app logic without changing durable data or permissions. Take: remove
app-private text helpers and Update Lab's private composer; generalized native
views and multi-module scheduling stay behind this consumer.

**Expanded (Charlie, 2026-09-14: "let's build this now"):** optional worker
placement for data modules (LLP 1027.002) — one module instance, one executor
owner, values cross and the runner commits; `main` stays the default and the
manifest opts a module in per platform. Unblocks app computation off the UI
thread with the same inputs, results, grants and durable data, ahead of a
measured hitch, for the story and because the limits will be hit. Take: Rust
on a web Worker waits behind a consumer that needs it (placement is
language-neutral on native, TypeScript-only on the web); a second worker, job
pools and load balancing stay out; the mixed-resource first-frame bake is
built with it rather than queued. The measurement bar in LLP 1027.002 §2 says
when a worker is claimed to help.

**Expanded (Charlie, 2026-09-16):** graceful-overload design and two bounded,
synthetic stress consumers (LLP 1041): Messages interaction under load and
async completion storms. Unblocks measuring queue pressure, UI settlement
bursts, and large-history costs on existing hosts. Take: broad new benchmark
apps and a generic job scheduler stay behind these two; no sixth blocking
check, additional worker pool, or parallel layout is admitted by these examples.

**Expanded (Charlie, 2026-09-16, Astra xhigh campaign):** add gigantic Markdown
and continuous native resize to LLP 1041's stress consumers; evolve bounded
native scheduling and viewport collections against these workloads. Take: more
showcase apps and speculative general job pools wait behind measured improvements
to these three. Existing ordered-effect and capability boundaries still apply.

**Expanded (Charlie, 2026-09-16: "Work through all four"):** continuously
interactive Messages, photo zoom, virtualized reorder and a draggable sheet with
a nested collection (LLP 1041 §8.5). Unblocks proving gesture takeover, collection
lifetime and background scheduling together on the existing hosts. Take: other
showcase apps and decorative effects wait behind these four and the original
three workloads. Shared-element presentation and the bounded geometry/motion
work those consumers need are admitted; a generic gesture arena, second
application-state graph and speculative parallel layout remain out.

**Expanded (Charlie, 2026-09-17: one impressive demo on iPhone, macOS and web):**
Exact Live combines crew chat, photos, a runbook and background jobs in one
authored workspace, using existing hosts and data seams. Unblocks showing the
interaction and performance work together in a coherent application. Take:
additional standalone showcase apps wait behind this composition; existing full
stress workloads and controls remain intact. No new UI framework, general job
scheduler or cross-device synchronization service is implied.

## Surfaces

**Expanded (Charlie, 2026-09-13):** replace app Rust below the data seam with
a separately linked native or interpreted Wasm module, in development and
production (LLP 1029.000). Unblocks business-logic updates without a host rebuild.
Take: multi-module scheduling and generalized native-view module loading stay
behind this single data-module consumer; Windows and Android hosts remain below.
**Refined (Charlie, 2026-09-14):** optional Wasm-first/native promotion for
explicitly stateless modules removes OS loading from the update latency path.
Generic executor-private state migration stays out; this is one committed
generation with an executor change, not a second app reload or patch protocol.

- **Windows.** A working Direct2D host exists in the old repo. It is real work, and it
  doubles the native matrix. Port it after the loop is proven.
- **Android.** Same.

Every surface multiplies the sweep, the presenter count, and the number of ways one
change can break.

## Authoring models

- **React tier.** The door stays open — meaning we design nothing that forecloses it.
  We do not build it. No React Facet bindings, no RecipeIR generation, no dual-framework
  parity gate. One authoring model, one set of bugs. Logic below the data seam is
  TypeScript by default or Rust (LLP 1027; Charlie, 2026-09-03); nothing runs
  JavaScript above it — not in Contract, not in the tree, not before first pixel.
- **Rust Native roots.** No Rust-owned UI roots, no deployment-manifest registrations,
  no app-ABI generator.
- **Platform-suffixed route overrides** (`.native.tsx`, `.mac.tsx`, ...). One route, one
  file. If a platform needs different behavior, that is a branch inside the component or
  a bug in the presenter.

## Features carried over as "no"

**Runtime**

- GPU / WebGPU substrate. The old repo carries ~139,000 lines of Rust adapter code
  across two files, neither marked generated. The door stays open the way React's
  does — we design nothing that closes it: an owned-pixel surface is a leaf node
  with a kernel-owned box (like `NativeView`), GPU content never influences layout,
  the host owns the frame, and animatable properties are extensible (CSS
  `@property`). When it is built, it compiles no shaders at runtime (LLP 0559 F8).
- Server generation in every form: SSR, streaming, static export, progressive forms,
  hydration, route payloads, response caching.
- Aquifer data tier, durable worker tier, capability capsules, durable capability grants.
- ~~Snapback / update economy.~~ Moved 2026-09-03 (Charlie: "you can pull out snapback
  for now"), in LLP 1026 D11's minimal form only: an installed app fetches a signed
  bundle of static files from a baked-in origin, after first pixel, into an on-disk
  store, for its next launch (LLP 1030 for every layer; 1030.000 for the verb). Unblocks
  deploying a change to installed apps from a bake and an upload. Take: `EXACT_DEV_PLAN`'s
  file poll and the two dev-only URL loaders collapse into that one store with two
  policies (1026 D12) — fewer paths after than before. What stays refused, as
  the *service* half of the old economy: per-user targeting, cohorts by identity, an
  update console, server-side anything, push delivery.
- Hot revision surfaces, staged reload. (HBC compilation came off 2026-09-03 for the
  *bake only* — LLP 1027 D5: `hermesc` runs at build beside the Contract compiler;
  nothing compiles at runtime, and the lean VM a host links cannot. A dev reload *carries
  state* — slots by name where their types still fit, settled resources where
  their arguments still match, the clock — and is otherwise a restart: no patch
  format, no generations, no identity matching. LLP 1007 §6; Charlie, 2026-08-28.)
- Islands and inline islands.
- Cross-runtime shared data in production. **Measured exception (Charlie,
  2026-09-18, LLP 1027.003 §9):** all four isolated transfer/storage directions
  were tried. The small conversion and app fixes are selected for delivery;
  immutable views, typed buffers and a mutable shared heap remain research.
  **Priority (Charlie, 2026-09-18, LLP 1027.004):** bounded Messages answers come
  next. Unblocks keeping producer, transfer and settlement work proportional to
  the requested window. Take: further shared-representation tuning and generic
  arrival-time structural reconciliation move behind this consumer. No new
  benchmark app, scheduler, core feature matrix or second application-state graph.
  Production ABI changes, arbitrary object unification and a default shared heap
  remain unselected.

**Components** — roughly 15 built-in tags, not 40; roughly 12 Facet components, not 47.

- No `lottie`, `rive`, `fileinput`, `pager`. Video admitted 2026-09-18
  (Charlie: design video and build a keyboard-resizing player; LLP 1042). Unblocks
  the native/web video consumer; router/viewport-fact integration (1038/1039)
  follows this player. (`webview` came off
  2026-08-30 as LLP 1020's `iframe` — it unblocks Weird Castle's entire content
  model, the client that exists to surface Castle web decks; the take: LLP 1013
  view transitions moved behind the deck lane. LLP 1020 §6. What stays no from
  exact1's webview: `top` topology, navigation policy, the controller ops, `allow`,
  author-facing `srcdoc` — each with its return trigger in LLP 1020 §5.)
- No camera anything.
- No gradient style rows. A gradient with anything on it is a canvas surface with
  children (LLP 1014 §5 — the take for widening `canvas`; the three rows return when a
  host earns them).
- No virtualList v2 (cert wires, extent demand, proxy lanes). **Admitted 2026-09-14
  (Charlie: "ok do what you think"):** a straightforward windowed list with bounded
  row/view lifetime and a separate decoded-image budget (LLP 1010 §6). Unblocks
  real Messages histories without constructing every row; memory and construction
  cost trigger it alongside 60 fps. Take: further Messages decorative Tapback/emoji
  artwork, material matching and animation-timing polish move behind list memory.
  O(N) input data is named separately from O(window) UI; recycling is no flat-memory claim.

**Motion** — **in v1**, in the LLP 1002 shape: CSS's `transition` model. Targets
are kernel style rows (`translate`, `scale`, `rotate`, `opacity`); a `transition`
row on the node says how they get there; the web host emits it as CSS and does
nothing per frame; every other host runs `exact-motion`, which is held to the
browser by fixtures. One declared deviation, `spring()`, lowered to keyframes on
the web. Gestures are in as follow-and-release: the platform recognizes, the
engine holds a value and springs it back with the release velocity. The seekable
clock is in, and it is the reason motion is testable: an agent advances time to
`settle_time()` and reads; it never waits.

Moved off this list 2026-08-28 (LLP 1002 D6): **delegation to CSS on the web** — it
unblocks a web host that ships zero motion bytes and a parity corpus with the
browser as the oracle, the same shape layout already has. In exchange, not in v1:

- **A gesture arena, claims, leases, compositions, or an interactive-navigation
  model.** Recognition, hit-testing, and scroll-vs-pan arbitration are the
  platform's (`touch-action`, `UIGestureRecognizer`); owning them is the
  permanent bug annuity LLP 0559 F1 describes. Scroll always wins.
- **A second value graph.** No shared-value plane, derived values, bindings, or
  plan node graph. The style row is the binding.
- **General layout transitions.** LLP 1041 §8.12's measured projection admits
  one explicitly registered numeric-height sheet (Charlie’s four-interaction
  campaign, 2026-09-17). Take: other animated layout properties and decorative
  effects wait behind that sheet and its nested collection. This is not a
  general layout-animation system or evidence of physical 120 Hz.
- **Decay, sequence, and repeat drivers; `@keyframes`.** A spring carries release
  velocity; nothing else needs a driver.
- **Reduced-motion policy in the engine.** The producer emits `transition: none`
  when the host reports the preference, as a stylesheet's media query would.
- **A Core Animation executor.** Permitted by LLP 1002 D2, not built; the Apple
  lane measures whether it earns its place.
- **`runOnJS` and the escape hatch / runtime graph admission** — never existed here.

**Tooling** — no Design Mode, no Guide system, no devtools UI, no TUI host, no blog/CMS.

**Agent API** — 8 operations, not 90:

`tree` · `screenshot` · `tap` · `type` · `state` · `layout` · `logs` · `clock`

**A ninth operation replaces one of the eight, same PR.** A new input is a form of
`tap` or `type` (a wheel is a `tap`; so would a drag be); a new question is answered
from `tree`, `state`, or `layout`. The old repo's six primitives grew eighty wire names
one reasonable "view" at a time; the count is the cost. (Charlie, 2026-08-29.)

Not shipping: session record/replay, causal trace, behavior diff and verify, mutation
dry-run, contract witness / dataflow / ~~source-map~~ / bindings, accessibility audit, plan
drag, correlate, visual query, network, perf, preferences, pasteboard, onboarding,
revalidate, code grant/resume/cancel.

Admitted 2026-09-10 (Charlie, LLP 1035 §5; 1035.002 D6, 1035.005 D3): **a
development-only map from plan node to its declaration and component call-site
chain**, emitted by the compiler beside the plan, keyed by the plan's digest, read by
the driver and never by a host, never in a bake. It unblocks jumping from a failing
node to its source. The take: no new Contract syntax until formatting and navigation
have landed (1035.005 §1's order), and `contract` blocks as executable assertions
(LLP 1006 §8) stay off the doing-list. The witness, dataflow and any general
provenance explorer stay refused.

`clock` replaces a `wait` operation on purpose. If the motion graph is closed-form under
a virtual clock, an agent advances time and reads the result; it never sleeps waiting for
an animation to settle. Settle timing is the single largest source of flake in the old
repo's agent loop, and under `RULES.md` a flaky check is worse than no check.

## Process

This half matters more than the feature half.

- **No governance corpus.** 20 docs, capped. No lane manifest, no verify registry with
  675 entries across 64 profiles, no contracts authority map, no issue priority scoring.
- **No speculative specs.** Specifying something you are about to implement is fine and
  expected — the deciding already happened upstream. Specifying something nobody is
  assigned to build is not written at all. The old corpus is 330 RFCs of deliberation
  against 52 specs of conclusion; the new repo imports conclusions.
- **No refine loops.** No dual-family review rounds, no READY verdicts, no revision ledgers.
- **No per-PR ceremony.** No framework decision log, no lane debt ledger, no provenance
  headers on hand-written files.
- **No blocking check the author did not run locally in under a minute.**
- **No 90-minute gate.** If verification takes 90 minutes it runs behind you, or it does
  not run.

## Deliberately worse

- No backwards compatibility, at all, before 1.0.
- No public API stability. Names change.
- No migration guides.
- Generated files are built, never committed.
- Sparse prose. The code and the checks are the authority.

## Moving something off this list

Write one line naming what it unblocks, and take something off the doing-list in the
same PR. If nothing can come off, the answer is no.

**Install-page slice, 2026-09-12 (Charlie, LLP 1030.003 D6a/D6b):** bake a
standard `/.exact/install/` page for web/iOS/macOS with configured installation
methods, and let a Mac development server invoke its existing simulator or device
build for a local Simulator or paired phone. Unblocks sharing one honest
installation URL and a local Apple development loop; the generic Go launcher,
distributable IPA, EAS/AppDrop adapter and
automatic provider provisioning remain behind proving this consumer. No new
update service or production native carrier is introduced.
