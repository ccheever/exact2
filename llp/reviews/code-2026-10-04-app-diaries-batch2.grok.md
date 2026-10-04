# Code review: the app-diary fixes, batch 2 (gaps/batch2), 2026-10-04 (grok)

- **Family:** xAI — `grok-4.7` via `grok -p`, reasoning effort xhigh, told to modify nothing (the tree was clean after each run).
- **Method:** three runs — A (language and input), B (runtime and hosts), each blind to Astra's; then a delta round over the twelve fix commits. The author (Claude, Opus 5.5, orchestrating) is not a reviewer.
- **Transcription:** each run's final message, unedited (progress narration before the findings dropped).
- **Verdicts:** A LAND WITH FIXES · B LAND WITH FIXES · delta LAND WITH FIXES.
- **Disposition:** A and B's six findings fixed with tests (ece9c6143, bdd4c030e, 9d803896d, fde2b0c76, b18b7fbd5, 6d9ff6204); the delta's three should-fixes fixed with tests (e98d3ce9f, 4972f7429, a4748def4).

---

## b2.grokA

## Must-fix

**The flex shorthand skips both unbounded checks.** `contract/lower/src/collection.rs:65` and `contract/lower/src/lib.rs:812`.

`numeric_literal` only matches `Expr::Number`, and `is_none_or` is true for every string. So `flex="none"` (grow 0, shrink 0, basis auto), `flex="0"` (same longhands as numeric `flex=0`: grow 0, shrink 1, basis `0%`), and `flex="initial"` all count as a vertical bound. The scroll check is weaker: any attribute named `flex` suppresses `lower-scroll-unbounded`. `shrinking_scroll` (`contract/lower/src/values.rs:875`) only looks at a `flex-shrink` longhand, so it does not correct this.

Failure: `list virtualized=true flex="none"` with no height compiles. An empty list also passes the bake lint (`extent > 0` is false), then grows with its rows and never windows. `scroll flex="none"` in a column is the same hole: the box grows with its content and never scrolls. Numeric `flex=0` is refused with `lower-collection-unbounded`, as `collection_bounds.rs` requires.

Fix: expand the shorthand before both checks. A positive grow, or a definite basis (`px`, `%`, `0px`), is a bound. `none`, `initial`, and a 0 grow with basis `auto` are not. For the scroll exception, treat the shorthand's shrink the same way as the `flex-shrink` longhand: shrink greater than 0 plus `min-height: 0` under a bounded column is a bound; `flex="none"` is not.

## Should-fix

**Option changes `KeyboardEvent.key` on the web and not on Apple.** `host/apple/Sources/ExactKit/KeyEvents.swift:108` (iOS at line 123).

`keyName` uses `charactersIgnoringModifiers`, which keeps Shift and drops Option. The web hosts send the browser's `event.key` (`host/web/glue.js:34`, `host/web-js/rt.js` key handler). On a US Mac, Chrome reports Option+A as key `"å"` with `altKey` true. AppKit reports key `"a"` with `altKey` true. Meta, Ctrl, and Shift chords match; this one does not. A handler written against Chrome (`key == "å"`, or `aria-keyshortcuts` compared to `event.key`) misses on macOS and iOS.

Fix: when Option is down and `characters` is a non-control character, use that character. Keep `charactersIgnoringModifiers` for Ctrl, where Chrome's `key` stays `"a"` and `characters` is a control character.

**macOS hover `pointermove` is not one per frame.** `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:148`.

`pointerHovered` calls `presenter.pointer` on every `mouseMoved`. Drags are coalesced (`pointerDragged` at line 137). The web host keeps only the latest move per animation frame (`host/web/input-glue.js:156`). LLP 1056 says one dispatch a frame. A fast mouse delivers many `mouseMoved` events per frame, so a `pointermove` action commits repeatedly on macOS while the web host commits once. Linux delivering every move is the LLP's stated exception.

Fix: coalesce hover the way `input-glue.js` does, and flush the pending sample before a down or an up.

## Checked, no defect

