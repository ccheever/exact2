# Panel: LLP 1024 Native modules — a tag that is not a host change (grok)

- **Family:** grok (xAI): `grok --prompt-file <brief> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --output-format plain`, cwd = the read-only capsule. Reported model not independently verifiable from the CLI; requested `grok-4.6` at `xhigh`. **This panelist authored the RFC's r1 draft in an earlier session**; the brief instructed it to defend or revise honestly, and it revised (see Disposition).
- **Method:** a three-voice panel at Charlie's request, 2026-08-31 — "Fable, sol ultra, grok 4.6 xhigh all discuss with each other," the editor folding once converged. Not a refine loop: no verdict binds, no approval given or withheld. Two rounds: **round 1 blind** (no panelist saw another), **round 2 mutually visible** (each read both peers' round-1 in full; five named disagreements A1–A5 were forced to final positions; fifteen apparent convergences B1–B15 required per-line confirmation). Third panelist: Claude Fable (the orchestrating session, which did not author the draft; its provenance in the `.fable.md` artifact). Capsule: the RFC, `CLAUDE.md` + both rules files, exact2 LLPs 1000/1001/1007/1008/1009/1012/1017.000/1020/1023, and the code as it stood (schema.json, lower lib+tags, lexer, WebArm.swift, apple host.rs, macOS Presenter.swift, apple build.mjs, web glue.js + gpu-glue.js, caltrain gpu lib.rs) — 22 files, working tree of 2026-08-31 (some carried the peer session's uncommitted edits).
- **Gates:** LLP 1024 r1 sha256 `7f7e4576fded500b4fe1822357e965dd353c4d84b04f9cb3a535816b4fcd66ab`; round-1 brief `f48cab792805eae907442cb65dc7b94aac98ece5378b80c54c9775d776b51240`; round-2 core brief `bb229706f78922962e9e6589f9c88ddbe3bfb01eae64ba8736ea5f5b6b3562e5`; this panelist's round-2 packet (core + both peers' round-1 verbatim) `e8f5aca8db26591d543e090c8564aba38533f7b1d5cab6484e7544b7f0e345a0`; capsule (evidence files, panel answers excluded) `3b5a30d20885a8bdde622df5e712b7f79bbdc38aa40c6e452362092224e69003`. No capsule file was modified while any panelist was in flight.
- **Runs:** round 1 2026-08-31T17:48:11Z → T17:55:39Z; round 2 T18:18Z → T18:24:16Z. Peers: codex `gpt-5.6-sol` at `ultra` (round 1 → T18:15:09Z, round 2 → T18:32:10Z); Claude Fable (round 1 written T17:52:07Z, before either peer's output existed; round 2 T18:19:05Z, before either peer's round 2 existed).
- **Disposition:** folded into LLP 1024 r2 — §8 records the panel. In round 2 this panelist reversed its own draft on artifact shape (per-tag → one app artifact with an inner table), on the `display: block` claim ("I was wrong in r1"), and on snapshot (required → capability bit); it held that the typo net is an error (`bake-unknown-module`), that `set_bounds` must not be required, and that no `module` production enters Contract grammar. The editor re-verified its factual claims against the live repo before folding: the injected-script loader rule (`host/web/glue.js:674-676`, `scripts/boot.mjs:44`), the lexer's uppercase acceptance (`contract/syntax/src/lexer.rs:136`), `NativeView::can_hold_children` false (`kernel/src/node.rs:10-20`), the GPU copy gate and inode comment (`host/apple/build.mjs:178, 227-228`) — all held.

---

## Round 1 — position (verbatim; the CLI's narration lines before the document marker are preserved)

I'll read the RFC, the binding rules, and the cited exact2 materials in the capsule, then write a grounded round-1 position.I'll read the RFC, the binding rules, and the cited LLPs next so every claim can be grounded in the capsule.I'll read the kernel, contract, host, and GPU/iframe citations next so the position can be checked against the actual code.I'll read the lowering path, iframe ABI, presenter arms, and packaging next — those are the load-bearing precedents.The remaining gaps are the `load`/`message` restriction, GPU glue, and how the web host maps `NativeView`.I need a few exact citations — custom-element admission, iframe replies, and the GPU loader — before writing the position.I have enough of the code and the cited LLPs to take a position. This revises my earlier draft where it disagrees with the rules or the capsule.

# LLP 1024 panel, round 1 (blind) — grok-4.6

Authorship: I drafted this RFC. I am not bound by it. The shape holds. Several decisions should not go to an implementer as written.

---

## 1. Overall position

The shape is right:

**hyphenated Contract tag → kernel `NativeView` (schema id 9, already there) → sibling artifact loaded after first pixel → one presenter arm.**

That is GPU’s “name → factory, after first pixel” (LLP 1009 D2) glued to iframe’s “on-demand dylib, C ABI, empty box if absent” (LLP 1020 D3/D5), pointed at a box the kernel already owns. It is the only design that keeps `schema.json` / `tags.rs` / a new `kind ==` arm from growing with every widget, which is the actual threat in §2. `canvas` and `iframe` staying first-party HTML tags is correct (LLP 1020 D1: iframe “is not module dispatch, because there are no modules, there is one tag”). Not ModuleIR, not a cargo feature, not a ninth agent op, not a new node type: all keep.

I would change, in leverage order:

1. **D1 is too open at compile and too quiet at bake.** Hyphen-grammar admission is right; a typo becoming a silent empty box on every host is not. The declaration is a file in the app’s `modules/` directory — the GPU `Cargo.toml` gate, not ModuleIR. **MATERIAL.**
2. **D4 does not actually copy iframe’s C ABI.** The table drops the reply callback snapshots use, does not pin threading/re-entrancy, and does not say how the box’s size reaches the module. An implementer of the fixture’s `screenshot` smoke cannot pick this up. **MATERIAL.**
3. **D3’s web loader misreads the GPU precedent.** GPU injects a `<script type="module">` after the paint stamp because `boot` counts static imports (`host-web-glue.js` 674–685; LLP 1007 §8; LLP 1009 D4). The RFC says `import()`. Copy the injected-script loader. **MATERIAL.**
4. **D6 in-process replace (recreate only that module’s views, no plan reload) is extra apparatus the landing does not earn.** Generationed inodes are already the GPU/iframe *next-launch* rule (`host-apple-build.mjs` 225–228). Live swap in a running process is strictly more than either sibling does, and without an instance snapshot it still drops a PTY. Cut it from the landing; keep generationed write. **MATERIAL** as a landing-scope cut; the design can stay as a named return.
5. **`$EXACT_MODULES` must be a dev-only locator**, same split LLP 1023 §8 already uses for network plans. A release/store binary that `dlopen`s whatever an env var names is a hole 1023 just closed for the wire. **MATERIAL.**
6. **Leftover attrs → `nativeViewProps` is underspecified** (literals vs computed; who JSON-encodes). **MATERIAL.**

