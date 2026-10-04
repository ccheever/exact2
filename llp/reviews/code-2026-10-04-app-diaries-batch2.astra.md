# Code review: the app-diary fixes, batch 2 (gaps/batch2), 2026-10-04 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox.
- **Method:** one round on the integration (merge resolutions, the unified optional-record rule, schema ids, cross-cutting behavior), mutually blind to the other family's two reviews; no second Astra round (codex budget), the fixes were delta-reviewed by Grok. The author (Claude, Opus 5.5, orchestrating) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all five fixed with tests (c3e884e2b, e4f57df62, 6043e345f, 9495fd442, 0af8bfc9c); the Grok delta round confirmed each.

---

1. **Must-fix — callable names mistaken for state references.** [component.rs:918](/Users/ccheever/projects/exact2-wt-batch2/contract/types/src/component.rs:918)  
   `state length = length("abc")` now fails with `type-initializer-scope`; changing the state name to `n` compiles. File-level functions sharing a state’s name also fail. The new initializer check treats call targets as variable reads, bypassing the resolver that `origin/main` used. **Fix:** resolve call targets through the existing function/shape/stdlib rules before diagnosing an initializer dependency.

2. **Must-fix — stored photos retain their previous symbol rendering.** [symbols.js:28](/Users/ccheever/projects/exact2-wt-batch2/host/web-js/symbols.js:28)  
   An image initially showing `symbol:photo`, then changing to `app:/data/photo.png`, returns before clearing its symbol attributes. Executing the hook and refresh with a stub file resolver confirmed that the blob URL lands, then the next refresh overwrites it with the symbol placeholder. **Fix:** clear symbol attributes and cached symbol state before entering the asynchronous `app:/` path.

3. **Must-fix — `_blank` links with `press` depend on having a router.** [rt.js:777](/Users/ccheever/projects/exact2-wt-batch2/host/web-js/rt.js:777)  
   In a JS-target app without routes, `link href="https://example.com" target="_blank" press=clicked` compiles but its click handler unconditionally prevents navigation. The router normally intercepts this combination, but isn’t installed here. The isolated listener probe produced `presses: 1, defaultPrevented: true`. **Fix:** honor the target’s browser-owned navigation directly in the press handler, independently of router installation.

4. **Should-fix — down/up-only children swallow ancestor pointer movement on wasm.** [input-glue.js:173](/Users/ccheever/projects/exact2-wt-batch2/host/web/input-glue.js:173)  
   With an ancestor handling `pointermove` and a child handling only `pointerdown`, hovering over the child delivers no move on wasm. The child marks `exactPointerMover` before checking whether it handles movement. The same in-memory event sequence delivered the ancestor’s record on the JS target. **Fix:** check handler eligibility before claiming the movement event.

5. **Should-fix — Linux reveal overlooks `overflow: auto`.** [reveal.rs:38](/Users/ccheever/projects/exact2-wt-batch2/host/linux/src/presenter/reveal.rs:38)  
   A target below the visible portion of a fixed-height `overflow="auto"` container remains clipped when the agent requests reveal. Wheel scrolling supports the new enum, but reveal’s container and axis checks recognize only `Overflow::Scroll`. **Fix:** include `Overflow::Auto` in all three checks and exercise reveal inside an auto scroller.

Validation: all 31 in-repo apps compile; prebuilt runner/kernel and focused integration tests passed, apart from a temporary-file test blocked by permissions. Caps and boot passed. The independently recomputed schema digest matches `0xdf2d2cd21f815378`, with unique prop IDs and `target=228`. Browser/native GUI verification was unavailable under the sandbox. No files changed.

Verdict: LAND WITH FIXES