LLP 1086 stage 1 matches the as-built notes. D4 refuses resources, derives, mutations, actions, self, and later state in an initializer. D5 reserves the sixteen words only at binders. D7.1's optional `KeyboardEvent` and `PointerEvent` counts match `Event::record` and `selection.rs`. D7.2 leaves `tabindex` for stage 2. D8's sequential `if k == "…"` widening holds; the missing `else` negation only adds false refusals.

`flex: 1` lowers to `1 1 0%`, matching Chrome. Border omitted parts reset to medium / none / currentcolor; `thin`/`medium`/`thick` are 1/3/5px; `1pt` is 4/3px. `overflow: auto` is distinct from `scroll`, and a visible axis beside a non-visible one becomes a scrollport. Viewport units pass through to CSS on the web. Cursor is in the inherited style mask Apple sends to child views. `offsetX`/`offsetY` are the content box on every host, which LLP 1056 declares on purpose. Textarea `submit` is refused in the compiler and skipped on the hosts. External `http(s)` and scheme-relative links open in a new context unless `target` is set.

Verdict: LAND WITH FIXES


---

## b2.grokB

## Must-fix

**`js/src/prelude.js:757`** (claim loop at 758–760, reply at 763–769; the request is recorded in `js/src/lib.rs:256` and removed only by `take_request` at `js/src/lib.rs:798`)

`moduleWide` is true only in the Hermes engine (`bytesDoor` is `__exact_bytes`, installed in `js/src/shim.cc`). On a finished answer, an unclaimed fetch is handed to the host only when no other call is still `pending`. If any other call is pending, `settle` replies and leaves the ticket in `pending` and the request in `host.requests`. The later scan that would claim it (lines 771–774) runs only for an answer whose status is still `pending`. A call parked on its own fetch usually becomes `done` in the fulfill drain before that scan, so it replies without taking the orphan.

Failure: a list answer is awaiting its own fetch. A save answer does `fetch(POST)` and returns without awaiting it. The save reply is applied and the UI shows success. The POST stays in `host.requests` and is never `take_request`'d. When the list fetch completes, that answer is already `done` and does not pick the POST up. Nothing is sent. The web module realm keeps the per-answer rule (`moduleWide` is false) and claims the fetch; the JS target uses the browser's `fetch`, which starts immediately.

Fix: a ticket created while `call.status === "pending"` is this answer's own fetch (the body, or its own `await` resume). Claim those even when `others` is set, the same way `call.storage > 0` is not gated. Leave only a ticket created after this call's `result.then` has set `status` to `done`: on the answer promise that handler is registered before any later answer can await it, which is the castle r4a shape (`a_fetch_made_after_awaiting_another_answers_fetch_is_not_left_behind`). Those stay unclaimed so the waiting answer can take them and the finished answer can still reply.

## Should-fix

**`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1284`**

`Capture.draw` composites sublayers on top of `draw(_:)` (`host/apple/Sources/ExactKit/BoxShadow.swift:354`). Fills are hidden for that turn (`hideBoxFills`). `imageLayer` is not. The capture commit changed the image predicate to `!(layerBoxEligible && !Capture.capturing && imageLayer != nil)`, so while `Capture.capturing` is set, `draw(_:)` paints the bitmap even when `imageLayer` is in the tree.

Failure: an agent screenshot of an image that took the layer path (no tint, object-fit the layer can express) with any transparent pixels. The same CGImage is composited twice with source-over, so a partial alpha comes out darker. An opaque JPEG looks unchanged.

Fix: for the capture, hide `imageLayer` the way inset-shadow fills are hidden, and keep painting the bitmap in `draw(_:)` so the shot stays sRGB. Restore the layer afterward.

**`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:679`** and **`host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:755`**

`pointer-events: none` is applied only in `ordinary()`. A canvas that has placements never uses that path for a miss: after the HUD and the placed children miss, both hit-tests `return self`.

Failure: a placement canvas styled `pointer-events: none` still becomes the hit for a point that misses every placed child, so the click does not reach the view under the canvas. A descendant that sets `auto` is unaffected; this is the canvas box itself.