What I would not change: fixture-first (D8); one host arm; one artifact per tag as the iteration model; iOS as a frozen signed roster; no native code on the LAN; no `NodeType::Terminal`; Linux as empty-box-until-a-consumer rather than a third ABI.

The “adding `ghostty-terminal` does not edit `schema.json`” claim is honest about the kernel. It is slightly dishonest about the host until the ABI is complete enough that a terminal does not punch `Presenter.swift` for keys, first-responder, and snapshot compose. That is a D4 problem, not a shape problem.

---

## 2. The admission rule (D1)

### Grammar: yes, with HTML’s actual test

The lexer already accepts hyphenated idents when a hyphen is followed by a letter (`contract-syntax-lexer.rs` 137–148). `ghostty-terminal` is one token; `x-1` is `x`, `-`, `1`. Lowering is the closed door: `tags::tag()` is a match, unknown names are `lower-unknown-tag` (`contract-lower-lib.rs` 678–679). That much the RFC has right.

The RFC’s test — “contains a hyphen and starts with an ASCII letter” (D1) — is **not** HTML’s potential-custom-element-name. HTML requires a lowercase ASCII letter, a hyphen, and exclusion of the reserved SVG/MathML names (`font-face`, `color-profile`, `annotation-xml`, …). The lexer accepts uppercase (`c.is_ascii_alphabetic()`). `Ghostty-Terminal` would compile here and fail `customElements.define` on the web host, which is the oracle (`rules-RULES.md` §Scope; LLP 1017 §8.1).

**MATERIAL:** admission is HTML’s potential-custom-element-name, lowercase, reserved names refused as `lower-unknown-tag` (or a dedicated id). Unhyphenated unknown tags stay `lower-unknown-tag`. `texxt` does not become a native view. That cut in §5 stays.

### Declaration: a file, not ModuleIR, not an open world

“Any unknown hyphenated tag is a custom element” is the web’s parse rule. Exact just spent LLP 1017 P1 closing quiet failures. A mistyped `ghosty-terminal` compiling into a `NativeView` that is an empty box on every host, with `tree` reporting `{unavailable: true}`, is a new quiet-failure class — worse than `lower-unknown-tag`, because the author sees a successful compile and a blank rectangle.

Runtime empty-box is still right **when the module is a capability this host does not have** (Linux iframe’s standing, LLP 1020 D5). That is not a typo. Distinguishing the two does not require ModuleIR (exact1 0525: generated Contract + Swift + fake from one authority). It requires the same existence gate GPU already has:

`host-apple-build.mjs` 178, 195, 229 copies the GPU dylib only when `gpu/Cargo.toml` exists. D3 already says a hyphenated tag *or* a `modules/` directory puts the file in the bundle. Make that directory (or a sibling `modules/<tag>.{js,dylib,so}`) the **declaration**.

- Compiler/bake: a hyphenated tag with no `modules/<tag>` artifact is `bake-unknown-module` (named, with the tag). That is LLP 1017 P1d’s shape — bake already boots the runner and lints the first frame.
- Runtime, artifact missing on *this* host (Linux, a stripped iOS copy, a failed `dlopen`): box stays, `tree` gets `{unavailable: true}`. Failures stay per node. D2 keeps this.
- No compile-time table in `exact-kernel`. LLP 1001 §9 left “a module registry” out of the kernel on purpose. The registry is a directory the host searches, as D5 already says.

This is not a closed tag table and not ModuleIR. It is “the file is present or not,” which is already the optional-capability rule (`CLAUDE-md.md` 47–49; `rules-RULES.md` does not want a cargo feature).

### Typo’d hyphenated tag

A silent empty box is **not** acceptable as the compile/bake outcome on any host. It is acceptable as the *runtime* outcome when the host cannot load a module the app did declare (Linux, AMFI, a packaging miss). **MATERIAL.**

### Unlocking `load` / `message`

Yes. They are currently refused on anything but `iframe` (`contract-lower-lib.rs` 1218–1223, `lower-attr-tag`). They are how iframe already signals ready and a string; the fixture’s smoke needs both. Do **not** unlock `src` / `sandbox` (same `lower-attr-tag` list) or `surface` (already `lower-surface-tag` at 1293–1298).

Handler arity already treats `message` as supplying a payload (`contract-lower-lib.rs` 1340–1351). Keep that.

`load` as “I am ready” is Exact’s closed event set, not HTML’s `load` on custom elements. That is fine. The set is closed (`rules-NOT-DOING.md` §Agent API; the handler table in `contract-lower-tags.rs` 160–169). Do not add a `ready` event.

### Leftover attrs → `nativeViewProps`

The kernel already has the box: `nativeViewModuleName` and `nativeViewProps` (`kernel-tables-schema.json` 34–35), both `kind: str`. Known attrs still bind through `tags::attr()` (style rows, handlers, `testId`, ARIA). Unknown attrs are today `lower-unknown-attr` (`contract-lower-lib.rs` 1204–1216). D1 wants leftovers to become JSON keys.

Two holes an implementer will trip on:

1. **Literal vs computed.** `iframe` `src` is one string prop and `expr_code` already allows a computed URL. Many leftover keys have to become **one** `nativeViewProps` string. The RFC does not say whether `ghostty-terminal cwd=stationId` is legal. GPU solved computed inputs as a runner side-output, not a kernel string (LLP 1009 D2). **MATERIAL:** landing is leftover values as literals only (string / number / bool), JSON-encoded at compile into `nativeViewProps`. Computed leftover attrs are a named cut, trigger “a module prop that is a function of state (terminal `cwd`).” Do not pretend the JSON object is live-bound.
2. **Name collisions with the attr table.** `width` is a style row, `type` / `value` / `id` are kernel props (`contract-lower-tags.rs` 173–208). They never enter the JSON. A module that wants a `type` prop loses. Document the reserved set as `tags::attr()`; module authors pick names that are not HTML/CSS (`cwd`, `scrollback`, `data-*`). **ADVISORY.**

