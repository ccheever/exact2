# LLP 1024: Native modules — a tag that is not a host change

**Type:** RFC
**Status:** Accepted. Charlie Cheever approved implementing it on 2026-09-26, which ratifies §6 Q1 (one app-scoped module artifact with an inner tag → factory table). D8 landed 2026-09-27; §9 is what was built.
**Systems:** Kernel (`NativeView` already in the schema — no new node type), Contract (hyphenated tags; leftover attrs as one literal JSON aggregate), Apple host (one NativeView arm; one app module artifact behind `dlopen`), Web host (the real custom-element tag; one injected sibling module), Linux host (unavailable in v1), Agent API (the eight operations; no ninth), GPU / iframe (stay first-party HTML tags)
**Author:** Grok 4.6 for Charlie Cheever. r2 folded by Claude Fable from the three-model panel of 2026-08-31 (§8); the fold is an edit for Charlie to accept, not an approval.
**Date:** 2026-08-31 (r1 and r2)
**Revised:** 2026-08-31 (r2 folded from the three-model panel; no status change). 2026-09-27 (§9, as built: implementer Claude (Opus 5.5); consumer the photo editor, `apps/photo-editor`.)
**Related:** `AGENTS.md` optional-capability rule (a separate artifact or another executor, never a cargo feature on a core crate); LLP 1000 (the crate graph; “an embedder links the crate they want”); LLP 1001 (`NativeView` is a kernel-owned box; the declared-deviations list); LLP 1007 §6 (a plan reload carries slots, matching resources, the clock, the store — not the tree, focus, or springs) and §8 (nothing imports before first pixel; `boot.mjs` counts); LLP 1008 (Apple host; the one-thread C ABI; the inode-replace rule in `host/apple/build.mjs`); LLP 1009 D2/D4 (the GPU module: one app artifact, an inner name → factory table, loaded after first pixel); LLP 1012 (the eight operations); LLP 1014 (canvas children — not this); LLP 1017 §8.1 and P1 (the words are the web’s; no quiet failures; the bake lint P1d); LLP 1020 D1/D3 (iframe is one HTML tag, not module dispatch; the on-demand dylib ABI this copies — including its reply callback); LLP 1023 (plans on the LAN; native code is never a network payload; a plan’s bytes are network bytes). Predecessor, research never authority: exact1 LLP 0149 (`NativeView` vs `NodeType`; `Terminal` is listed as a NativeView module) and LLP 0525 (ModuleIR — the generated-every-binding machine this RFC refuses).

## 1. Summary

A non-HTML widget — a terminal, a map that is not a `canvas` surface, anything
that is a platform view with its own megabytes — is a **hyphenated tag** in
Contract, a **`NativeView`** in the kernel, and a factory in the app’s **one
module artifact** at runtime. The host has one arm. After the seam lands, the
*second* widget edits nothing but the app’s own module crate — not
`schema.json`, not `tags.rs`, not `Presenter.swift`. (The first landing does
edit all the usual places once; §7 is honest about that.)

The tag is HTML’s extension point: a custom element name (lowercase, contains
a hyphen). On the web the node *is* that custom element, upgraded by a module
the page loads after first paint. On Apple the presenter `dlopen`s the app’s
single module dylib — the way iframe already asks `libexact_web.dylib` for a
WKWebView — and asks its inner tag → factory table for a platform view. No
module crate in the app, no file in the bundle, no bytes in the 2.3 MB host.
A plan that never names a module tag never loads the artifact.

`canvas` and `iframe` stay first-party tags: they are HTML. This RFC is for
everything that is not.

The app’s module roster — the tag → factory table its module crate’s build
emits — is the declaration, the analogue of `customElements.define`. A
hyphenated tag not in the roster is a **bake error**, so a typo is a named
diagnostic in the dev loop, never a silent empty box. The runtime empty box
is reserved for “declared, but unloadable on this host” (Linux in v1, a
packaging miss, a failed load), reported per node.

Iteration is restart-shaped, like every sibling artifact today: a module
rebuild is a new inode and a page/process reload. In-process live-swap is a
named cut, not a v1 mechanism. iOS device is the signed bundle at `codesign`
— a frozen roster, not a live registry.

This is a proposal. It has no implementer and is not a spec. It lands only
when a consumer and an implementer are named; the v1 bar (Caltrain, Weird
Castle’s wordmark) does not move for it. The landing is the seam plus a
fixture module a smoke can drive; Ghostty is an app crate that would consume
the seam, not the thing that proves it.

## 2. Motivation

Three facts from the last two days, one missing cut.

**The host is 2.3 MB because it is one program.** The Caltrain iOS `.app`
is 5.5 MB on a phone: 2.3 MB ExactIOS (kernel, runner, Taffy, ibex2, the
UIKit presenter, the baked plan), 3.0 MB `libexact_gpu.dylib`, 0.15 MB
`libexact_web.dylib`. The 30 MB `libcaltrain_apple.a` is the unlinked
archive; after strip and thin LTO only 2.3 MB is in the binary. GPU and
iframe are already optional artifacts. An embedder with a strict download
budget can omit them. They cannot omit the next widget unless it is the
same kind of artifact — and today it is not: a new *kind* still punches
`tags.rs` and a `kind ==` arm in both presenters.

