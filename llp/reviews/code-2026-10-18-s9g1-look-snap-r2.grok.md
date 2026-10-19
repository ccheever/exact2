# Code review: LLP 1121 stage 9 G1, a look snaps on the web, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `eb3363e8815651a658bd42c5e74b130e21e5cacf8004729f0a4967da271832be`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (pointer focus snapping, shadow focus, style read before the attribute, media query and disabled, tests).
- **Disposition:** fixed in round 2's commit.

---

There is one blocking finding. The keydown path that is supposed to snap a focus look never holds the node still in Chromium, so that look eases.

1. **Blocking.** `host/web/input-glue.js:475-478` (the flush that makes this miss is `host/web/input-glue.js:127` and `:149`, in the capture listener at `:120-161`).

   Chromium's `UpdateHadKeyboardEvent` runs before dispatch. On a keydown with no ctrl/alt/meta, if the document had not yet had a keyboard event, it sets that flag and dirties `:focus-visible` on the focused node. It does not recalculate style yet. The frame's own style pass is after `requestAnimationFrame` callbacks, but any `matches()`, `getComputedStyle`, or `getClientRects` before that forces the recalc immediately.

   This file forces it before the snap listener runs. The document capture `keydown` listener is registered first, so it runs first. For an ordinary key it reaches `composedPath().some(... el.matches(...))` at line 127 and `document.activeElement.matches(...)` at line 149. That flush applies the new `:focus-visible` rule while the authored `transition` is still in force, and the look starts easing. The snap listener then does `was = el.matches(":focus-visible")`, sees `was` true, and returns. The `requestAnimationFrame` is never scheduled, so `stillThrough` never runs.

   A held key cannot recover. After the first keydown the flag is already set, so later repeats leave `:focus-visible` alone, `was` stays true, and they return the same way.

   Scenario: click a node that has a focus look and `transition: background-color 400ms` (the click clears the keyboard-event flag), then press a key that does not move focus — a letter, Shift, or Space on something that is not a button. The focus look eases for the full 400ms. Tab is a different path: its focus move happens in the default action and is heard by `focusin`/`focusout`.

   Fix: on a non-repeat, non-modifier, non-composing keydown, set `data-exact-snap` on the focused node before any listener reads style. That means the first lines of the document capture listener at line 120, ahead of line 127. Track "already showing `:focus-visible`" in a `WeakSet` updated when a snap or `focusin` confirms it, so the decision does not itself read style. Let the key's flush happen under `transition: none`, then remove the attribute. Drop the `requestAnimationFrame`. Walk `shadowRoot.activeElement` to the deep focused node (finding 3).

2. **Should-fix.** `host/web/input-glue.js:460-463`.

   `focusin` and `focusout` snap any node whose look-rule text contains `focus-visible` and whose computed `transition-duration` is non-zero. They never check that `:focus-visible` turned on or off. A pointer click focuses the node with `:focus-visible` left off, and a later blur of that pointer focus leaves it off. Both still call `stillThrough`, which sets `transition: none` for one flush and cancels a transition that was already running on that node. That is outside the accepted edge deviation: the look did not change, and a base change on that node is supposed to keep easing.

   Fix: keep a `WeakSet` of nodes currently showing `:focus-visible`. On a keyboard `focusin`, set `data-exact-snap` before any style read, flush, then remove it and add the node to the set if it matches. On a pointer `focusin`, skip the node so a running base transition survives. On `focusout`, snap only a node the set says was showing, again with the attribute set before any read. `arrive` at line 565 already passes `focusVisible: by.key`, so a keyboard route arrival follows a real key and a pointer arrival passes false.

3. **Should-fix.** `host/web/input-glue.js:462` and `:476`; the hover walk at `:465`.

   For a listener on `root`, a focus move inside an open shadow root retargets `focusin`/`focusout`'s `target` to the shadow host, and `document.activeElement` is that host. The host matches `:focus-within`. `:focus-visible` is on the inner node. `root.contains` and `parentElement` stop at the host, so the inner node is never a candidate. A focus look on a node inside a shadow root never snaps. The same retarget snaps the host whenever the host's own rule text contains `focus-visible` and it has a non-zero transition, including when Tab moves into the shadow and the host's look does not change.

   The web host's own shadow is the route header's Back button (`host/web/navigation.js:172-174`, style at `:149`). That button's `:focus-visible` outline is not an exact look and sets no `transition`. An exact focus look inside any shadow, and a focus look on a header that is itself the shadow host, still hit this.

   Fix: take the deep node from `composedPath()[0]` or by walking `shadowRoot.activeElement`, and walk ancestors through `getRootNode().host`. Snap that node only when `:focus-visible` actually flips (finding 2). Hover can keep starting at the retargeted host: `:hover` applies to the host.

