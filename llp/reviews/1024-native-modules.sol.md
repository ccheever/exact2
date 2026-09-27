# Panel: LLP 1024 Native modules — a tag that is not a host change (sol)

- **Family:** OpenAI: `codex exec -m gpt-5.6-sol -c model_reasoning_effort=ultra -s read-only --skip-git-repo-check -` (prompt on stdin), cwd = the read-only capsule; no repository access. Requested `gpt-5.6-sol` at `ultra` (Charlie named the effort: "sol ultra").
- **Method:** a three-voice panel at Charlie's request, 2026-08-31 — "Fable, sol ultra, grok 4.6 xhigh all discuss with each other," the editor folding once converged. Not a refine loop: no verdict binds, no approval given or withheld. Two rounds: **round 1 blind**, **round 2 mutually visible** (each read both peers' round-1 in full; five named disagreements A1–A5 forced to final positions; fifteen convergences B1–B15 confirmed per line). Peers: grok-4.6 at xhigh (the RFC's original author, revising) and Claude Fable (the orchestrating session). Capsule contents: see the grok artifact — identical capsule, identical gates.
- **Gates:** LLP 1024 r1 sha256 `7f7e4576fded500b4fe1822357e965dd353c4d84b04f9cb3a535816b4fcd66ab`; round-1 brief `f48cab792805eae907442cb65dc7b94aac98ece5378b80c54c9775d776b51240`; round-2 core brief `bb229706f78922962e9e6589f9c88ddbe3bfb01eae64ba8736ea5f5b6b3562e5`; this panelist's round-2 packet `a49394031e283614c53775ef5c0b9bf7667cc54a3991670d2c4a1d7874415bf4`; capsule `3b5a30d20885a8bdde622df5e712b7f79bbdc38aa40c6e452362092224e69003`. No capsule file was modified while any panelist was in flight.
- **Runs:** round 1 2026-08-31T17:48:11Z → T18:15:09Z; round 2 T18:18Z → T18:32:10Z. Peers: grok round 1 → T17:55:39Z, round 2 → T18:24:16Z; Fable round 1 T17:52:07Z, round 2 T18:19:05Z.
- **Disposition:** folded into LLP 1024 r2 — §8 records the panel. This panelist's round-1 catches drove the largest revisions: the one-app-artifact-with-inner-table shape (adopted 3-0 in round 2, deleting `$EXACT_MODULES` and the plan-name→filesystem edge), the pre-first-pixel load hazard in the obvious `modules.attach` (the post-paint queue is now D3's rule), the D1/D4 event-set contradiction, the unimplementable leftover-attrs lowering (now a literals-only canonical aggregate), and the versioned function-table ABI. In round 2 it yielded its Contract `module` declaration to the build-emitted roster checked at bake, and dropped its required `set_bounds` to bounds observation. The editor re-verified its round-2 claims against the live repo before folding: fixed rows precede authored attributes with last-binding-wins (`contract/lower/src/lib.rs:743-812`), the post-first-draw GPU load gate (`host/apple/macos/Sources/ExactMac/Presenter.swift:573-580`), the `cacheDisplay` capture path (`Presenter.swift:948-978`), the bake-lint precedent P1d (`llp/1017.000-contract-v1-1.spec.md:76-87`), the 30-second edit budget (`rules/RULES.md:35-43`) — all held.

---

## Round 1 — position (verbatim)

# Round 1 position — LLP 1024

This is a design position, not an approval or status recommendation.

## 1. Overall position

The architectural spine is right: an authored module tag should lower to the existing `NativeView`, load app-owned code outside the core host, and pass through one generic presenter arm. `NativeView` is already schema id 9 with string props for its module name and payload; Apple already serializes it as kind `"native"`, but the presenter has no corresponding creation branch yet (`kernel-tables-schema.json:14-16,54-55`; `host-apple-host.rs:642-656`; `host-apple-macos-Presenter.swift:267-295`).

I would materially change the plan in this order:

1. **MATERIAL — Make hyphenation eligibility, not admission.** Require a name-only Contract declaration so typos still fail at compile time.
2. **MATERIAL — Use one app-scoped sibling artifact with an inner tag→factory table initially.** Remove the global `$EXACT_MODULES` directory and per-tag search machinery.
3. **MATERIAL — Replace the prose ABI with a versioned function table covering errors, ownership, threading, resize, focus/input, callbacks, and optional capture.**
4. **MATERIAL — Cut in-process dylib/web-module HMR from the first landing.** Restart the page/process after code rebuilds; retain plan reload’s existing carried-state semantics.
5. **MATERIAL — Require a real consumer and implementer before landing.** The fixture verifies a seam but does not justify the registry, loader, ABI, and packaging apparatus.