**The kernel already has the box.** `NativeView` (schema id 9) carries
`nativeViewModuleName` and `nativeViewProps`. It is a leaf, never measured,
sized by its style rows. Nothing in `exact-kernel` needs to know Ghostty
exists. LLP 0149 said this in the predecessor (`Terminal` → NativeView).
exact2 then special-cased `Canvas` and `WebView` because those names *are*
the web. The leftover type was never given a Contract tag or a presenter
arm. LLP 1020 D1 is explicit: iframe “is not module dispatch, because
there are no modules, there is one tag.”

**The whole shape already shipped, for two other things.** GPU is **one app
artifact with an inner name → factory table** (`apps/caltrain/gpu`’s
`REGISTRY`; adding `surface=aurora` did not add a node type), copied by
`build.mjs` only when the crate exists, loaded after first pixel. Iframe is
a C ABI (`exact_web_create` with event *and reply* callbacks /
`platform_view` / `set_src` / `snapshot` / `destroy`) behind `dlopen` at
the first create commit. A native module is GPU’s packaging and registry
glued to iframe’s ABI, pointed at `NativeView`.

Without this RFC the next terminal is `NodeType::Terminal` and a first-party
host change, which is how a 15-tag kernel becomes 40. With it, the one
host change is the arm; every later widget is an app artifact.

## 3. What this is not

- **Not a cargo feature** on `exact-kernel`, `exact-runner`, or
  `exact-apple`. Optional capability is a file that is present or not
  (`AGENTS.md`). A feature flag is a build matrix.
- **Not ModuleIR.** exact1 generated Contract, Swift, and a fake from one
  authority (0525). Drift tickets followed. This RFC’s authority is the
  tag name, the kernel box, and a small C ABI. Props the compiler does not
  already know are a JSON object on `nativeViewProps`. There is no `module`
  declaration in Contract grammar either — the roster is packaging, not
  language.
- **Not a plugin directory.** No `$EXACT_MODULES` search path, no
  first-hit-wins, no per-tag files. The module name is a table key and is
  never concatenated into a filesystem path (§4 D5) — a plan’s bytes are
  network bytes (LLP 1023), and 1023’s Stage-1 incident was a foreign plan
  driving a local binary.
- **Not a ninth agent operation.** The guest (iframe) already joins the
  eight (LLP 1020 D4). A native module does the same through the ABI’s
  events and optional snapshot. Agent `type` *into* a module is a named cut
  (§5), not an implication.
- **Not a new HTML tag.** There is no `<terminal>` in HTML. Custom elements
  are how the web adds those. `ghostty-terminal` is a legal custom-element
  name; `terminal` is not, and it stays `lower-unknown-tag`.
- **Not a rewrite of GPU or iframe.** `canvas` / `iframe` remain the
  first-party tags they are. A wgpu surface is still `canvas surface=name(…)`.
  A WKWebView is still `iframe`. Migrating either onto NativeView is a
  later take with no consumer.

## 4. Design

### D1 — A hyphenated unknown tag is a custom element; it lowers to `NativeView`

The lexer already accepts hyphenated idents (`font-size`, `aria-label`;
`contract/syntax/src/lexer.rs`, LLP 1017 §8.1). `ghostty-terminal` is one
token today. Lowering is the closed door: `tags::tag()` is a match, and
an unknown name is `lower-unknown-tag` (`contract/lower/src/lib.rs`).

**Admission is HTML’s potential-custom-element-name, not “contains a
hyphen.”** The conservative subset:

```
[a-z][a-z0-9_]*(-[a-z][a-z0-9_]*)+
```

Lowercase start, at least one hyphen, lowercase throughout, and the
SVG/MathML reserved names refused (`font-face`, `font-face-src`,
`font-face-uri`, `font-face-format`, `font-face-name`, `color-profile`,
`annotation-xml`, `missing-glyph`). Uppercase is refused, not normalized —
the lexer accepts `Ghostty-Terminal` today (`lexer.rs:136`,
`is_ascii_alphabetic`), and it would compile here and then fail
`customElements.define` on the web host, which is the oracle. Source
spelling, DOM tag, and table key are the identical string. Unhyphenated
unknown tags stay `lower-unknown-tag`; `texxt` does not become a native
view. HTML’s grammar is the admission test, not an Exact invention.

A tag that passes lowers to:

- node type `NativeView` (a leaf, never measured; `can_hold_children` is
  false — `kernel/src/node.rs:10-20` — so children are refused by the
  existing `lower-leaf-children`)
- `nativeViewModuleName` = the tag name
- **a fixed `display: block` row.** Not because CSS says so — it does not.
  A custom element, defined or not, has **no UA style rule and is
  `display: inline`**; the kernel has no `inline`, and the web host emits
  `display` only when the row is set, so making the DOM element the real
  tag (D2) without this row would render a bare module tag inline in the
  browser and block on native — the cross-host default-divergence class
  this repo exists to kill. The tag therefore *declares* block the way
  `column` declares its direction (`tags.rs:65-71`): a plan-visible tag
  row, not a kernel deviation, and the parity fixture compares against
  HTML that also says `display:block`. Fixed rows are applied before
  authored attributes and the last binding per row wins
  (`contract/lower/src/lib.rs:800-812`), so an authored `display` still
  overrides.
- no 300×150 default (that is the replaced-element default `canvas` and
  `iframe` take from HTML; a NativeView is not measured, so an author who
  wants a box sets `width` / `height`)
- known attributes still bind first: style rows, the existing handler set,
  `testId`, ARIA — the whole `tags::attr()` table. **`renamed()` spellings
  are still refused** with their CSS names (`tags.rs:296-351`): `fontSize`
  does not become a module prop, exactly where authors write the most
  unfamiliar markup.
