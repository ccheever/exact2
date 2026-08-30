# Code review: the iframe node — LLP 1020 M1+M2 on web/macOS/iOS, 2026-08-30 (grok)

- **Family:** grok (xAI): `grok --prompt-file <brief> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --no-subagents --no-plan --output-format plain`, cwd = the read-only capsule; no repository access.
- **Method:** one-shot code review from the brief, mutually blind to the other family (codex gpt-5.6-sol xhigh, same capsule). The capsule: the brief, `CLAUDE.md`, `rules/`, LLP 1020, the full text of the 21 primary files, and the unified diff of the whole change.
- **Gates:** brief sha256 `1385976c96ff8d91002f8f778aa1c3ca68c542ab149beee8e1b95a0759846a04`; diff sha256 `e4519d2c2417e80456f7ce6878a547615ccf003f45aa2d2e17cb0827ef4f81ae`; capsule sha256 `7d43a9d2177da9fce61f6f87d34210579c7ddbed610f593d826c9cac56e8c6b8`. The code reviewed is `lane/webview` `ead5835` over main `17350d0`. No capsule file was modified while either reviewer was in flight.
- **Runs:** 2026-08-30T22:23:26Z → received by T22:39Z.
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (verbatim; the CLI's narration lines are preserved)

I'll review LLP 1020 against the capsule materials: rules, the RFC, the files under review, and the unified diff. Starting by mapping the capsule and reading the governing docs.The capsule is flattened. I'll read the brief, rules, RFC, and then the implementation files.Governing docs are in. Next I’ll read the implementation files and the unified diff in batches.The kernel/compiler tables look consistent so far. Next I’ll read the host, runner, agent, and Apple-arm files — that’s where routing and sandbox bugs tend to live.Compiler/ABI wiring looks consistent. Next I’ll inspect the web glue, Apple arm, presenters, and agent paths for sandbox and lifecycle issues.Apple-arm lifecycle and the wrapper script look like the highest-risk area. I’ll read the agent hosts, smoke, and the rest of the diff next.I’ll search the remaining diff for kernel, Linux, EventKind, and agent-tree wiring — those are the codec and parity risks.The kernel/compiler/ABI wiring is coherent; the remaining defects are in Apple lifecycle, local `src` materialization, agent honesty, and RFC trades this PR did not land. Checking a few line-accurate citations, then I’ll write the ranked review.The compiler/ABI/schema path is sound: `iframe` is a 300×150 `WebView` leaf in the tag table, `src`/`sandbox` are kernel props 42/43, `load`/`message` are appended `EventKind` ordinals, dispatch kinds 8/9 match on both ABIs, and the string payload is threaded the same way as `key`. Message routing on both arms is by source identity; `sandbox=""` would actually land as `sandbox=""` rather than fail open. What the green smokes did not pin down is the Apple local-`src` materialization, the content-world frame pointer, nested runloop during `tree`/`screenshot`, iOS screenshot compositing, and the RFC trades this PR never landed.

---

## HIGH

### H1. Apple agent bridge binds to *any* non-main frame (last writer wins)

`WebArm.swift:53–56` injects `exactAgent` `'ready'` with `forMainFrameOnly: false`. `WebArm.swift:209–212` then does:

```209:212:WebArm.swift
        if message.name == "exactAgent" {
            if !message.frameInfo.isMainFrame { guestFrame = message.frameInfo }
            return
```

Every nested frame in the guest overwrites `guestFrame`. `tree` / `tap` / `type` then `evaluateJavaScript` in that frame (`WebArm.swift:287–302`, `WebModule.swift:288–316`). A Castle deck with one child iframe silently drives the child, not the deck. The fixture has no nested frames (`deck-index.html`), so smoke cannot catch this.

The intended frame is the wrapper’s inner `#exact-frame` — the unique non-main frame whose parent is the main frame.

### H2. Local `src` is materialized by assigning `srcdoc` *after* an iframe that already has `src`

`WebArm.swift:122–146` emits:

```html
<iframe id="exact-frame" src="…" sandbox="…"></iframe>
<script>… inner.srcdoc = …</script>
```

The inner iframe is in the document with authored `src` before the script runs. For `/deck/index.html` that starts a navigation to `https://exact.invalid/deck/index.html` (the simulated wrapper origin cannot serve it), then `srcdoc` replaces it.

Consequences grounded in this code, even without a live trace of how many `load`s WebKit emits:

- `inner.addEventListener('load', …)` (`WebArm.swift:145`) will fire for whatever navigations actually complete. An abort/error load still fires `load` in HTML, which is kind `2` then kind `0` (`WebArm.swift:223–225`) — `loading=false` and the app `load` handler — **before** the real srcdoc document is ready.
- Process-death recovery sets `suppressLoad` for **one** inner load (`WebArm.swift:229–234`). A second inner load (srcdoc) would deliver a `load` event the RFC forbids for silent re-serve.
- The same src-then-srcdoc sequence is in `navigationScript()` (`WebArm.swift:167–170`).

**FLAG:** I did not run WebKit; smoke’s boolean `deckLoaded` and “message = deck-ready” still pass if an extra `load` happens. The markup order is the defect; the runtime count is unproven.

Fix that keeps authored `src` visible: put `srcdoc` on the element in the initial HTML (so `srcdoc` wins at parse and `src` is ignored), or omit `src` from the tag when inlining.

---

## MEDIUM

### M1. iOS screenshot compositing does not “whichever path has pixels wins”

`AgentIOS.swift:200–209` keeps the WKWebView visible and draws the arm snapshot in `NodeView.draw` (`Presenter-ios.swift:551–555`). UIKit paints `draw(_:)` first, then subviews. The live WKWebView is **on top** of the snapshot.

- If the live view is opaque and blank, it **covers** a good snapshot.
- If both have pixels and they disagree by a pixel, you get double-paint / ghosting.
- `Presenter-ios.swift:976–977` still says “the remote platform views are hidden” — that is the macOS path (`AgentMac.swift:170–177`), not iOS.

macOS is the honest one: hide the remote layer, compose only `takeSnapshot`. Smoke’s `cyan > 5` (`smoke.mjs` around the deck crop) only proves some #8fdcff/#aeb8d8 pixels exist in the crop. On iOS that can be `drawHierarchy` alone; it does not prove `takeSnapshot` contributed, and it would not catch a 1px misalignment.

### M2. `tree` / `tap` / `screenshot` spin the main runloop

`WebModule.swift:266–269`:

```266:269:WebModule.swift
        let deadline = Date(timeIntervalSinceNow: 5)
        while !wait.done && Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01))
        }
```

`tree()` calls this per iframe (`WebModule.swift:288`); `snapshots()` does it per live webview (`WebModule.swift:351–354`). Nested `.default` can deliver `load`/`message`, apply a batch, and destroy the node whose `handle` is in use. Completions are `[weak self]`, so this is more likely a timeout/wrong-tree than a crash — but it is not a closed agent turn. A `load` handler that navigates away mid-`tree` is in scope for a real deck, not for this fixture (`recordDeckLoad` only sets a bool).

### M3. Apple `tap` reports success when the guest script says `ok:false`

`WebModule.swift:308–318` returns `{ok:false}` when there is no target, then:

```317:318:WebModule.swift
        guard case .success = request(entry, script: script) else { return ["error": "guest tap failed"] }
        return ["tapped": Int(owner.id), "guest": true, "at": at]
```

Any completed eval is treated as a hit. `type` is slightly better (it copies `value` if present) but also ignores `ok`. Smoke’s tap hits `document.body` via the fallback, so it cannot catch this.

### M4. Linux RFC D5 is not in this change

RFC D5 requires the empty box plus `tree` `{unavailable: true}`. The unified diff has **no** Linux host file. Smoke skips deck steps on Linux (`smoke.mjs` `if (host === 'web' || apple)`). Workspace build still has to compile, so the box probably paints — the declared `unavailable` bit is not evidenced.

### M5. RFC §6 NOT-DOING trade is not in the PR

`rules-NOT-DOING.md:57` still lists `webview`. RFC §6 says that line comes off in the same PR that lands M1. `change.diff` does not touch that file. The rule is “one line naming what it unblocks, and take something off the doing-list in the same PR.”

### M6. Web host does not remount when `sandbox` changes

Apple re-serves on sandbox change (`WebArm.swift:89–96`) because HTML applies `sandbox` at navigation. Web glue only `setAttribute` (`glue.js:57`). A dynamic sandbox token change is spec-false on the oracle host. v1 apps are unlikely to change it; still a parity hole.

### M7. `loading=false` is a single bit with no generation

Kind `2` clears `loading` (`WebModule.swift:239`) with no match to `WebArm.generation`. An inner abort-load (H2) or a stale wrapper message that passed generation… generation is checked (`WebArm.swift:218`). Stale *loads from a previous inner navigation in the same generation* still clear the bit. `tree` then evaluates (`WebModule.swift:288`) against a document that is not the guest yet (`guestFrame` is also last-wins, H1).

---

## LOW

### L1. Presenter kind is the RN word `webview`

`host-apple-host.rs:626` maps `NodeType::WebView => "webview"`. The Contract tag is `iframe`; the kernel type is `WebView`. Internal, but it is exactly the name class D1 refuses. Agent `tree` still reports `WebView` from the runner — smoke checks that.

### L2. `load` / `message` are global handlers

`tags.rs:168–169` admit them on every tag. An `image load=` compiles and never fires (glue only listens on `HTMLIFrameElement`, `glue.js:125`). Same pattern as other handlers; worth an iframe-only check if you want the compiler to catch typos.

### L3. Event-name tables are duplicated

`load`/`message` are added in `format.json`, `tags.rs`, `contract/analyze` `HANDLERS`, `contract/types` refine, and both ABI comments. Pre-existing, not a second declaration of kernel props.

### L4. `localDocument` limitation is not a code constraint

The multi-file-subresource hole is in the brief, not next to `WebArm.swift:175–187`. Relative URLs inside a srcdoc document resolve against `https://exact.invalid/frame/<id>/index.html` or `about:srcdoc`, not the bundle. Hosted `https:` decks skip this path (`scheme == nil` guard) and are fine.

### L5. Path containment does not resolve symlinks

`WebArm.swift:183–185` uses `standardizedFileURL` + `hasPrefix(root + "/")`. Same shape as image `resolveSource`. A symlink inside the asset root that points out would pass. `..` itself is blocked.

### L6. File caps

Largest touched sources in the capsule: `runner.rs` 1468, `Presenter-ios.swift` 1052. Under 1500. WebKit stays in `WebArm.swift`; `WebModule.swift` does not import it.

---

## The two implementation decisions

**Bundle-relative `src` → `srcdoc` (`WebArm.swift:175–187`, `121–170`).**  
Containment is the right check (standardized path, slash-terminated prefix). Inlining via base64 + `TextDecoder` is injection-safe — far better than stuffing the file into an HTML attribute. `src`/`sandbox` stay on the real inner `<iframe>`, so this is not a sandbox fail-open. The limitation *is* confined to schemeless URLs; `https:` decks never enter `localDocument`. What is not honest: (1) H2’s parse order, (2) no in-code note that a multi-file bundle’s subresources will not resolve, (3) iOS simulator smoke sets `EXACT_ASSETS` to the checkout (`agent.mjs` `openIOS`), so it never proved the bundle copy `apple-build.mjs` adds. Device-without-`EXACT_ASSETS` is unproven.

**iOS screenshot layering.**  
The diagnosis (simulator `takeSnapshot` can drop text; `drawHierarchy` can see WKWebView) is plausible. The implementation does not do “whichever has pixels wins”; it does live-on-top (M1). macOS hide-and-compose is the path that actually matches D4’s capture handshake. Keep iOS live-on-top only if you *discard* a snapshot that would fight it, or hide the WKWebView like macOS and treat `drawHierarchy` as the iOS-only fallback when `takeSnapshot` is empty.

---

## RFC findings

1. **D5 is specified and not delivered** (M4). Either implement `{unavailable: true}` on Linux `tree` or narrow D5 to “smoke skips the deck; the box is empty.”
2. **§6 is specified and not delivered** (M5). The NOT-DOING trade is a rule, not a suggestion.
3. **§5 forbids author-facing `srcdoc`; the Apple arm uses `srcdoc` internally.** That is a valid implementation trick if the RFC says so. As written, a reader will think srcdoc does not exist in v1. Add one sentence: scheme-relative `src` may be inlined as the inner iframe’s `srcdoc`; it is not a Contract attribute.
4. **D1’s “synthesizes an error document on provisional failure” is a top-topology lesson.** `didFailProvisionalNavigation` is on the WKWebView (`WebArm.swift:253–263`). In frame topology, inner-iframe failures are the engine’s error document plus the inner `load` listener. Wrapper failure re-serves a wrapper whose *inner* iframe gets the error HTML as srcdoc (`wrapper(error:)`). The RFC text is slightly wrong for the topology it mandates; the code is closer to iframe semantics than the RFC is.

D4’s “guest time is outside `clock settle`” is implemented: glue `clock()` waits inflight fetches and animations, not iframe loads (`glue.js:452–467`); Apple `settle` is the motion engine. Smoke calls `clock('settle')` then **polls** for `deck-ready`, which is the honest shape — it does not prove settle *didn’t* wait, only that they did not rely on it.

---

## Security of the boundary

Sandbox pass-through on the path I can read is faithful:

| Path | What lands |
|---|---|
| Web | `setAttribute("sandbox", value)` / `removeAttribute` (`glue.js:44,57`) |
| Apple | `sandbox="…"` on a real inner `<iframe>` (`WebArm.swift:123,132`) |
| Empty string | `Some("")` → `sandbox=""` (restricts), not omitted |
| Absent | attribute omitted (unsandboxed) — spec-true |

`isMainFrame` on `webkit.messageHandlers.exact` (`WebArm.swift:214`) stops the guest frame from forging `{kind,generation,payload}` through the default-world handler. Payload narrowing (string or `JSON.stringify`, drop on failure) matches D2 on both arms (`glue.js:72–76`, `WebArm.swift:138–142`). The private world `exact.agent` plus `forMainFrameOnly: false` user script is invisible to page JS; that part of D4 holds.

With authored `allow-scripts allow-same-origin` (the fixture, `app.contract:117`), a srcdoc guest is same-origin with `https://exact.invalid` and can `parent.eval(...)` into the wrapper, which *would* pass `isMainFrame`. That is the sandbox the author granted, and it does not unsandbox the guest relative to the tokens; it only lets them forge extra `load`/`message` strings — data the app already treats as untrusted. Not a HIGH fail-open.

**FLAG (needs a live run):** empty `sandbox=""`, omitted sandbox, and a guest *without* `allow-scripts` were never driven. Smoke’s fixture is the dangerous pair on purpose (so `contentDocument` / outline works on web). Restriction-fidelity is therefore unproven.

The content-world bridge is not guest-visible. H1 is an *agent* targeting bug, not a guest-to-host privilege bug.

---

## What the runs did not prove

| Claim | Evidence gap |
|---|---|
| Sandbox is fail-closed | Fixture grants `allow-scripts allow-same-origin` |
| Process-death silent re-serve | No test; H2 can leak a `load` |
| `src` change / destroy-during-load / screen leave+reenter | Smoke is mount, message, tap, back |
| Dylib missing → empty box + log, no crash | Not in smoke |
| Linux `{unavailable:true}` | Deck steps skipped |
| `clock settle` ignores guest rAF | They poll after settle; a fast load would also pass |
| iOS `takeSnapshot` has text | `cyan > 5` can be live `drawHierarchy` |
| iOS *device* bundle `deck/` | Simulator `EXACT_ASSETS` is the checkout |
| Cross-origin guest on web is opaque; on Apple still outlined | Fixture is same-origin / srcdoc |
| Nested-iframe agent targeting | Fixture has one document |
| Kernel `WebView` is a leaf | `schema.json` has no leaf bit; `kernel/build.rs` is not in this change. **FLAG:** confirm `can_hold_children()` is false, matching Canvas |

Codec compatibility is the part I *would* trust from the text: new `EventKind` values are appended; new prop ids are appended; `SCHEMA_DIGEST` will change and old plans refuse (`runner.rs` boot checks it). That matches “no compat before 1.0.”

---

## Fold (the orchestrator, 2026-08-30, commit 2a586b8)

Every finding was verified against the live tree before folding; all held. Applied in
`2a586b8` (codex gpt-5.6-sol xhigh executed the fold from a per-finding brief):

- **sol HIGH 1 (teardown UAF + arm leak)** — confirmed by reading: the handler
  retain cycle keeps the arm alive past `destroy` while its callback box is freed.
  Fixed: `invalidate()` (stop loading, remove both handlers, delegate nil, an
  `invalidated` gate on emit/reply) before release; destroy order corrected.
- **grok H1 / sol M2 (guestFrame last-writer)** — fixed: the ready script posts only
  from the wrapper's direct child (`parent === top && self !== top`), first-wins.
- **grok H2 / sol M1 / grok M7 (src-then-srcdoc; generation races)** — fixed: every
  `src` change re-serves the wrapper (navigationScript deleted); a local fixture's
  inner iframe is srcdoc-only, no `src` attribute, so no doomed navigation exists.
- **sol M6 (wrapper self-reload bypass)** — fixed: `serving`-gated navigation policy;
  anything not host-initiated cancels and re-serves.
- **grok M1 / sol M5 (iOS double-composite)** — applied as reviewed, then
  **re-measured and adjusted**: the single-source simulator arrangement regressed to
  0 guest pixels — `takeSnapshot` is also the flush that makes `drawHierarchy`
  rasterize the remote layer. Final: simulator = snapshot composed as underlay
  beneath the visible live render (both halves measured necessary); device = macOS's
  hide-and-compose, single source. The RFC's D4 note records the split.
- **grok M3 / sol M3 (agent honesty)** — fixed: `ok:false` is an error on Apple; the
  web arm implements selector/coordinate guest tap and guest type, with a stable
  cross-origin error, mirroring the Apple guest script.
- **grok M6 (web sandbox change)** — fixed: re-navigation applies the new tokens,
  matching D2's re-serve.
- **sol M7 / grok L2 (global attrs)** — fixed: `lower-attr-tag` rejection + corpus.
- **sol M4 / grok L4/L5 (symlinks; srcdoc limit)** — fixed: symlink-resolved
  containment; the single-file limitation is now an in-code constraint comment.
- **grok L1 (kind name)** — fixed: `iframe`.
- **grok M4 (Linux D5)** — fixed: the Linux carrier's `tree` marks a WebView
  `{unavailable: true}`.
- **grok FLAG (leaf)** — confirmed already-false by omission; asserted in the kernel
  node tests.
- **sol LOW 1 / grok M5 (the NOT-DOING trade)** — applied by the orchestrator:
  `webview` off §Components with LLP 1020 §6's line and take; QUEUE §5 reordered.
- **grok RFC 3/4** — the RFC is amended as built (srcdoc materialization declared
  with its multi-file limit; the error document restated for frame topology).

Verification after the fold: the five checks, the web smoke, the iOS-simulator
smoke, and the Linux smoke are green. The macOS smoke's canvas steps fail on this
machine with the display asleep/locked (the LLP 1008 §8 occlusion trap, reproduced
3/3 with the same signature at both the lane and its base) — the macOS rerun is owed
the moment a display is on; the pre-fold macOS smoke, deck steps included, was green.
Open evidence gaps grok's table names (device `EXACT_ASSETS`, restriction-fidelity
under a locked-down sandbox, destroy-mid-load under WebKit) stay open as test debt,
recorded here rather than papered over.