That leaves a smaller design: declared tag → `NativeView` → one app-native artifact → factory lookup → platform view, loaded after first paint.

## 2. The admission rule (D1)

**MATERIAL — Any unknown hyphenated tag is too permissive.** Contract currently has no fallback tag or attribute: the closed table returns `None`, lowering emits `lower-unknown-tag`, and unknown attributes are rejected (`contract-lower-tags.rs:9-14,54-128`; `contract-lower-lib.rs:671-680,1204-1216`). Contract v1.1 explicitly records this as “no quiet failures,” including diagnostics for unknown hyphenated names (`llp-1017.000-contract-v1-1.spec.md:20-48`).

Use a name-only declaration:

```contract
module ghostty-terminal
```

It should contain no prop schema, generated bindings, platform paths, or deployment policy. It travels with a transitively `use`d Contract file, as components and their supporting declarations already do, and otherwise compiles away; each occurrence still writes `nativeViewModuleName` (`llp-1017.000-contract-v1-1.spec.md:154-173`; `kernel-tables-schema.json:54-55`). This is not ModuleIR—it merely distinguishes author intent from a typo.

Thus:

- `ghostty-terminal` after a matching declaration lowers to `NativeView`.
- `ghostty-termnial` remains `lower-unknown-tag`, ideally with a nearest-declaration suggestion.
- A normal typed Contract component should wrap the raw module tag for app authors.

**MATERIAL — Specify one exact lowercase name grammar.** The lexer currently admits uppercase letters and underscores, and only treats a hyphen as part of an identifier when followed by an ASCII letter (`contract-syntax-lexer.rs:136-155`). D1’s “starts with an ASCII letter and contains a hyphen” is therefore not even identical to the lexer’s grammar, much less a sufficient web contract. Use a conservative canonical subset such as:

```text
[a-z][a-z0-9_]*(?:-[a-z][a-z0-9_]*)+
```

Reject uppercase rather than normalizing it; source spelling, DOM tag, factory key, and diagnostics must remain identical.

**MATERIAL — “Leftover attrs become JSON” lacks an implementable lowering rule.** There is one `nativeViewProps: str`, while existing lowering produces one expression binding per known prop and reduces duplicate bindings to the last (`kernel-tables-schema.json:54-55`; `contract-lower-lib.rs:799-812,1283-1290`). Simply routing each unknown attribute to that prop would retain only one.

D1 must define a single reactive aggregate:

- Full replacement, not patches.
- Canonically sorted top-level object.
- String values in v1, matching the hosts’ existing string prop projection; cleared attributes are absent (`host-apple-host.rs:659-671`).
- `class`, style rows, and handlers are host-only.
- Other authored attributes—including generic accessibility/test attributes—are visible to the module, though the host may also consume them.
- Deterministic escaping and number/bool-to-string conversion.
- A rejected replacement leaves the last accepted configuration active.

Producing this aggregate may require a small plan/runner helper; D8 must budget that work rather than implying the existing schema string solves it.

**ADVISORY — Unlock `load` and `message`, but only on declared module tags.** Both names already exist as handlers and `EventKind`s; the present restriction is merely an iframe-only check (`contract-lower-tags.rs:160-169`; `contract-lower-lib.rs:1218-1224,1362-1374`). Define `load` as “the instance accepted initial props and bounds and is attached,” and keep `message` as a low-rate UTF-8 control event—not a terminal byte stream.

**MATERIAL — Empty-box failure containment must not be silent success.** LLP 1012’s current `tree` shape has no `unavailable` field (`llp-1012-agent-api-v1.spec.md:33-42`). The web host already augments iframe nodes with host-owned status, so a generic module status is possible, but it must be specified (`host-web-glue.js:472-482`). I recommend:

```json
"module": {
  "name": "ghostty-terminal",
  "state": "loading|ready|unavailable|failed",
  "error": "..."
}
```

A missing required artifact in a release build is a packaging failure. A remote dev plan referencing an absent local module, or Linux’s declared unsupported case, may retain the empty box—but must expose this status and a stable log diagnostic.