Children refused: yes. Lowering already errors `lower-leaf-children` when `!t.node_type.can_hold_children()` (`contract-lower-lib.rs` 817–822). I cannot see `NativeView`’s `can_hold_children` implementation in the capsule; the RFC claims it is already false. Schema does not encode it. **ADVISORY:** the implementer confirms against generated `NodeType` before relying on it. The cut is still right.

No 300×150 default: correct. That default is the replaced-element rule `canvas` and `iframe` take from HTML (`contract-lower-tags.rs` 106–118). A custom element is `display: block` with auto sizes; NativeView is not measured (LLP 1009 D3, same as Canvas). Author sets `width` / `height`. CSS, no deviation.

---

## 3. The ABI (D4)

Copying iframe’s C ABI is the right *instinct*. The table in D4 is not that ABI.

Iframe’s actual surface (`host-apple-webarm-WebArm.swift` 14–19, 365–405):

| Export | Signature (as built) |
|---|---|
| `exact_web_create` | `(id, ctx, EventFn, ReplyFn) → handle` — **both callbacks required** |
| `exact_web_platform_view` | `handle → view*` |
| `exact_web_set_src` / `set_sandbox` | bytes + length + present flag |
| `exact_web_snapshot` | `(handle, token)` — pixels come back on **ReplyFn** |
| `exact_web_agent_eval` | guest-DOM only; RFC correctly drops this |
| `exact_web_destroy` | handle |

`EventFn` is `(ctx, id, kind: u32, bytes, len)`. `ReplyFn` is `(ctx, id, token, kind, bytes, len)`. Snapshot is asynchronous (`takeSnapshot` at 296–311) and the presenter composes guest pixels through `Capture.web` (`host-apple-macos-Presenter.swift` 652–656, 941–947).

D4 lists `exact_native_snapshot` and an “event callback” but **no reply channel**. The fixture’s smoke is `tree` + `tap`/`screenshot` round-trip (D8.3). Without a reply callback, screenshot has nowhere to put pixels. That is not a later-take. **MATERIAL:** copy `ReplyFn` + token kinds (0 = png bytes, 2 = error text — iframe’s `sendReply` at 349–358). Snapshot joins iframe’s capture handshake, not a new compose path.

Also copy the C types, not a prose table. `exact_native_event` is not an export; it is the `EventFn` passed to create, as iframe does. Integer `kind` values need a table (iframe uses 0/1/2/3 at 99–101, 239–243 with no names in the header). **MATERIAL** to pin discriminants to the existing `EventKind` set the Apple host already maps (`host-apple-host.rs` 544–553: press/change/hover/focus/blur/key/submit/load/message).

### Threading / queue

**MATERIAL**, even for the color-box fixture, because `message` re-enters the runner.

Precedent, already paid for:

- Apple C ABI: “All calls on one thread; the bridge is thread-local” (LLP 1008 §4).
- Presenter: “an event arriving while a batch is being applied waits for the batch to finish — the runner is never re-entered” (`host-apple-macos-Presenter.swift` 769–778, `applying` / `waiting`).
- WebArm hops `scheduleServe` onto the main queue (106–110).

Rule to write: every `exact_native_*` call is on the presenter thread (main on Apple). The event callback may fire off-thread; the host hops to main and goes through the same `send` gate as iframe `load`/`message`. The callback must not re-enter `apply`. `dlopen` itself is on first create, after first pixel, on that same thread (or bounced to it).

### Resize

Iframe fills the node’s box with `autoresizingMask = [.width, .height]` (`host-apple-macos-Presenter.swift` 290–295) and the presenter writes `v.web?.frame = v.bounds` on every `frame` op (859). There is no `exact_web_set_frame`. The module observes its view’s bounds.

**For the fixture:** that is enough. Specify it: the host sizes `NodeView`; the platform view fills it; the module observes bounds. No extra export in the landing.

**For a terminal with a PTY:** cols/rows are a function of the content-box in points and the font. Bounds observation is still enough if it is guaranteed. Do not add `exact_native_set_frame` until a module proves `NSView.resize` / `viewDidLayout` is not an equivalent. **ADVISORY** to say that explicitly so a Ghostty author does not invent a second size channel.

Optional `exact_native_intrinsic` stays a cut. Image already has `Kernel::set_intrinsic_size` (LLP 1001 §1; `host-apple-host.rs` 333–340). A terminal is style-sized.

### Focus and key routing

`NodeView` takes first responder when it has `press`/`focus`/`blur`/`key` (`host-apple-macos-Presenter.swift` 102–105) and implements `keyDown`. Hit-testing goes to subviews first (`hitTest` at 382; default AppKit). A module view that fills the box will eat clicks. A module view that does not override `acceptsFirstResponder` will not get keys.

**For the fixture:** make the platform view hit-test transparent (or not first-responder) so the smoke’s `tap` lands on `NodeView` the way a colored `view` does. Agent `tap` on macOS is real `NSWindow.sendEvent` at the box center (LLP 1012 §1). If the module view is topmost and ignores the click, `tap` is a no-op. **MATERIAL** for the fixture spec, not for a new ABI entry.

**For a terminal:** the module owns first-responder and keys while focused. The host must not steal `keyDown` from a focused subview (it already doesn’t — `keyDown` runs on the first responder). Agent `type` currently “a non-input target is refused on both” (LLP 1012 §1). Typing into a PTY is therefore a host/agent change, the same class as iframe’s guest `type` (`host-web-glue.js` 514–539). That is **not** in this RFC’s landing. Name it: agent `type`/`tap` into a native module is a later take, trigger “a module that is itself an editor.” Until then the agent drives a terminal through `message` or not at all. **ADVISORY** as a named cut; **MATERIAL** if the RFC keeps implying the eight operations just work through snapshot + event callback (D4, §3 “Not a ninth”). Snapshot is screenshot, not typing.

### What is extra

- `name` on `create` is only needed if one dylib hosts many tags. Under “one artifact per tag” the host already looked up by name. Harmless to keep for an iOS umbrella packaging transform. **ADVISORY.**
- No eval channel: correct (D4, §5).
- Opaque JSON instead of `src`/`sandbox`: correct for the fixture. For a terminal, JSON literals are enough for v1 (`cwd` as a literal; computed `cwd` is the leftover-attr cut above).