- leftover attributes become **one literals-only JSON aggregate** in
  `nativeViewProps`: values are string / number / bool literals, carried
  as strings (the hosts’ existing string projection), keys canonically
  sorted, the whole object **replaced** on change (never patched), a
  cleared attribute absent, deterministic escaping. A computed leftover
  (`cwd=stationId`) is a named cut (§5) — the aggregate is not live-bound
  in v1, and lowering many attrs into one string prop needs a small
  plan/runner helper the landing budgets for.
- `load` and `message` unlock on module tags (they are how a module says
  “I am ready” and “here is a string,” exactly as iframe’s). `src`,
  `sandbox`, and `surface` stay refused on them.

**The typo net is an error, not a warning.** The app’s module crate emits
its tag roster at build (the GPU move — LLP 1009 D2’s registry, made
readable); `contract::bake`, which already boots the runner and lints the
first frame (LLP 1017 P1d), refuses a module tag not in the roster as
`bake-unknown-module`, naming the tag and the nearest roster entry.
`BakeError` already fails every host’s `build.rs`, so the diagnostic lands
in the ~20 ms dev loop and in every build. What the author sees:

1. **Typo’d tag** (`ghosty-terminal`): `bake-unknown-module`, with the
   roster and a nearest-name suggestion. No plan is produced.
2. **In the roster, artifact missing at packaging**: `build.mjs` fails the
   release build, named. A dev-runtime miss (deleted file, AMFI, a failed
   load): the box stays, `tree` reports the module status, `logs` is loud.
3. **Platform-unsupported** (Linux v1): bake succeeds; the box is empty
   with `state: "unavailable"` and the host named. Iframe-on-Linux’s
   standing (LLP 1020 D5).

### D2 — The host has one NativeView arm; the module name is a table key

Presenter `create` grows one arm, next to `canvas` / `iframe` / `input`:

```
kind == "native" → modules.attach(owner, name)
```

`modules.attach` re-validates `name` against D1’s grammar (a **key check on
plan bytes** — a doctored plan that skipped bake must die here, with both
the reason and the name in the log), looks it up in the loaded artifact’s
tag → factory table, calls `create`, and adds the returned platform view as
a subview of the node’s box. Prop patches that are not style rows go to the
module as the replaced JSON aggregate. The existing event kinds enter the
runner the way iframe’s `load` / `message` already do.

No factory → the box stays, empty, and `tree` reports a status object:

```json
"module": { "name": "ghostty-terminal", "state": "loading|ready|unavailable|error", "error": "…" }
```

per node, never a boot refusal, always loud in `logs`.

The Apple host’s `kind_for(NativeView)` already emits `"native"`
(`host/apple/src/host.rs:646`). The web host currently maps `NativeView` to
`"div"` (`host/web/src/host.rs:538`); it becomes the tag name itself, so
the DOM’s custom element is the oracle — safe only together with D1’s
fixed `display: block` row.

### D3 — One app module artifact, loaded after the paint gate, never on the boot path

**The shape is GPU’s, exactly** (LLP 1009 D2/D4): one app-scoped artifact
per platform with an inner tag → factory table, present only when the app
has modules, loaded on first use after first pixel.

**Native (Apple).** The app’s `modules/` crate (beside `gpu/`) builds one
dylib, copied by `build.mjs` under one stable load name
(`libexact_modules.dylib` — the `libexact_gpu.dylib` move,
`host/apple/build.mjs:184`) only when `modules/Cargo.toml` exists — the GPU
gate (`build.mjs:178`), not iframe’s always-copy. Loaded with
`dlopen(path, RTLD_NOW | RTLD_LOCAL)` at the first module-node create
**after the paint gate**. The host crate still `#![deny(unsafe_code)]`; the
artifact owns the `unsafe` boundary, as GPU and WebArm do.

**Web.** One app-relative JS module (the executor of the same table, JS
exports for the C names), loaded by an **injected `<script type="module">`
after the paint stamp — never an `import()`**: `boot.mjs` counts the
pre-pixel module graph and flags dynamic imports (`scripts/boot.mjs:44`);
the GPU glue records the rule in place (`host/web/glue.js:674-676`). It
defines the custom elements for the roster’s tags.

**The gate is explicit, because the obvious implementation is wrong.** A
module node in the first frame arrives while the first batch is applying —
loading there is code before first pixel. Creates during the initial batch
make the empty box and queue; loading starts at the established post-paint
gate (Apple: the next turn after first draw, the GPU precedent at
`Presenter.swift:573-580`; web: the injected script from the frame after
the paint stamp). A node created after first pixel attaches immediately.
“First use” is this paragraph, not a phrase.

**iOS device / App Store.** The artifact is in the signed `.app` at
`codesign` (`Frameworks/libexact_modules.dylib`, signed with the app —
the existing embedded-dylib path in `build.mjs`). AMFI will not run a
native slice written after install; Store 2.5.2 is the same fact as
policy. iOS is a **frozen roster**, not a live registry. Simulator is not
the oracle for this.

**The LAN never delivers native code.** LLP 1023: the network carries
plans. The module artifact is a local file, like today’s GPU dylib. A URL
that names a dylib is a refusal. The web module is a page resource in the
app’s own fetch graph, like GPU wasm — never an envelope payload.

### D4 — The ABI is one versioned function table; the callbacks are iframe’s, both of them

One exported entry:

```
exact_native_abi() → const ExactNativeAbi*
```

returning a **size-versioned table**: ABI major version, struct size, the
tag roster, per-tag capability bits, and function pointers. Iframe may live
as six independently `dlsym`’d unversioned symbols because it is built in
the same `build.mjs` run as the host; a module artifact is the opposite by
design — an app-side artifact iterated against a host binary its author
did not build — so version skew is checked once at load, and a mismatch is
per-node `unavailable` with both numbers logged, never a crash inside
`create`.

| Entry | Job |
|---|---|
| `create(tag, props_json, event_fn, reply_fn, ctx) → handle` | Construct. **Both** callbacks, as iframe’s create takes them (`WebArm.swift:365-369`) — `reply_fn` is how a snapshot answers. Typed success/error. |
| `platform_view(handle) → view*` | The `NSView` / `UIView` the host inserts; the module retains ownership. |
| `set_props(handle, props_json)` | Replace the whole aggregate. A rejected replacement leaves the last accepted one active. Style rows never enter. |
| `snapshot(handle, token)` | **Optional, per-tag capability bit.** Tokened, answered on `reply_fn` with iframe’s kinds (0 = PNG bytes, 2 = error text; `WebArm.swift:349-358`). Without the bit, the host’s ordinary platform-tree capture is authoritative — a plain `NSView` composites in `cacheDisplay`; the bit exists for Metal- and remote-layer-backed views, which are blank there (the canvas readback and `Capture.web` precedents). |
| `destroy(handle)` | Last call; invalidates the instance. |
| *(reserved, nullable)* `set_bounds`, `agent_input` | Named cuts (§5). Nullable slots so their return never breaks the table. |

**Events.** The full existing handler set, pinned: empty payload —
`press`, `focus`, `blur`, `submit`, `load`; boolean — `hover`; UTF-8
string — `change`, `key`, `message`. Integer discriminants are the
kernel’s `EventKind` values the Apple host already maps
(`host/apple/src/host.rs:544-553`). (r1’s table omitted `press`, `hover`,
`submit` while D1 admitted them; that contradiction is closed.)

**Threading and reentrancy, written down.** Every table entry is called on
the platform’s UI thread (LLP 1008 §4’s one-thread rule). `event_fn` /
`reply_fn` may fire from any thread — a PTY’s reads will — and carry a
host-issued instance nonce: the host copies the bytes, hops to the
presenter thread, and enters the runner through the same gate as iframe
`load`/`message` (the `applying`/`waiting` queue,
`Presenter.swift:769-809`); it never re-enters `apply` synchronously, and
callbacks bearing an invalidated nonce are dropped. `destroy` invalidates
the nonce before module teardown. No unwind crosses the C boundary; input
buffers are borrowed for the call, callback bytes are borrowed for the
callback and copied immediately.

**Bounds are observed, not pushed.** The host sizes `NodeView`; the
platform view fills it (`autoresizingMask`, and the frame op already
writes the child on every layout — `Presenter.swift:290-295, 852-859`,
iframe’s contract, which has no `set_frame`); the module observes its
view’s bounds and backing scale (`backingScaleFactor` /
`devicePixelRatio`). One size channel. A terminal’s cols × rows is that
observation times its font — not an ABI hole. `set_bounds` returns with a
module target that has no platform view, or a measured observation gap.

No eval channel (iframe’s `exact_web_agent_eval` is a guest-DOM bridge; a
native module has no guest DOM). If a module needs inspectability beyond
`tree` / `screenshot`, it puts it in `message` or in the snapshot.

The web executor is the same names as JS exports, not a C ABI.

### D5 — Lookup is a table, and the name is never a path

The registry is the loaded artifact’s tag → factory table. Resolution:

- **Release**: the signed bundle’s one artifact. Nothing else. A missing
  artifact for a non-empty roster already failed the build (D1).
- **Dev-capable hosts only**: one explicit override — `EXACT_MODULES`
  names the app’s module artifact *file* (not a directory, no search
  order), the same standing as `EXACT_DEV_PLAN`. Release binaries do not
  read it, the LLP 1023 dev-capable split.
- **Web**: the app-relative module script, same-origin, in the page’s own
  fetch graph.

**`nativeViewModuleName` is never concatenated into a filesystem path.**
It is validated (D1 grammar), then used as an in-memory key. A plan is a
network payload (LLP 1023) and its Stage-1 incident — a foreign plan
booting a local binary — is the standing demonstration that plan bytes
reach further than authors intend. Table miss → the D2 status object.

**Generationed write, never in place — kept, for the one inode.**
`host/apple/build.mjs:227-228` records why: rewriting a mapped dylib’s
inode poisons the kernel’s code-signature cache and the next `dlopen` is
`SIGKILL (Code Signature Invalid)`. Rebuilds write a fresh inode and move
it into place; the *next process* loads it. Production iOS has one
generation, signed with the app.

**Do not `dlclose`. Ever, in v1.** Not for Swift/ObjC (the runtime keeps
the classes), and no “pure C/Rust exception” either — nothing in v1
unloads, because nothing in v1 swaps (D6). The artifact stays mapped until
process exit.

### D6 — Introduce is a plan reload; iterate is a restart; live-swap is cut