## 3. The ABI (D4)

Opaque handles plus full-replacement JSON are the right general boundary. Literal copying of the iframe symbols is not.

**MATERIAL — Snapshot cannot complete through the proposed ABI.** D4’s `create` receives only `event_fn`, while `snapshot(handle, token)` returns nothing (`llp-1024-native-modules.rfc.md:183-190`). The actual iframe ABI passes separate event and reply callbacks, and snapshot asynchronously returns PNG or error bytes through the tokened reply (`host-apple-webarm-WebArm.swift:14-19,296-311,349-369`).

**MATERIAL — Use one versioned entry point returning a size-versioned function table.** Six independently looked-up, unversioned symbols are acceptable for a first-party dylib built in lockstep, but not for searched or replaceable app code. The descriptor should contain:

| Surface | Required contract |
|---|---|
| Identity | ABI version, struct size, exported tag roster, capability bits |
| `create` | Tag, initial props, host callbacks, host-issued instance token; typed success/error |
| `platform_view` | Borrowed `NSView`/`UIView`; module retains ownership |
| `set_props` | Full JSON replacement; typed rejection leaves previous props active |
| `set_bounds` | Content-box width/height in points plus scale |
| `agent_input` | Optional implementation of existing `tap`/`type` forms |
| `snapshot` | Optional asynchronous turn-capture using the reply callback |
| `destroy` | Last host call; invalidates the instance |
| Callbacks | Event, reply, diagnostic; each carries the instance token |

No exception or Rust unwind may cross the C boundary. Input buffers are borrowed for the call; callback bytes are borrowed for the callback and copied immediately by the host.

**MATERIAL — D1 and D4 disagree on events.** D1 admits nine handlers, while D4 omits `press`, `hover`, and `submit`; yet D8 expects a tap to traverse the ABI (`llp-1024-native-modules.rfc.md:110-124,185-190,287-291`). Support the complete existing set:

- Empty payload: `press`, `focus`, `blur`, `submit`, `load`.
- Boolean: `hover`.
- UTF-8 string: `change`, `key`, `message`.

The Apple host already includes all nine handler names in creation batches (`host-apple-host.rs:533-558`).

**MATERIAL — Specify threading, reentrancy, and stale-callback handling.** Apple’s host ABI otherwise runs on one thread; worker results are explicitly queued and pumped on the presenter thread. Presenter events that arrive during a batch are similarly queued, then dropped if their view died (`llp-1008-apple-host-v1.spec.md:115-147`; `host-apple-macos-Presenter.swift:769-809`).

The module rule should be:

- Lifecycle/view entry points run on the platform UI thread.
- Event and reply callbacks may originate on any thread.
- The host copies bytes, queues onto the presenter thread, and never re-enters the runner or module synchronously.
- Every instance receives a host nonce; callbacks from an invalidated nonce are dropped.
- Destroy invalidates the nonce before invoking module teardown.

This is essential for a PTY, whose reads and process-exit notifications naturally occur off the UI thread.

**MATERIAL — Resize and focus/input are missing.** The host emits changed frames and explicitly resizes the iframe child today (`host-apple-host.rs:459-483`; `host-apple-macos-Presenter.swift:852-860`). Add `set_bounds`; platform-view observation alone cannot cover a future headless backend.

The returned module view—not merely the Exact wrapper—must be eligible for first responder and tab order. Current focus routing chooses an input field or `NodeView`, and the agent’s text operation is currently input-specific (`host-apple-macos-Presenter.swift:887-927`; `llp-1012-agent-api-v1.spec.md:75-84`). A terminal must receive real native text input, IME composition, paste, modifiers, and selection through its platform view. Contract’s normalized `key` callback is observation, not the PTY input channel. An optional `agent_input` maps the existing eight-operation API onto the module without adding a ninth operation.

**ADVISORY — Snapshot should be optional.** WKWebView needs a special snapshot because its remote layers do not appear in ordinary capture; LLP 1020 explicitly limits that expensive path to turn capture, not live canvas composition (`llp-1020-webview.rfc.md:240-253,270-284`). Ordinary native views should use the host screenshot path. Only remote/GPU-backed modules need the tokened snapshot capability.

For a real terminal, the module may own its PTY initially. PTY bytes stay internal; props configure cwd/theme/command, `set_bounds` drives rows and columns, and `message` remains low-rate. Preserving a shell across process or renderer replacement still requires a stable sidecar/host-owned PTY and may remain cut until that consumer arrives.