Fix: if this node's `pointer_events` is `none`, that final return is nil, on both platforms.

Storage let-go, refusal codes on the filesystem path, launch-URL storage keys, app roots before the first frame, the 1 MiB `data:` bound, text press, visible-overflow hits on node children, and SVG `light-dark()` reapply match the code paths I traced. I did not execute the integration suites.

Verdict: LAND WITH FIXES


---

## b2-delta.grok

## Findings

- **Astra 1 — callable names mistaken for state references.** Fixed. An initializer call is flagged only after the record, file-function, and action rules miss, so `length("abc")` and a file `fn` that shares a state name compile, and `state n = go()` still reports `type-initializer-scope`.
- **Astra 2 — stored photos keep the previous symbol.** Fixed. `unsymbol` clears the symbol attributes and cached symbol state before the asynchronous `app:/` path.
- **Astra 3 — `_blank` links with `press` need a router.** Fixed. A browser-owned click (modified, non-`_self` target, or `download`) returns from the press handler without `preventDefault`.
- **Astra 4 — down/up-only children swallow ancestor `pointermove` on wasm.** Fixed. `exactPointerMover` is set only when that node handles `pointermove`.
- **Astra 5 — Linux reveal overlooks `overflow: auto`.** Fixed. The container check and both axis checks accept `Overflow::Scroll` and `Overflow::Auto`.
- **Grok A — flex shorthand skips both unbounded checks.** Partly. `none`, `"0"`, and `initial` are no longer a vertical bound, and the shorthand's shrink feeds the scroll exception; a definite `0px` basis is still refused.
- **Grok A — Option changes `KeyboardEvent.key` on the web and not on Apple.** Fixed. With Option down, a single non-control character is the key; Ctrl stays on `charactersIgnoringModifiers`.
- **Grok A — macOS hover `pointermove` is not one per frame.** Fixed. One pending sample is sent on the display link, and a down or an up flushes it first.
- **Grok B — an unawaited Hermes fetch is dropped while another answer is pending.** Fixed. A ticket opened while `call.status === "pending"` is claimed even when another call is in flight; a ticket opened after `done` stays unclaimed for the waiter.
- **Grok B — macOS capture paints a layer-path image twice.** Fixed. Capture hides `imageLayer` and paints the bitmap once in `draw(_:)`, then restores the layer.
- **Grok B — a placement canvas with `pointer-events: none` is its own hit.** Fixed. After placed children miss, macOS and iOS return nil when this node's `pointer_events` is `none`.

## New defects

- **Should-fix** — `contract/lower/src/values.rs:904`. `flex_bounds` treats every zero basis as unbounded, including the length `0px`. `list virtualized=true flex="0 0 0px"` and `scroll flex="0 0 0px"` fail `lower-collection-unbounded` and `lower-scroll-unbounded`. A scrollport's automatic minimum is 0, so `flex: 0 0 0px` is a definite empty scrollport, the same shape as `height=0`, which these checks accept. `flex="0"` stays unbounded because its basis is `0%`. `contract/cli/tests/it/collection_bounds.rs:189` locks the refusal in. Treat a zero length (`0`, `0px`) as a bound, and keep `0%`, `auto`, and the content keywords unbounded when grow is 0.
- **Should-fix** — `host/apple/Sources/ExactKit/Mac/PresenterMac.swift:137`. `deinit` invalidates `pumpLink` only. Releasing a presenter that still has a hover sample leaves the run loop holding that `CADisplayLink`, and the link keeps its `hoverTarget`. The tick's weak presenter is nil, so nothing invalidates the link and it fires for the rest of the process. Invalidate `hoverLink` in `deinit`, and invalidate it inside the tick when the presenter is gone.
- **Should-fix** — `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:196`. One `hoverMove` slot serves the whole presenter. A pointer that crosses two `pointermove` nodes in the same frame keeps only the second sample, so the first node's last move is never delivered. Keep the latest sample per view and flush each of them.

Verdict: LAND WITH FIXES