**Introduce a tag the running app has never seen.** The author adds the
factory to the app’s module crate and writes
`ghostty-terminal width="100%" height=240` in Contract. The resident
compiler emits a `NativeView` (roster check at bake). `dev.mjs` pushes the
plan. The host `exact_boot_plan`s with `Runner::carry` (LLP 1007 §6):
slots by name, matching resources, clock, store. The tree is torn down and
rebuilt; scroll, focus, springs, view ids do not survive — already the
rule, and this RFC does not reopen identity matching. The first create
after the paint gate loads the artifact. Selected station, login, clock
stay. The terminal is new, which is correct.

**Iterate module code.** Rebuild the one artifact onto a fresh inode,
restart the page/process. That is what GPU and iframe do today — a
GPU-crate edit reloads the page; a native Rust edit is `build.mjs --run` —
and the panel cut in-process replace from the landing (§5): the sibling
precedents are restart-shaped, the D7 pointer registry existed only to
serve the swap, and without an instance snapshot a PTY dies either way, so
live-swap bought a nicer seconds-loop, not the seam. Plan-only edits still
reload in ~20 ms and do not relink.

**Module-instance state does not survive unless the module says so.** A
canvas surface is a function of plan inputs: destroy and `bind` again is a
refresh (LLP 1009: the device outlives plan reloads; surfaces do not). A
terminal’s PTY is not a function of props. v1 has no instance snapshot.
Keeping a PTY across a restart is a later ABI (or a sidecar process the
host owns). It is not a reason to delay the seam.

**iOS device** has no replace-without-reinstall and no
introduce-without-reinstall. The roster is the bundle.

### D7 — On the web, the node is the custom element, defined once

The web batch’s `op.tag` is the module tag; `glue.js` creates the real
element. `customElements.define` runs once per tag, at the roster’s
definition when the module script loads; wasm-bindgen `init` stays a
singleton. The plan’s `display: block` row arrives in `op.css`, which is
assigned **before** the element is inserted (`glue.js:230-247`), so there
is no pre-upgrade inline flash and no second styling authority — the plan
row is the one authority, and the browser parity fixture uses the same
explicit `display:block`.

A plan reload explicitly unmounts module instances before the host clears
the DOM (`views.clear()` / `root.replaceChildren()` — the GPU reset
precedent at `glue.js:641-652`), and the defined elements are reused by
the next plan’s creates. A module-*code* edit reloads the page, like a GPU
wasm edit (D6). The r1 pointer-registry-and-remount design existed only
for live-swap and is cut with it.

The web *executor* of a given module is that module’s problem: xterm.js,
a wasm, a canvas. The representation is the tag and its props. Parity
with native is the module author’s, not the kernel’s — the kernel sees a
box. A fixture module that paints a solid color on every host is what
the smoke holds; Ghostty-on-Metal vs xterm is not a kernel fixture.

### D8 — The first landing is the seam and a fixture, not Ghostty

**Precondition:** a named consumer and a named implementer, and the v1 bar
does not move. Fixture-first is the order of work, not the authorization
for it (`rules/NOT-DOING.md` §The bar; LLP 1001 left a module registry out
of the kernel “until a consumer exists”).

1. Compiler D1: the PCEN grammar, the fixed `display:block` row, leftover
   attrs as the literal aggregate (with its plan/runner helper),
   `load`/`message` unlocked, `bake-unknown-module` against the emitted
   roster.
2. Presenters D2–D5 on macOS and web. Linux: `{unavailable}` status, no
   loader. iOS: bundle copy through the real signing path, no live add.
   No in-process swap.
3. A **fixture module** in-repo (not an app): one hyphenated tag, a
   colored box, hit-test transparent so agent `tap` lands on `NodeView`
   (LLP 1012’s `tap` is a real window event at the box center — an opaque
   module view would eat it), the snapshot bit **set** so the tokened
   path is exercised. `node scripts/smoke.mjs` drives web and macOS the
   way it drives iframe’s deck, and asserts: `tree` shows the module
   status object through `loading → ready`; props replace (multiple
   reactive keys, clearing, ordering, escaping); all nine events with
   their payloads; a background-thread callback after `destroy` is
   dropped (the nonce); loading starts only after first pixel; a plan
   reload does not re-define the element; ordinary capture shows the box
   and the tokened snapshot returns its color; the failure family —
   missing artifact, missing factory, wrong ABI version, rejected props —
   each yields its named status, empty box, log line, and a running app.
4. `build.mjs` copies the artifact only when the app has one (the GPU
   gate), fails a release build whose roster names what the bundle lacks,
   and signs it through the existing embedded-dylib path so the iOS
   simulator run traverses `codesign` for real. Iframe’s always-copy is
   not the pattern to extend.

Ghostty, maps that are not wgpu, anything with a PTY or a unique renderer,
is an app crate that implements D4. It is out of this RFC’s landing.
Caltrain and Weird Castle’s v1 bar do not depend on it.

## 5. Cuts (and what earns them back)