## 4. Lookup and lifecycle (D5, D6, D7)

**MATERIAL — Prefer one app artifact with an inner factory table.** That is the actual GPU precedent: one artifact per app, whose Caltrain implementation exports four `(name, arity, factory)` rows (`llp-1009-gpu-canvas.rfc.md:39-75`; `apps-caltrain-gpu-lib.rs:221-229`). The build recognizes one app-owned GPU crate and copies it under one stable load name (`host-apple-build.mjs:175-195`).

Use the same first shape:

- One app-native artifact per platform, loaded at the first `NativeView`.
- One app-relative web artifact.
- A hand-written tag→factory table inside each.
- No artifact for an app with no native modules.
- Split artifacts only after two large unrelated consumers demonstrate harmful co-loading or relinking.

This keeps `name` meaningful in `create` and removes most filesystem registry machinery.

**MATERIAL — Remove `$EXACT_MODULES` from v1.** It is not necessarily escalation for someone already controlling the process environment, but it is ambient, global across apps, and lets a fetched plan activate code outside the active app’s bundle. LLP 1023 treats app identity as compatibility data rather than trust and scopes executable capability through the app’s `BundleEntry`; native executables never arrive over the network (`llp-1023-one-url-serving.rfc.md:265-304,386-403`).

Resolution should be app-scoped:

- Release Apple hosts: only the signed bundled artifact.
- macOS development: the active app’s locally rebuilt artifact.
- Web: an app-relative, same-origin page asset—not an envelope/native-code URL.
- Linux v1: unsupported, so no loader.

If a dev override is later necessary, make it one explicit active-app artifact path, honored only by dev-capable builds—not a first-hit-wins directory.

**MATERIAL — Initial attach currently risks violating the post-first-pixel rule.** Presenter creation occurs while the first batch is being applied; calling `modules.attach` there would synchronously load code before first draw (`host-apple-macos-Presenter.swift:799-821`). The existing Apple GPU path waits until the first draw and dispatches loading to the next turn; the web injects its module script only from the animation frame after the paint stamp (`host-apple-macos-Presenter.swift:573-580`; `host-web-glue.js:674-684,705-711`).

Create an empty box and queue props/bounds during initial apply. Start artifact loading only at that established post-paint gate. A node created after the host’s first pixel may attach immediately.

**MATERIAL — Cut in-process code replacement from the first landing.** A fresh inode solves overwriting a mapped signed file; current code proves exactly that and no more (`host-apple-build.mjs:224-236`). It does not prove that two Swift/ObjC generations, callbacks, threads, and type registrations can safely coexist.

The simpler precedent is restart-shaped:

- Plan edits rebuild the tree while carrying compatible runner state, with explicitly no generations (`llp-1007-web-host-v1.spec.md:196-218`).
- Web program edits reload the page; native Rust edits require a new binary (`llp-1007-web-host-v1.spec.md:226-240`).
- Native `{rebuilt}` is session-terminal under LLP 1023 (`llp-1023-one-url-serving.rfc.md:179-189`).

For v1:

- Publish the rebuilt artifact on a fresh inode.
- Restart the page/process.
- Never `dlclose`; keep the opened image mapped until process exit.
- Be honest that code rebuild does not preserve in-memory runner or PTY state.

If measured need later earns HMR, require generation-addressed artifacts, load-and-validate-before-swap, instance→generation ownership, transactional replacement of every instance, last-good fallback, stale-callback suppression, focus restoration, and a restart fallback. The draft currently specifies none of those fully.

**ADVISORY — Keep the host-owned web shell, but install its implementation once.** It gives Exact a stable element, baseline styling, and predictable mount/setProps/unmount behavior across plan reloads. Defer mutable generation pointers. A plan reload must explicitly unmount module instances before clearing DOM state, analogous to the GPU reset that occurs before `views.clear()` and `root.replaceChildren()` (`host-web-glue.js:641-652`).

**MATERIAL — Follow the actual web loading precedent.** The RFC says `import()`, while current GPU glue deliberately injects `<script type="module">` after paint and says an import is counted by the boot check (`host-web-glue.js:674-684`; `llp-1007-web-host-v1.spec.md:279-286`). Use the established injected-script gate or explicitly redesign the check; do not leave the contradiction to the implementer.