### Linux

`platform_view → NSView / UIView / (Linux) nothing` is honest. The Linux host is “Pure Rust; no system library is linked” (LLP 1000, `host/linux` paragraph). `dlopen` of a `.so` is a `libdl` edge that iframe refused by being absent (LLP 1020 D5). **Landing Linux is the empty box + `{unavailable: true}`.** Do not invent a paint-into-buffer export for the fixture. That is Q3; I take the other side there.

---

## 4. Lookup and lifecycle (D5, D6, D7)

### Search order

Key = tag name. Search `$EXACT_MODULES/<name>/current`, then the bundle (`Frameworks/<name>.dylib` / next to the executable / `<name>.so`), then `/modules/<name>.js` on the web. First hit wins. The shape matches how GPU is one load-name (`libexact_gpu.dylib`, `host-apple-build.mjs` 184) and iframe is `libexact_web.dylib` (185, 234–236, 264).

Two problems:

1. **`$EXACT_MODULES` in a release binary.** LLP 1023 §8: “No native executables over the wire”; “network plan loading is compiled only into dev-capable hosts.” This env var is not the network, but it is the same class of hole: unsigned native code from a path the developer did not bake. iOS will not load it anyway (D3, AMFI). macOS will. **MATERIAL:** honor `$EXACT_MODULES` only in a dev-capable host (the same compile-time split as network plans, or “only when `EXACT_DEV_PLAN` / the URL locator is live”). Store/release ignores it. Document it next to `EXACT_DEV_PLAN` / `EXACT_ASSETS`, not as a production plugin path.
2. **Web `/modules/<name>.js` is a page resource**, not a 1023 envelope payload. LLP 1023 D2 already says web GPU wasm is “part of the web app’s own fetch graph, invisible here.” Same rule. Fine, as long as it is not fetched on the pre-pixel path.

Generationed write, never in place: **keep.** `host-apple-build.mjs` 225–228 records the SIGKILL (Code Signature Invalid) failure. That rule is about the *next* `dlopen`, including first create after a rebuild while an old mapping of a *different* inode is live. Production iOS has one signed generation. Correct.

Do not `dlclose` a Swift/ObjC image: **keep** as the host rule. Ghostty is Swift. A leak of one mapping per *process lifetime* is the cost if we cut in-process swap (one mapping, period). If live-swap returns, it is one mapping per rebuild, parked. Fine.

### D6 — introduce vs replace

**Introduce a tag via plan reload:** correct, and it is already the loop. `exact_boot_plan` + `Runner::carry` keeps slots, matching resources, clock, store; tears down the tree; does not keep scroll/focus/springs (LLP 1007 §6; `rules-NOT-DOING.md` §Runtime; `host-apple-host.rs` 88–90). The new create `dlopen`s. Do not reopen identity matching. The terminal is new, which is correct.

**Replace the module in process without a plan reload:** this is new apparatus GPU and iframe do not have. GPU-crate edits reload the **page** (LLP 1007 §6; RFC D7 cites this). Native GPU/iframe rebuilds are a new process (`build.mjs --run`) or a new binary. Generationed inodes exist so the next process’s `dlopen` is not SIGKILL, not so a running presenter swaps `NSView`s.

And D6 already admits the PTY dies without an instance snapshot. So in-process swap does **not** buy “iterate Ghostty’s renderer without dropping the session.” It buys: other nodes’ scroll/focus/springs survive, GPU/iframe stay mapped, no tree rebuild. That is a nicer seconds-loop. It is not what proves the seam. The fixture does not need it. It forces D7’s pointer registry, `fs.watch` on `$EXACT_MODULES`, and the “recreate only that module’s views” path.

**MATERIAL:** cut in-process replace from the landing (add a §5 row). Return trigger: “a consumer iterating a renderer without a plan reload, with a measured cost to full `exact_boot_plan`.” Keep generationed write (D5). Keep introduce-via-plan-reload (D6 first half). iOS device remains frozen (no live add, no replace-without-reinstall). Honest.

Module-instance state does not survive a replace unless the module says so: keep this sentence even if replace is deferred. Canvas surfaces are a function of plan inputs (LLP 1009 D4: device outlives plan reloads; surfaces do not). A PTY is not. Later ABI or a sidecar process. Not a reason to delay the seam. Agreed.

### D7 — web shell + pointer

`customElements.define` throws the second time. wasm-bindgen `init` is a singleton. If in-process swap is in the landing, the shell-element-plus-pointer is the right trick, and it is the DOM analogue of swapping the `NSView` inside `NodeView`.

If swap is cut, D7 collapses: define the custom element once on first introduce; a module rebuild is a page reload, same as GPU wasm. That is worse iteration for a sibling module, and it is also what the repo does today for every other sibling. I would rather ship that than invent a pointer registry for a color box.

**MATERIAL** only as a consequence of the D6 cut. If the other panelists keep live-swap, D7 as written is right.

Parity of Ghostty-on-Metal vs xterm is the module author’s, not the kernel’s. The kernel sees a box. A fixture that paints a solid color on every host is what the smoke holds. Correct, and this is why fixture-first is not a dodge.

### Web create path

`glue.js` does `document.createElement(op.tag === "canvas" ? "div" : op.tag)` (236). I **cannot** verify from the capsule that the web wasm currently emits `"div"` for `NativeView` — `host/web/src/host.rs` is not in the capsule; LLP 1007 §1’s tag table does not mention `NativeView`. Apple `kind_for` emits `"native"` (`host-apple-host.rs` 646). For D2 to work, the web batch’s `tag` must be `nativeViewModuleName` (`ghostty-terminal`), not `"native"` and not `"div"`. `"native"` is not a valid custom element name (no hyphen). **MATERIAL** to specify the web tag emission; the Apple kind stays `"native"` plus the name in props.

---

## 5. The landing (D8) and the cuts (§5)

Fixture-first is right. Ghostty is a consumer of the ABI, not the proof. Caltrain and Weird Castle’s v1 bar do not depend on it (`rules-NOT-DOING.md` §The bar). The RFC is a proposal with no implementer, which is allowed for an RFC (it says so in §1); it must not pretend to be a spec (`rules-RULES.md` §Scope).

Landing order in D8 is the right four steps, with these amendments:

1. Compiler D1, **plus** bake-unknown-module, HTML name grammar, leftover attrs as literals-only JSON, `load`/`message` unlocked.
2. Presenter one arm + D3/D4/D5 on macOS and web. **Linux empty box.** iOS bundle copy, no live add. **No in-process swap.**
3. In-repo fixture: one hyphenated tag, colored box, `message` with a known string, snapshot of that color, hit-test transparent so `tap` works. `scripts/smoke.mjs` on web and macOS, iframe-deck style.
4. `build.mjs` copies only modules the app has. Do **not** extend iframe’s always-copy (`host-apple-build.mjs` 234–236, 264 — `libexact_web.dylib` is copied even when the app has no `iframe`; GPU is gated on `gpu/Cargo.toml` at 178). D3/D8 already say this. Keep it.

### Cuts: keep, add, do not restore

Keep every cut in §5. None of them is a mistake. `NodeType::Terminal`, ModuleIR, cargo features, migrating `canvas`/`iframe`, children, intrinsic callback, PTY snapshot, `dlclose`, live add on iOS device, native code on the LAN, unhyphenated open tags, identity matching, ninth op, eval: all stay cut.

**Add these cuts:**

| Cut | Why | Return trigger |
|---|---|---|
| In-process module replace (no plan reload) | GPU/iframe do not do this; PTY dies anyway; D7’s pointer registry exists only for it | Measured cost of `exact_boot_plan` on an app with a terminal, plus a consumer iterating a renderer |
| Computed leftover attrs | One `nativeViewProps` string; no JSON-stringify in Contract | A module prop that is a function of state |
| Agent `type` / guest-style `tap` into a native module | LLP 1012 refuses `type` on non-inputs; iframe special-cased a guest bridge | A module that is itself an editor |
| Linux `dlopen` / paint-into-screenshot | LLP 1000 “no system library is linked”; iframe D5 | A Linux app that ships a native module |
| `$EXACT_MODULES` in release binaries | LLP 1023’s dev-capable split | Never for store; never for unsigned code |

### Honesty of “does not edit `schema.json`”

True for the kernel. The first landing **does** edit `tags.rs` (the hyphenated branch, leftover attrs, `load`/`message`), both presenters (one arm), `build.mjs` (copy gate), glue (loader + tag name). That is “the same class of apparatus GPU and iframe already are, generalized so the third widget does not add a fourth arm” (§7) — which is the honest sentence. The dishonest one is the summary’s “does not edit `schema.json`, `tags.rs`, or `Presenter.swift`.” After the arm exists, the *second* widget does not edit those. Say that.

Where complexity actually lands for `ghostty-terminal`:

- Module author: C ABI, snapshot PNG, `load`/`message`, platform view, bounds, first-responder, packaging under `modules/ghostty-terminal`, a web executor (xterm or wasm) if they want web parity. That is real work. It is not host work. The RFC should say so.
- Host: one arm, once.
- Compiler: D1, once.
- Ghostty’s PTY, renderer, and agent typing stay in the app crate / later takes.

That split is the point. It only holds if D4 is complete enough that keys/focus/snapshot do not leak back into `Presenter.swift`. Right now they would.

---

## 6. The four open questions (§6)

**1. One artifact per tag, or one dylib with an inner table?**

One artifact per tag. GPU’s inner table is GPU-specific: every surface shares one wgpu device and one module ABI (`apps-caltrain-gpu-lib.rs` 221–229, `REGISTRY`; LLP 1009 D2). Native modules do not share a device. A Ghostty rebuild must not relink the color fixture. Iframe is already one artifact, one widget.

iOS signing pressure is a **packaging transform**, not an ABI choice: `build.mjs` may, later, link an umbrella `libexact_native.dylib` for device builds. `create` can keep a `name` argument so that transform does not change the C surface. Development and macOS stay one file per tag.

Charlie to confirm, as the RFC asks. My vote is one per tag.

**2. Hyphenated tag colliding with a future HTML element?**

Custom-element names with a hyphen will not collide with HTML elements; that is the web’s extension point. Freeze the *current* reserved list (`font-face` and friends) in D1 so we do not compile an SVG tag as a NativeView. If HTML ever ships `<ghostty-terminal>`, that is also the web’s problem and we deal with it as the web does (polyfill era is over; a collision is a rename). Not designed further. Agreed, with the reserved-list patch.

**3. Linux fixture.**

Landing: **unavailable**, copy LLP 1020 D5. Headless Linux is the CI host and a painter with no view tree (LLP 1000). `platform_view` has nowhere to go. `dlopen` fights “no system library is linked.” Absence is `{unavailable: true}`; the smoke’s native-module steps are web/macOS only, as the deck steps are.

A fixture that paints into the screenshot buffer is a later take when a Linux app ships a module. I disagree with the RFC’s recommendation to load-and-include in v1.

**4. Dev-server ping.**

If in-process swap is cut, a module rebuild is: new inode (D5) + restart the native host, or a plan save that triggers `exact_boot_plan` (which re-`dlopen`s on create). `EXACT_DEV_PLAN` already covers the plan half. A dylib rebuild with no Contract edit does not. That is the same as a GPU dylib rebuild today: `build.mjs --run`. Do not add `fs.watch` on `$EXACT_MODULES` in the landing.

If swap is kept, `fs.watch` on `$EXACT_MODULES` is the right landing; SSE next to the plan is the envelope-shaped sequel (LLP 1023 D2 already has `events`). I am voting to not need this yet.

---

## 7. What the RFC misses

**MATERIAL**

1. **Snapshot has no reply path.** Iframe’s `ReplyFn` is load-bearing for the fixture smoke. Copy it. Join `Capture.web`.
2. **Threading / re-entrancy** is not written. Iframe and the presenter already have the rule. The fixture’s `message` will hit it on day one.
3. **Bake-time existence check** vs silent empty box, against LLP 1017 P1.
4. **HTML custom-element grammar** (lowercase, reserved names), against LLP 1017 §8.1 / “the words are the web’s.”
5. **`nativeViewProps` lowering** (literals-only JSON in v1).
6. **Web loader**: injected script after paint, not `import()` in `glue.js` (misread of LLP 1009 D4 / `host-web-glue.js` 674–685).
7. **Web `op.tag`** must be the custom-element name. Apple kind stays `"native"`.
8. **`$EXACT_MODULES` is a release-binary hole** unless gated like LLP 1023’s dev-capable split.
9. **In-process swap is not what GPU/iframe do**; generationed inodes are. The RFC conflates them via the `build.mjs` comment.