| Cut | Why | Return trigger |
|---|---|---|
| `NodeType::Terminal` (and friends) | Protocol ABI; 0149’s rule; the whole point | The kernel must measure it, scroll it, or otherwise understand it before the module runs |
| ModuleIR / generated fakes / a `module` Contract declaration | exact1 0525; four-authority drift; the roster is packaging, not grammar | A second consumer that cannot keep props in a JSON object |
| Cargo features on core crates | Build matrix, `AGENTS.md` | Never; a missing module is a build error (roster) or an empty box (host) |
| **In-process live-swap / HMR** | GPU and iframe are restart-shaped; the PTY dies anyway; D7’s pointer registry served only this | A consumer iterating a renderer, with a measured cost of `exact_boot_plan` / restart it cannot pay |
| **Per-tag artifacts, `$EXACT_MODULES` directory, search order** | One app artifact is the GPU precedent; a table key never touches the filesystem | Measured relink of two large unrelated modules breaching the 30 s edit budget (`rules/RULES.md`), or material co-loading cost |
| **Computed leftover attrs** | One literal aggregate; no live-bound JSON stringify in v1 | A module prop that is a function of state (a terminal’s `cwd`) |
| **`set_bounds`** | Bounds observation is the iframe contract; one size channel | A module target with no platform view, or a measured observation gap |
| **Agent `type` / guest-style `tap` into a module (`agent_input`)** | LLP 1012 refuses `type` on non-inputs; iframe’s guest bridge was its own special case | A module that is itself an editor; the nullable slot is reserved |
| **Linux loading** | The host links no system library (LLP 1000) and has no platform view; `{unavailable}` is iframe’s standing | A Linux app that ships a native module and names its view/pixel target |
| Migrating `canvas` / `iframe` onto NativeView | They are HTML; they already have arms | A world with no first-party tags left, not this RFC |
| Children / light DOM | `can_hold_children` is false; iframe is a leaf | A module that is a container the kernel must lay out |
| Intrinsic-size callback | Image already has the kernel seam | A module whose natural size should drive layout |
| Instance snapshot (PTY survival) | Live process state is not `Carried` | A consumer that iterates a renderer without dropping a session |
| `dlclose` | Swift/ObjC cannot; and v1 never unloads — **no C/Rust exception** | Nothing in v1; revisit only with live-swap |
| Live add / swap on iOS device | AMFI, store 2.5.2, signed bundle | Apple ships a supported in-app native plugin API |
| Native code on the LAN | LLP 1023; iOS will not load it | Never for dylibs; the web module is page fetch graph, not envelope |
| Unhyphenated or uppercase open tags; reserved SVG/MathML names | `texxt` and `Ghostty-Terminal` would compile; the web is the oracle | Never |
| Identity-matching the tree on reload | NOT-DOING §Runtime; LLP 1007 §6 | A measured need to keep focus/scroll across a *plan* reload, separate RFC |
| Ninth agent op | LLP 1012; iframe already joins as eight | A question `tree` / `state` / `screenshot` cannot answer |
| Eval into the module | Iframe needs it for a guest DOM; a native view does not | A module that is itself a document |
| Module-rebuild ping (`fs.watch` / SSE) | Restart-shaped iteration needs none | Live-swap’s return; then a typed module-generation event on the existing SSE, never a code URL |

## 6. Open questions — resolved by the panel, awaiting Charlie

r1 asked four; the panel (§8) answered all four unanimously. They are
recorded here as leanings for Charlie to ratify, not decisions:

1. **One dylib per tag, or one artifact with an inner table?** **One
   app-scoped artifact with an inner tag → factory table** — the GPU
   precedent as actually built. Per-tag was r1’s recommendation and the
   author’s own round-1 position; both reversed once live-swap was cut
   (the iterate-independently argument served the swap) and the
   name-never-a-path property fell out. `create` keeps its `tag` argument
   so a later per-tag split never changes the C surface. Split trigger in
   §5.
2. **HTML-name collision.** Closed as a non-question: hyphenated names are
   the web’s author space. The reserved-name list in D1 is the one patch.
3. **Linux fixture.** **Unavailable in v1** — r1’s load-and-composite
   recommendation is withdrawn (the author withdrew it itself in round 2);
   the host links no system library and has no view tree to host a module.
4. **Dev-server ping.** **None in v1** — restart-shaped iteration needs no
   watcher. The SSE shape is named in §5’s cut row for live-swap’s return.

Still genuinely open, and Charlie’s: ratifying Q1’s shape, and naming the
consumer + implementer that turn D8 from a proposal into work.

**Ratified (Charlie Cheever, 2026-09-26).** His go-ahead to implement this
LLP ratifies Q1: one app-scoped module artifact with an inner tag → factory
table. The implementer is Claude (Opus 5.5); the consumer is a photo-editor
example app whose main surface is a module tag (§9).

## 7. Trade

`rules/NOT-DOING.md` §Components does not lose a line. This RFC does not
add `terminal` as a built-in tag; it adds a seam so a *non*-built-in does
not become one. The v1 bar (Caltrain, Weird Castle’s wordmark) does not
depend on the landing, and the landing does not start until a consumer and
an implementer are named.

What the landing adds, once: one compiler path, one presenter arm per
host, one ABI, one fixture, a packaging gate — the same class of apparatus
GPU and iframe already are, generalized so the third widget does not add a
fourth arm. The summary’s claim is therefore about widget **N+1**: the
*first* landing edits `tags.rs`, both presenters, `build.mjs`, and the
glue, exactly once.

Where the complexity actually lands, honestly: on the module author. A
real `ghostty-terminal` owns the C table, the platform view, bounds and
first-responder behavior, a snapshot for its Metal layers, packaging under
`modules/`, and a web executor if it wants web parity. That is real work;
it is not host work, and that split is the point.

Working-set trade to admit this Draft: LLP 1022 (the parked serial
runtime-owner research) leaves `llp/current/`. It remains in the corpus.
It is parked; this is live.

## 8. The panel (r2)