## 5. The landing (D8) and cuts

Fixture-first is the correct implementation order once the work has a real consumer. Fixture-only is not sufficient authorization or ABI proof.

**MATERIAL — The feature-scope trade is unpaid.** The binding v1 scope says only Caltrain and Weird Castle’s wordmark exist; everything else does not. LLP 1024 itself says neither depends on native modules, while the kernel spec explicitly leaves a module registry out until a consumer exists (`rules-DEFERRED.md:7-15,128-133`; `llp-1024-native-modules.rfc.md:296-298,344-353`; `llp-1001-kernel-v1.spec.md:222-231`). Removing LLP 1022 from the working-set overlay pays the document-count budget, not the implementation trade.

Before landing, either:

- name the consumer, implementer, and displaced v1 work; or
- defer implementation until after the v1 bar.

**MATERIAL — Strengthen the fixture.** The colored-box fixture should additionally cover:

- multiple reactive props, clearing, ordering, and JSON escaping;
- create/setProps/setBounds/destroy/recreate;
- all advertised events;
- a background-thread late callback after destroy;
- missing artifact, wrong ABI, rejected props, and failed create;
- proof that initial loading begins after paint;
- plan reload without duplicate shell definition;
- normal capture and optional snapshot;
- signed bundled execution on iOS simulator, with a physical-device signing/load run asynchronously.

Existing packaging signs embedded dylibs before signing the iOS app, so the fixture must actually traverse that path rather than merely be copied (`host-apple-build.mjs:252-277`).

**ADVISORY — Keep these cuts:** per-widget node types, ModuleIR/generated fakes, core cargo features, migrating `canvas`/`iframe`, children, intrinsic measurement, generic eval, PTY state serialization, identity-matching plan reloads, native code over the LAN, and a ninth agent operation.

**MATERIAL — Change these cuts:**

- Make `dlclose` “never in v1,” with no supposedly pure C/Rust exception.
- Add in-process module HMR itself to the cut list.
- Make snapshot optional rather than universal.
- Cut Linux loading until a real live rendering/view target exists.
- Treat a missing required release artifact as a build error, not as the justification for an empty box.

The “no `schema.json`/`tags.rs`/Presenter edit” headline remains narrowly true after the seam, but complexity moves to the app module: the Contract declaration and typed wrapper, app factory table, native and web implementations, JSON validation/defaults, ABI adapter, input/focus/accessibility behavior, packaging/signing, and parity tests. The RFC should say “no further Exact-core dispatch edits,” not imply that adding a terminal becomes cheap.

## 6. Positions on the four open questions

1. **MATERIAL — One app artifact with an inner table.** Follow GPU’s proven app-module shape. Split per tag only after measured co-loading or iteration cost.

2. **ADVISORY — Freeze the namespace rule, not a global name ledger.** Declared hyphenated names always mean modules; Exact should not silently reinterpret one as a future built-in. Before 1.0, an unavoidable web/platform collision becomes an explicit compiler error and deliberate rename, consistent with the repository’s lack of API stability (`rules-DEFERRED.md:141-146`).

3. **MATERIAL — Linux is explicitly unavailable in v1.** Its host directly paints the kernel tree through Vello/tiny-skia and has no platform-view presenter, while D4 returns no Linux view (`llp-1000-exact2-root.explainer.md:72-80`; `llp-1024-native-modules.rfc.md:183-190`). Do not `dlopen` a fixture merely to paint through a screenshot side channel. A real Linux consumer must first choose a toolkit view or host-owned pixel/render target.

4. **ADVISORY — No module-rebuild ping in the first landing.** Use the existing page/process rebuild path. If HMR later earns scope, extend the existing SSE protocol with a typed module-generation event and current-generation hello; do not create a native-only `fs.watch` protocol. The event may carry name and digest, but never a native code URL or machine path; native hosts resolve a locally published app artifact.

## 7. What the RFC misses

**MATERIAL — Failure mode:** the obvious `modules.attach` implementation loads during the first create batch, before first paint. The RFC’s lazy-load promise needs an explicit queue and paint gate, not merely the phrase “first use.”

**MATERIAL — Misread precedent:** GPU demonstrates one app artifact with an inner registry, not per-tag filesystem discovery. Iframe demonstrates a special asynchronous capture for remote WebKit layers, not a mandatory snapshot ABI for every native view.