**ADVISORY**

10. Fixture platform view should be hit-test transparent so agent `tap` is the existing path.
11. Frame size = host sets `NodeView.frame`, module view fills, module observes bounds. Say it so a terminal author does not add an export.
12. Agent `type` into a native module is an iframe-shaped special case, not implied by “joins the eight.”
13. Event `kind` integers pinned to `EventKind`.
14. Confirm `NativeView::can_hold_children` against generated kernel code; the schema does not encode it.
15. Linux `dlopen` vs LLP 1000’s “no system library is linked.”
16. The summary’s “does not edit `tags.rs` or `Presenter.swift`” is true for widget *N+1*, not for the landing.
17. Leftover attr that fuzzy-matches a CSS name (`font-szie`) should still get `renamed()`’s hint rather than vanish into JSON.

**A simpler shape the RFC almost has and then overshoots**

The minimum seam is: hyphenated tag + `modules/<tag>` file + one presenter arm + iframe C ABI including `ReplyFn` + generationed copy + empty box if absent + introduce via the existing plan-reload carry. That is GPU packaging + iframe ABI + the leftover kernel type. Live-swap, `$EXACT_MODULES` as a live registry, D7’s pointer, and SSE pings are a second RFC a Ghostty renderer loop can demand once the color box is green on two hosts.

**A capsule precedent it misread**

LLP 1009 D4 / glue.js 674–685: the GPU module is a **script element injected after the paint stamp**, never an `import()`, because `scripts/boot.mjs` counts static imports (LLP 1007 §8). D3’s “the glue `import()`s” copies the wrong sentence from D2 (“a second wasm”). Use the injected-script rule, with `/modules/<name>.js` as `s.src`.

LLP 1020 D3’s ABI includes a **reply** callback the D4 table dropped. “Copied from `WebArm.swift`’s `@_cdecl`s, not invented” is the right slogan and the table does not follow it.

---

## Concerns, tagged