At Charlie’s request (2026-08-31), three models discussed this RFC to
convergence before the fold: **grok-4.6** at xhigh (the r1 author,
instructed to revise honestly), **gpt-5.6-sol** at ultra, and **Claude
Fable** (the orchestrating session; it did not author r1). Two rounds over
a sealed 22-file capsule — round 1 blind, round 2 mutually visible with
five named disagreements forced to final positions — full provenance,
gates, timestamps, and both rounds verbatim in
`llp/reviews/1024-native-modules.{grok,sol,fable}.md`. Round 2 ended
unanimous on every forced item and all fifteen convergence items; no split
went to Charlie.

What the panel changed, and who moved:

- **The artifact shape reversed** (r1: per-tag files under a searched
  `$EXACT_MODULES` directory → r2: one app artifact, inner table). Sol
  held it from round 1; grok and Fable moved once live-swap fell. It
  deleted the search order, the symlink generations, and — structurally —
  Fable’s path-traversal catch: plan bytes now reach a table key, never a
  path.
- **Live-swap, D7’s pointer registry, and the module ping were cut**
  (grok and sol against Fable’s round-1 keep; Fable yielded to the
  restart-shaped precedent).
- **The `display: block` claim was false CSS** (Fable’s catch, verified
  against the live web host during the panel: the CSS emitter writes
  `display` only when set, so D2 would have shipped web-inline /
  native-block). Grok: “I was wrong in r1.” The fixed-row fix and its
  ordering are D1’s now.
- **The ABI grew what the copy had dropped**: iframe’s reply callback
  (without it the fixture’s screenshot cannot answer — all three, round
  1), the versioned function table (sol’s shape), the pinned nine-event
  set closing a D1/D4 contradiction (sol), the threading/nonce rules (all
  three), snapshot demoted to a capability bit the fixture still
  implements (sol’s argument, grok’s condition).
- **The typo net became an error**: sol’s round-1 compile-time `module`
  declaration and Fable’s round-1 build warning both died; the converged
  net is grok’s roster-at-bake, on sol’s P1d grounding, with sol yielding
  its grammar in round 2.
- **The load gate became explicit** (sol: the obvious `modules.attach`
  loads during the first batch, before first pixel; D3 now owns the
  queue-then-gate mechanism) and **the web loader was corrected** to the
  injected-script rule (grok, verified against `glue.js` and `boot.mjs`).
- **The grammar tightened** to lowercase PCEN with the reserved names
  (grok), and **Linux went to `{unavailable}`** with the r1 author
  withdrawing its own composite recommendation.

The editor re-verified every load-bearing factual claim against the live
repository before folding; the per-claim results are in the three review
artifacts’ Disposition lines. Nothing in this revision rests on a panel
assertion that was not checked.

## 9. As built (Claude (Opus 5.5), 2026-09-27)

D1–D8 landed as written except where this section says otherwise. Every
row below was verified by running it; the commands are at the end.