**ADVISORY — Web baseline:** the existing web host documents its reset and defaults in terms of a bare `div`, while the new DOM node has a different tag identity (`llp-1007-web-host-v1.spec.md:47-73,143-158`). The fixture should pin the bare module box’s display, sizing, clipping, hit-testing, and accessibility behavior rather than assume the generic shell inherits the correct baseline.

The simpler overlooked shape is:

1. A name-only Contract declaration.
2. The existing `NativeView` and its two props.
3. One app-native artifact and one web artifact, each with a tag→factory table.
4. A versioned ABI with props, bounds, input, lifecycle, and optional capture.
5. Deferred loading after first paint.
6. Linux unavailable.
7. Page/process restart for module-code edits.

## Three decisions for round 2

1. Should undeclared hyphenated names remain compiler errors, with a name-only `module` declaration as the admission boundary?
2. Should v1 use one app artifact plus restart-on-code-change, or pay now for per-tag artifacts and in-process generation swapping?
3. Should the minimum ABI include versioning, bounds, focus/agent input, any-thread callbacks, and optional snapshot while declaring Linux unavailable?

---

## Round 2 — final positions (verbatim)

## A. Final positions

**A1 — Change from (iii) to a precise (ii)/A2 hybrid.** Lowering admits HTML potential-custom-element names; the app artifact’s build-declared factory roster is the declaration and emits bake metadata from that same authority. There is no `module …` Contract grammar and no hand-maintained second manifest. A roster miss is fatal `bake-unknown-module`, not a warning: bake already produces named diagnostics that every host build fails, and GPU already establishes a build-emitted roster checked by the compiler ([LLP 1017:76–87](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/llp-1017.000-contract-v1-1.spec.md:76), [LLP 1009:60–63](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/llp-1009-gpu-canvas.rfc.md:60)).

1. A syntactically invalid name remains `lower-unknown-tag`; a grammar-valid typo such as `ghosty-terminal` reaches bake and gets `bake-unknown-module`.
2. If the roster claims target support but the aggregate artifact or factory is absent, bundle assembly fails with a named missing-artifact/factory diagnostic. If disappearance, signing, ABI, or load failure is discoverable only at runtime, the box remains and `tree` reports `state:"error"` with the cause; `logs` is loud.
3. A deliberately unsupported platform reports `state:"unavailable"` with `error:"unsupported on <host>"`; that is the legitimate empty-box case, not conflated with a typo or broken package.

**A2 — Keep the app-scoped artifact with an inner tag→factory table.** V1 has one fixed artifact per app/executor, loaded after the established paint gate. `nativeViewModuleName` is host-validated and used only for an in-memory lookup; it never enters a path. Release loads the bundle’s fixed artifact, while dev-capable builds may use one explicit app-artifact path. This deletes `$EXACT_MODULES`, search order, first-hit semantics, per-tag `current` symlinks, and the plan-name→filesystem edge in D5. That edge is real because D5 currently interpolates the name into paths while the schema constrains it only as a string ([LLP 1024:202–219](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/llp-1024-native-modules.rfc.md:202), [schema:54–55](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/kernel-tables-schema.json:54)); network-delivered plans have already demonstrated that they can drive a local binary ([LLP 1023:551–554](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/llp-1023-one-url-serving.rfc.md:551)). GPU’s single app artifact and factory registry are the direct precedent ([apps-caltrain-gpu-lib.rs:221–229](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/apps-caltrain-gpu-lib.rs:221)).

The dev loop is: Contract-only edits remain plan reloads; a module-code edit incrementally rebuilds and relinks the one app module artifact, installs it at the fixed destination using a fresh inode, then restarts the native process or web page. The host executable is not relinked. Remove-then-copy at a stable loader name is already the native inode-safe pattern ([host-apple-build.mjs:224–236](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/host-apple-build.mjs:224)). Split per tag only if measured aggregate relink time breaches the 30-second edit budget or unrelated module fetch/load cost becomes material ([rules-RULES.md:35–43](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/rules-RULES.md:35)).

**A3 — I change to bounds observation; no `set_bounds` entry in v1.** The host must keep the returned platform view equal to the node’s box; the module observes that view’s bounds and backing scale. That is the sole size channel. The iframe precedent installs an autoresizing child and also writes `frame = bounds` on each frame op, with no frame export in its ABI ([Presenter.swift:290–295](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/host-apple-macos-Presenter.swift:290), [Presenter.swift:852–860](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/host-apple-macos-Presenter.swift:852), [WebArm.swift:365–405](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/host-apple-webarm-WebArm.swift:365)). Since Linux is unavailable in v1, headless operation does not justify a second channel. A future non-view executor can earn an optional table extension.