4. **Should-fix.** `host/web/input-glue.js:457`, called at `:463`, `:467`, and again inside the frame callback at `:478`.

   `moving()` calls `getComputedStyle` on each candidate before `data-exact-snap` is set. The state has already flipped when the event is dispatched, so that read is the first style recalc under the authored transition and starts it. `stillThrough` then tries to cancel it. One `getComputedStyle` in `stillThrough` does flush every snapped node, but only after these per-node reads. A mouse edge pays that twice (finding 7). The same order is on the focus path.

   Chrome often jumps a transition cancelled in the same turn to its end value, which is what `looks.test.mjs` asserts for hover and for `focus({focusVisible: true})`. That cancel-after-start was not verified in Safari.

   Fix: collect the edge nodes without reading computed style, set `data-exact-snap` on all of them, do one `getComputedStyle`, then remove the attribute. A zero-duration node snaps as a no-op, and the press observer does not watch this attribute, so the duration filter is what forces the early read.

5. **Should-fix.** `host/web/input-glue.js:467`.

   Hover snapping treats selector text containing `:hover` as an edge. `looks.rs` wraps that rule in `@media (hover: hover)` (`host/web/src/host/looks.rs:44-46`), and the selector also excludes `:disabled` and `[disabled]`. The text still contains `:hover` when the media matches false and when the node is disabled, so `pointerover` still runs `stillThrough` and cancels a transition already running on that node. The held rule `[data-exact-held~=hover]` does not contain the substring `:hover`; the hit is the media-wrapped rule.

   Fix: require `matchMedia("(hover: hover)").matches`, and skip a node matching `:disabled, [disabled]:not([disabled="false"])`, the same exclusion `looks.rs` writes.

6. **Should-fix.** `host/web/tests/looks.test.mjs:102-107` (the snap case is `:73-117`).

   The focus assertion calls `element.focus({ focusVisible: true })`, which fires `focusin`. It never clicks and then presses a key, so finding 1 stays green. Also absent: hovering the focus-only `#focused` and pointer-focusing it (findings 2 and 5), a focus move between ancestor and descendant, a press paint staying put across a snap, and a focus look inside a shadow root. The file runs in Chromium only, so Firefox's mouse-only scroll and Safari's cancel-after-start are untested. I did not run this test.

   Fix: click `#focused`, dispatch a non-navigation key, and expect the focus color at its end value on the next frame. Add the negative cases next to the existing child-move case.

7. **Nit.** `host/web/input-glue.js:472`.

   A real mouse move fires `pointerover`/`pointerout` and `mouseover`/`mouseout`. The second pass walks and flushes the same edge again. If the first `stillThrough` already committed the look, the second pass is duplicate work, not a second transition. Firefox scroll under a still pointer sends only the mouse pair, so those listeners have to stay.

   Fix: if a `pointerover`/`pointerout` in this turn already handled the same target and `relatedTarget`, ignore the matching mouse event.

8. **Nit.** `host/web/input-glue.js:442-447`.

   A non-empty selector string is cached for the life of the page. An empty walk is not cached (`:446` returns before `set`), so a look stylesheet that arrives later is picked up when the first walk found nothing. Wasm rules are read from `exactRules` on every call (`:439`). Token edits rewrite declarations, not selectors, so they stay valid. A cross-origin sheet is skipped per sheet (`:445`). One cross-origin `@import` throws inside `walk` and drops the rest of that sheet, including a later look rule in the same sheet.

   Current emitters do not hit this: the JS target emits one static sheet and a new look set gets a new `data-exact-looks` index, and the looks sheet has no `@import`. Fix when a live selector edit of an existing index becomes real: drop the cache entry when `document.styleSheets` changes, and catch per rule so one import cannot abandon later rules in that sheet.

The press-paint restart from round 1 is fixed. `#exact-snap` is appended at `:436`, while the head observer connects only once a press paint exists (`:283-285`), and the per-node observer watches `style` and `class` only (`:282`). Toggling `data-exact-snap` does not notify it. Light-DOM ancestor and descendant focus moves snap the node that gained or lost focus. The accepted deviation (any running transition on the node ends at a real look edge) and the §10 list (layout or insert under a still pointer, WebKit's hover timer, sticky touch hover after pointerup) are unchanged.