**Compiler (D1).** `contract/lower/src/native.rs`: the lowercase PCEN
grammar with the reserved names, the fixed `display: block` row, the
known-attribute table first with `renamed()` spellings still refused, and
`load`/`message` unlocked; `src`, `sandbox` and `surface` stay refused. The
leftover attributes lower to one binding of `nativeViewProps` through a new
plan opcode, `NativeProps(n)` — the “small plan/runner helper” — whose one
implementation is `exact_runner::stdlib::native_props`: keys sorted, values
carried as strings (numbers as JavaScript prints them), an option’s `none`
leaves its key out, JSON escaping of `"`, `\` and C0 only.

- **Deviation — computed leftovers are in.** §5 cut them until “a module prop
  that is a function of state”. The photo editor is that consumer: its
  Rotate 90° and Reset controls are state the module must see. A leftover
  may be any string, number or bool expression, or an option of one; the
  aggregate is still one object, replaced whole.
- **Deviation — the roster is declared in `app.json`**, `"modules": ["tag",
  …]`, the shape GPU modules took in LLP 1009 D6, rather than emitted by the
  module crate’s build. The compiler reads it where it reads the app’s
  directory (`contract/cli/src/native.rs`, on every compile of an app path:
  the dev loop and every host’s `build.rs`), so `bake-unknown-module` names
  the tag, the roster and a one-edit suggestion before any plan exists;
  `contract::bake` itself has no app directory. The artifact’s own roster
  (its table) is checked against the declaration at build (below). The
  roster and the table’s ABI are the compatibility id’s `nativeModules`
  input (reserved as `null` until now), so a new tag is a binary change.

**The module “crate” is Swift on Apple and JavaScript on the web.** A
platform view is AppKit/UIKit: `modules/apple/*.swift` is compiled with the
host’s `host/apple/modules/ExactNativeModule.swift` (the table, the event
object, `ExactNativeInstance`, `ExactNativeFactory`) into one
`libexact_modules.dylib`; `modules/web/index.js` exports the same names.
The app declares its table as `let exactNativeModules: [String:
ExactNativeFactory]`.

**Presenters (D2–D5).** Apple: `Sources/ExactKit/NativeModule.swift`, one
`native` arm reached from both NodeView files through `embedPlatformView`,
`updateEmbedded` and `destroyEmbedded` (which also carry iframe’s lines, so
the two NodeView files shrank). The artifact is `dlopen`ed once per process
at the paint gate — the turn after first draw, beside the GPU module’s — and
never closed. The table is D4’s: `major` 1, `size` 72, the roster as a JSON
C string, `create`, `platform_view`, `set_props`, `snapshot`, `destroy`, and
two reserved null slots; `create` and `set_props` return a refusal’s text
through a caller buffer. Callbacks carry a host nonce, are copied, and
always hop to the main queue before the presenter’s gate; a retired nonce is
dropped and logged. The name is re-checked against the grammar on plan
bytes, then used only as a table key. `EXACT_MODULES` names another
artifact file only when the baked trust is not `production`. Capture merges
the tokened snapshots of snapshot-bit tags into the iframe capture map
(`Capture.web`) and hides those live views.

**Web (D7).** The node is the custom element: `tag_for` returns the module
name (re-checked). A shim in `glue.js` records each module element; after the
browser’s first paint entry `native-glue.js` loads, defines each roster tag once
as a bare `HTMLElement` subclass, and injects the app’s module as an inline
module script with a static import (never an `import()`). Props arrive as
the element’s `data-nativeviewprops` attribute and are observed; events are
nonce-checked and dispatched after the batch. A plan reload destroys every
instance before the DOM is cleared and reuses the definitions. The web
never asks for a snapshot: the page capture already composites every
element.

**Linux.** `tree` shows `module: {name, state: "unavailable", error}`.

**Build (D8.4).** `host/apple/build.mjs` compiles and copies the artifact only
when the app’s roster is non-empty, signs it beside the web arm on macOS,
and puts it in the iOS bundle’s `Frameworks`, where the embedded-dylib loop
signs it (so the simulator run traverses `codesign`). It reads the
artifact’s roster from its table (`bun:ffi`; for an iOS build, a macOS slice
of the same sources); `host/web/build.mjs` reads the web module’s `roster`
export. A roster tag the artifact lacks fails a production build, named, and
warns in development.

**The fixture (D8.3)** is `apps/native-fixture`, a fixture app rather than a
product: `exact-fixture` (the snapshot bit; paints with a layer the ordinary
capture cannot see; echoes accepted props; fires all nine events from a
background thread; refuses `reject=true`; calls back after `destroy`),
`exact-plain` (painted in `draw`, no snapshot bit) and `exact-absent` (in the
roster, never in the artifact). `scripts/smoke-native.mjs`, which
`smoke.mjs` runs for that app, asserts D8’s whole list. On Apple, the
wrong-ABI artifact is a two-line C table the smoke builds; on the web, it is
a copy of the build with its module skewed or removed.

Verified (2026-09-27):

- `bun scripts/smoke.mjs web --app native-fixture --app-only`: ok, 32 of 32
  native checks, four runs in a row (Chrome for Testing 154). The web gate
  was first two animation frames, as the GPU module’s is; the smoke caught
  the adapter starting about 2 ms before Chrome’s first paint entry, so the
  gate is now that entry (two frames and 250 ms where Chrome records none).
- `bun scripts/smoke.mjs macos --app native-fixture --app-only`: ok, 33 of 33.
- `bun scripts/smoke.mjs ios --app native-fixture --app-only`: ok, 33 of 33
  (iPhone 18 Pro simulator, iOS 27; the artifact in `Frameworks`, ad-hoc
  signed).
- `EXACT_UPDATE_TRUST=production EXACT_UPDATE_GENESIS=1 bun host/web/build.mjs
  native-fixture`, and the same for `host/apple/build.mjs`: both fail,
  naming `exact-absent`.
- `bun scripts/agent.mjs linux --plan …` on a module plan: `unavailable`.

Not verified: an iPhone device run (AMFI, a team signature), and a plan
reload on Apple hosts (the web smoke asserts it).

**Consumers (2026-09-27).** `apps/photo-editor` is the named consumer:
`<photo-editor>` is UIKit on iOS (pinch about the centroid, two-finger
rotation, a rubber-band pan that flings with its release velocity, crop
corner and edge handles, double-tap reset), AppKit on macOS (trackpad
magnify and rotation, mouse drags, double click, wheel zoom; drawn in
`draw(_:)`, so the ordinary capture sees it and no snapshot bit is needed)
and Pointer Events on the web. It reports its resting state as `change` and
`"edited"` as `message` when a gesture ends; the Contract shows the values,
and its Rotate 90° and Reset buttons — the non-gesture path — reach the
module only through the computed `turns` and `reset` props.
`apps/map-demo` is a second: `<native-map>`, MKMapView with the snapshot
bit (MKMapSnapshotter) on Apple and an OpenStreetMap tile map on the web.
Both were driven through the agent on web, macOS and the iOS simulator.
Agent-driven there: the buttons on every host; single-pointer drags (pan,
fling to the bound, crop handle) on web and macOS; double click and pin
presses on the web; a pin click on macOS. Two-pointer pinch and rotation ran
on the web only as synthetic PointerEvents dispatched in the page, which is
not one of the eight operations. Not verified: trackpad magnify and rotation
on macOS, every UIKit gesture on iOS (the simulator carrier’s contact needs
Accessibility permission this machine lacks), and double click on macOS (the
carrier sends no double click).

One lesson for module authors: an agent-mode host serves requests on the main
thread back to back, so a module’s wall-clock easing hardly advances inside
a burst of operations. The photo editor therefore reports where an edit will
rest when the gesture ends, and eases there afterwards, on a timer in every
run-loop mode.

**Working set.** §7’s trade was spent on 2026-08-31 (88ccf8c5: 1022 left
`llp/current/` and 1024 entered), and 1024’s link later left for the
Markdown reader lane (6ece0180). `llp/current/` is at 15 of 15, so 1024 does
not re-enter it here. That would take another archive, which is Charlie’s
call.