| # | Concern | Tag |
|---|---|---|
| 1 | D1: typo’d hyphenated tag is a silent empty box; no bake-time `modules/<tag>` existence check | **MATERIAL** |
| 2 | D1: admission test is not HTML’s potential-custom-element-name (lowercase, reserved names) | **MATERIAL** |
| 3 | D4: no `ReplyFn` / token; fixture `screenshot` cannot land | **MATERIAL** |
| 4 | D4: no presenter-thread / no-reenter rule | **MATERIAL** |
| 5 | D3: web `import()` vs GPU injected-script + `boot` check | **MATERIAL** |
| 6 | D2: web `op.tag` must be the custom-element name | **MATERIAL** |
| 7 | Leftover attrs: literals vs computed, JSON encoding | **MATERIAL** |
| 8 | `$EXACT_MODULES` honored by release binaries | **MATERIAL** |
| 9 | D6 in-process replace in the landing (conflated with generationed inodes) | **MATERIAL** (scope) |
| 10 | Fixture hit-testing / first-responder so `tap` works | **MATERIAL** (fixture spec) |
| 11 | Linux `dlopen` in the landing vs empty box | **MATERIAL** (scope; I pick empty box) |
| 12 | Agent `type` into native modules implied by “joins the eight” | **ADVISORY** (name the cut) |
| 13 | Resize/frame observation unstated | **ADVISORY** |
| 14 | Summary over-claims “does not edit `tags.rs` / `Presenter.swift`” | **ADVISORY** |
| 15 | `can_hold_children` not visible in schema | **ADVISORY** |
| 16 | Event kind discriminant table | **ADVISORY** (but cheap; fold with #3) |
| 17 | HTML reserved-name list | **ADVISORY** (fold with #2) |
| 18 | D7 pointer registry if swap is cut | **ADVISORY** (consequence of #9) |

I would not send this RFC to an implementer until 1–11 are folded. 12–18 can ride in the same edit.

---

## Three decisions for round 2

1. **D1 declaration.** Is HTML hyphen-grammar enough at compile, with empty-box at runtime (draft as written)? Or is a `modules/<tag>` file the bake-time declaration (my position), with empty-box reserved for “declared but unloadable on this host”? If you want a declaration, is `modules/` the right home, or a Contract `module name` line (which I think is too close to ModuleIR)?

2. **D4 surface.** Copy iframe *including* `ReplyFn`, integer kinds, presenter-thread, capture handshake — and treat bounds-fill as the resize contract — or ship a smaller fixture ABI (`create` / `platform_view` / `set_props` / `destroy` / event only) and leave screenshot as “the box’s background”? I think the smaller ABI fails D8.3 the day it lands. A terminal’s extras (PTY snapshot, agent `type`, intrinsic size) stay cut either way.

3. **Live replace in the landing.** Keep D6 in-process swap + D7 pointer + `$EXACT_MODULES` watch, on the theory that sibling artifacts exist *so that* iteration does not restart the host? Or cut swap to “generationed inode + next `dlopen` (plan reload or process restart),” which is what GPU and iframe actually do, and make `$EXACT_MODULES` a dev-only override for that next load? I am on the cut. This also decides Q3 (Linux paint vs unavailable) and Q4 (ping vs nothing) as consequences.

---

## Round 2 — final positions (verbatim; narration preserved)

I'll read the full panel brief and capsule so I can take final positions on the named disagreements rather than restating shared ground.I'll locate my round-1 writeup and check the cited capsule files so the final positions rest on the code, not on memory.I have the round-1 positions. Next I’ll check the cited host, lowering, and ABI files so A1–A5 rest on the capsule, not on panel prose.I have the cited host and lowering facts. Next I’ll write the round-2 positions: only the deltas, with A1–A5 closed.# LLP 1024 round-2 position (grok-4.6)

I change my mind on artifact shape, the `display` claim, ABI versioning form, and snapshot optionality. I hold: no new Contract production, no required `set_bounds`, typo net is an **error** not a warning. Fable’s host-side name gate stays, cheaper, because the name no longer touches a path.

---

## A. Named disagreements

### A1 — Admission. Hybrid of (ii), adapted to one app artifact. Not (i), not (iii).

**CHANGED** the declaration site; **HOLD** no new Contract grammar; **HOLD** the typo is an error.

HTML’s rule is the right *parse*: a potential-custom-element-name lowers to `NativeView`. That is 1017 P9 (“the words are the web’s”), not a closed-vocabulary hole 1017 P1 was closing — `texxt` is not a custom element; `ghostty-terminal` is. A `module ghostty-terminal` production is a third thing HTML does not have, and it is a second roster next to the factory table. I refuse (iii).

Fable’s grammar-only plus a **warning** is not 1017 P1. Unknown names in this repo are errors (`lower-unknown-tag`, `lower-unknown-attr`). A successful compile of `ghosty-terminal` into a blank rectangle is the quiet-failure class, even if `tree`/`logs` later name it. I refuse (i) as stated.

**Declaration is `customElements.define`’s analogue: the app artifact’s tag→factory table.** The compiler learns that table the way it already learns GPU surfaces — a roster the module crate’s build emits, checked at bake (`llp-1009-gpu-canvas.rfc.md:60-61`; `apps-caltrain-gpu-lib.rs:221-229`). Diagnostic: `bake-unknown-module`. No `modules/<tag>` files (those die with A2). The runtime empty box is reserved for “in the roster, unloadable on this host.”

If GPU-style roster emission is too much apparatus for one fixture tag, **defer to Charlie**: take sol’s name-only `module` line rather than Fable’s warning. Either is an error. Do not ship both (two authorities). Do not ship warn-only.

What the author sees:

1. **Typo’d tag** (`ghosty-terminal`): `bake-unknown-module`, names the tag and the roster, no plan. Uppercase / reserved SVG-MathML / no hyphen: `lower-unknown-tag` at lowering (lexer still accepts uppercase, `contract-syntax-lexer.rs:136-148`). A doctored plan on the LAN that skips bake: host grammar-checks the string as a **table key** (Fable’s gate, kept), table-miss → empty box + module status + log. Never a path.
2. **Declared-but-missing artifact**: roster non-empty, dylib/js absent. Release `build.mjs` is a **hard fail** (sol; GPU already copies only when the crate exists, `host-apple-build.mjs:178,229-232`). Dev runtime miss (deleted file, AMFI): empty box, `state: "unavailable"`, log.
3. **Platform-unsupported** (Linux v1): bake succeeds, box empty, `state: "unavailable"`, log names the host. Same standing as iframe on Linux (`llp-1020-webview.rfc.md:262-266`).

### A2 — Artifact shape. **CHANGED.** One app-scoped artifact, inner tag→factory table.

Sol is right, and my r1 “one per tag” does not survive the live-swap cut. The iterate-independently argument was decisive *for in-process replace*. That is gone (B.6). What remains is link time: a Ghostty rebuild relinks a color box. GPU already accepts that for four surfaces in one `REGISTRY` (`apps-caltrain-gpu-lib.rs:221-229`; `llp-1009-gpu-canvas.rfc.md:39-61`). Split later, on measured co-loading or link cost of two large unrelated consumers — sol’s trigger, which I take.

The argument that actually moves me: **`nativeViewModuleName` is plan bytes, and a plan is network bytes** (`kernel-tables-schema.json:54`; `llp-1023-one-url-serving.rfc.md:397-401`). D5 as written concatenates that string toward `dlopen`. A table lookup on one well-known load name (`libexact_native.dylib`, the GPU move at `host-apple-build.mjs:184`) means the module name never touches the filesystem. Fable’s regex still runs, as a key check, not as a path sanitizer. That is the structural close, not a ten-line filter.

This also deletes `$EXACT_MODULES`, first-hit-wins, and per-tag generation symlinks in one stroke. Generationed **write** of the single inode stays — that comment is about the next process’s `dlopen`, not about swap (`host-apple-build.mjs:225-228`).

**Dev loop for one module.** Edit the app’s native crate (or the in-repo fixture crate). `build.mjs` rebuilds the one dylib onto a fresh inode and copies under the stable load name. Page/process restart. Plan-only edits still `exact_boot_plan` and do not relink. Iterating `ghostty-terminal` while a fixture tag lives in the same table is iterating `aurora` while `map` lives in `REGISTRY`. A single explicit app-scoped override path, honored only by dev-capable builds, may point at that inode; release resolves the signed bundle only. No directory search.

Host rule to write into D5: **never concatenate `nativeViewModuleName` into a filesystem path.** Validate PCE grammar, look up the factory, miss → unavailable. That is Fable’s MATERIAL catch, discharged by the shape.

### A3 — Bounds. **HOLD:** not a required export. Optional nullable slot only.

Iframe has no `set_frame`. The presenter already writes the child into the node’s box on every `frame` op (`host-apple-macos-Presenter.swift:852-859`) and iframe is created with `autoresizingMask = [.width, .height]` (`:290-294`). A second size channel whose numbers can disagree with that write is the drift. Linux is `{unavailable}` (B.8), so the “headless backend needs a push” case is not in v1.

PTY cols/rows: the platform view has points and backing scale (`backingScaleFactor` / `devicePixelRatio`). Observation is what a normal `NSView` terminal already does. Not an ABI hole.

**v1 contract, one sentence:** the host sizes `NodeView`; the module view fills it; the module observes bounds and scale from the platform view. No export required.

**Concession to the function table:** a **nullable** `set_bounds(w, h, scale)` pointer may exist in the table. Host calls it iff non-null, with the **same** w/h as the frame op it just applied and the view’s backing scale — one source. Fixture leaves it null. Ghostty may fill it if Metal makes observation awkward. Required entry: no.

### A4 — `display`. **CHANGED.** I was wrong in r1. Accept Fable’s fix, with one tightening.

The RFC’s “a custom element is `display: block` … CSS, no deviation” is false CSS: no UA rule, so `display: inline`. Kernel default is `block` (`kernel-tables-schema.json:80`). Web CSS emitter writes `display` only when the row is set (`llp-1007-web-host-v1.spec.md:56-59`: every **set** row). D2’s custom-element tag with no `display` row is therefore **inline on web, block on native** — the four-disagreeing-default-layers class `CLAUDE-md.md:7-17` names as the reason this repo exists. Declaring a 1001 deviation would leave the parity harness lying about a bare module tag. Do not.

**Fix:** hyphenated-tag lowering emits a `display: block` row, `column`’s fixed-row precedent (`contract-lower-tags.rs:65-71`). Plan-visible, every host agrees, oracle HTML says `display:block`.

**Tightening:** `column`’s `display:flex` *is* the tag. Block is the missing UA default, so it must yield to an authored `display`. Last-wins already does this if the fixed row is applied *before* attributes (`contract-lower-lib.rs:799-809`; 1017: “a tag’s fixed row and an attribute — the last one wins”). Specify that order. If the lowerer ever applies fixed rows after attributes, emit the block row only when the author did not bind `display`.

**Also take Fable’s optional:** the web shell’s constructor (and/or `:host`) sets `display: block` so the pre-upgrade custom element does not flash inline. Baseline styling against a different tag identity, which 1007 documented for `div` (`llp-1007-web-host-v1.spec.md:47-73`).

### A5 — Snapshot. **CHANGED** toward sol, fixture still implements.

Iframe’s tokened snapshot exists because WKWebView layers are remote and do not appear in ordinary capture (`llp-1020-webview.rfc.md:240-253,270-284`; compose is `Capture.web`, `host-apple-macos-Presenter.swift:652-656`). A color `NSView` composites in the normal screenshot. Requiring PNG production of every module is an iframe tax on a box. Capability bit, optional pointer in the table.

Ghostty is Metal-backed; canvas already special-cases Metal readback under capture (`host-apple-macos-Presenter.swift:565-571`). The first real consumer will set the bit. So:

**optional-with-capability-bit, and the fixture implements it.** D8.3 asserts *both* ordinary capture (the box is visible without the bit) *and* the tokened path (ReplyFn + token kinds 0/2, copied from `host-apple-webarm-WebArm.swift:14-19,349-369`). An ordinary NSView module omits the bit. ReplyFn stays on `create` regardless — snapshot is not its only client.

---

## B. Convergence — one line each

1. Assent. **Sol’s function-table**, one exported getter (`exact_native_abi` → version + struct size + capability bits + roster + pointers). Not a u32 plus independently `dlsym`’d symbols; iframe can do that because it is built in the same `build.mjs` run, and a module is not. ReplyFn + token kinds on `create`, verbatim.
2. Assent. UI-thread exports; any-thread callbacks copied, queued, never re-entering `apply` (`host-apple-macos-Presenter.swift:769-809`; LLP 1008:147); nonce/stale drop.
3. Assent. Injected `<script type="module">` after the paint stamp, never `import()` (`host-web-glue.js:674-676,705-711`).
4. Assent. Creates in the initial batch queue an empty box; artifact load starts at the established post-paint gate (`host-apple-macos-Presenter.swift:573-580`; glue `:709-711`).
5. Assent. HTML potential-custom-element-name, lowercase, reserved SVG/MathML refused; do not normalize uppercase.
6. Assent. In-process replace cut, named return; D7 = define-once + page/process restart; no `fs.watch`/SSE ping; `dlclose` never in v1, no C/Rust exception.
7. Assent. No `$EXACT_MODULES` directory; one explicit app-scoped path, dev-capable only; release = signed bundle. Stronger under A2: that path *is* the one artifact.
8. Assent. Linux `{unavailable}`; no `dlopen`; host links no system library (`llp-1000-exact2-root.explainer.md:72-79`). I withdraw the RFC’s load-and-composite recommendation.
9. Assent. Web `op.tag` = custom-element name; Apple kind stays `"native"`.
10. Assent. `load`/`message` on module tags; `src`/`sandbox`/`surface` refused; D4 pins all nine handlers with payloads and integer discriminants (`host-apple-host.rs:544-553`).
11. Assent. Literals only; full-replacement canonically-sorted JSON; computed a named cut; known attrs bind first; `renamed()` still refused.
12. Assent. Module status object `{name, state, error}`, not bare `{unavailable: true}`; per node; loud in `logs`.
13. Assent. Agent `type`/guest-`tap` named cut; `agent_input` reserved in the table numbering, not implemented.
14. Assent. Lands when a consumer and an implementer are named; v1 bar does not move; “does not edit `tags.rs`/`Presenter.swift`” is true for widget N+1, not the landing.
15. Assent, plus A4’s display/sizing pin and A5’s dual capture assertion.

---

## C. Close

**(1) Converged plan.** A hyphenated PCE name lowers to the existing `NativeView` with a plan-visible `display: block` default (overridable), leftovers as one canonically-sorted literal JSON props blob, `load`/`message` unlocked, `renamed()` still refused. Bake fails `bake-unknown-module` unless the tag is in the app artifact’s emitted factory roster — that table is the declaration, analogue of `customElements.define`; no new Contract production unless Charlie prefers sol’s name-only line as the cheaper error. One app-native artifact and one app-relative web artifact, each a tag→factory table, loaded after first paint through the existing GPU gates (injected script, not `import()`; Apple `loadIfNeeded` on the next turn after first draw). The module name is a table key, never a path; Fable’s grammar check still runs on plan bytes. D4 is one versioned function table with ReplyFn, threading/reentrancy/nonce rules, optional `snapshot` and nullable `set_bounds`/`agent_input`; required surface is create / platform_view / set_props / destroy. Host sizes the box; the view fills it. Linux is unavailable. Missing release artifact is a build error; unloadable-on-this-host is an empty box with a status object. In-process swap, `$EXACT_MODULES`, `dlclose`, computed props, agent typing into the module, and live-add on iOS device are cuts. Module-code edits restart the page/process; plan edits `exact_boot_plan`. The landing is that seam plus a fixture that is hit-test transparent, implements snapshot anyway, and asserts the failure paths — and it does not start until a consumer and an implementer are named.

**(2) Block if the editor folded tonight.** (a) A4’s plan-visible `display: block` — shipping web-inline/native-block is the bug class this repository exists to prevent. (b) D5 must not concatenate `nativeViewModuleName` into a `dlopen` path; one-artifact table lookup is the close I want, Fable’s host grammar gate on per-tag paths is the acceptable fallback; D5 as written is not. (c) Typo net is an error (`bake-unknown-module` or `module` line), not a warning. (d) Function table + ReplyFn + threading written into D4; the prose table cannot answer a snapshot or survive skew. (e) Named consumer and implementer; fixture-only does not authorize the apparatus. (f) The post-paint queue, not “first use” as prose.

I would not block on nullable `set_bounds` existing in the table, on snapshot being optional, or on Charlie picking sol’s `module` line over GPU-style roster emission. I would block on `set_bounds` as **required**, on grammar-only with warn-and-empty-box as the typo outcome, and on independent unversioned `dlsym`s.