**A4 — Accept Fable’s fix, sharpened to one authority.** Custom-tag lowering emits an overridable fixed `display:block` row. Fixed rows already precede authored attributes, and last binding wins, so an explicit `display:flex|grid|none` still overrides it ([contract-lower-tags.rs:63–71](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/contract-lower-tags.rs:63), [contract-lower-lib.rs:743–755,799–812](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/contract-lower-lib.rs:743)). Do not add a second shell-style authority: web glue assigns `op.css` before the element is inserted into the tree, so the plan row already prevents pre-upgrade divergence ([host-web-glue.js:230–247,325–327](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/host-web-glue.js:230)). The browser parity fixture must likewise use explicit `display:block`.

**A5 — Optional capability, but the fixture must implement it.** Each factory table has a `SNAPSHOT_PNG` capability bit and corresponding optional tokened function slot. Without it, ordinary platform-tree capture is authoritative. With it, the host requests PNG bytes through `ReplyFn` and composites exactly one source at the node’s position. This avoids burdening ordinary views while serving Metal/remote-layer modules: normal AppKit capture walks `cacheDisplay`, Metal alone would be blank, and WKWebView requires its own asynchronous snapshot path ([Presenter.swift:948–978](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/host-apple-macos-Presenter.swift:948), [LLP 1012:93–98](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/llp-1012-agent-api-v1.spec.md:93), [LLP 1020:240–253](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3eec2c5f-df46-4acc-a576-a02a20836a63/scratchpad/panel-1024/capsule/llp-1020-webview.rfc.md:240)). The fixture sets the bit so the callback/token/capture path is actually tested; Ghostty would set it too.

## B. Convergence

1. Confirm: use the size-versioned function-table shape, exposed through one `exact_native_abi()` entry; `create` receives `EventFn`, `ReplyFn`, and context.
2. Confirm the stated threading, copying, queuing, non-reentrancy, nonce, and stale-callback rules.
3. Confirm injected module script after the paint stamp; never `import()`.
4. Confirm initial creates queue and loading begins only at the established post-paint gate.
5. Confirm HTML potential-custom-element-name grammar: lowercase and reserved names refused.
6. Confirm live-swap is cut; define once, then page/process restart for module-code edits; no watcher, SSE ping, or `dlclose`.
7. Confirm `$EXACT_MODULES` is gone; one dev-only app-artifact override, bundle-only in release.
8. Confirm Linux is `{unavailable}` in v1.
9. Confirm web `op.tag` is the custom-element name; Apple remains `"native"`.
10. Confirm `load`/`message` unlock; `src`/`sandbox`/`surface` remain restricted; pin all event payloads and discriminants.
11. Confirm literals-only, canonical full-replacement JSON; known attrs and `renamed()` refusals win before leftovers.
12. Confirm `tree` reports `{name,state,error}` and failures are per-node and loud in `logs`.
13. Confirm module `type`/guest-`tap` is cut; reserve the null `agent_input` slot without implementing it.
14. Confirm the consumer/implementer landing condition, unchanged v1 bar, and N+1 correction to the no-core-edit claim.
15. Confirm the expanded fixture matrix, including post-paint proof, failure distinctions, restart-without-duplicate-define, and iOS signing traversal.

## C. Close

The converged v1 is: potential custom-element syntax lowers to a plan-visible, block-defaulted `NativeView`; the app’s generated factory roster catches unknown module names at bake; one fixed app-scoped artifact loads after paint and performs a validated in-memory tag lookup; its size-versioned function tables carry event/reply callbacks, explicit threading rules, platform-view bounds observation, and optional tokened snapshots; web uses the real custom-element tag, Apple uses its one native arm, Linux declares unavailability; module failures are per-node and inspectable; code edits restart rather than live-swap; and the fixture proves successful loading, capture, events, packaging, timing, and every named failure class.

I would not block the editorial fold if those positions are incorporated. I would block an implementation landing until a consumer and implementer are named, the ABI header and generated roster have one unambiguous authority, and the full fixture—including aggregate-artifact absence, missing factory, wrong ABI, rejected props, stale callbacks, and iOS signing—passes.
