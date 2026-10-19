# Code review: LLP 1121 stage 9 G1, a look snaps on the web, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 4: high), sandbox read-only, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `eb3363e8815651a658bd42c5e74b130e21e5cacf8004729f0a4967da271832be`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: click-then-key missed, the JS target never loaded the glue for a look, a delay-only transition skipped; cache, shadow focus, test timing).
- **Disposition:** fixed in round 2's commit.

---

**Blocking issues remain: 1–3.**

1. **blocking — [host/web/input-glue.js:476](/tmp/rv-g1-2/host/web/input-glue.js:476): click-then-key still misses the snap.**  
   Blink and WebKit update `:focus-visible` **before dispatching `keydown`**. Consequently, `was` is already true on the first key, and line 477 returns without scheduling anything. The focus look still transitions. ([Blink dispatch](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/input/keyboard_event_manager.cc), [WebKit dispatch](https://raw.githubusercontent.com/WebKit/WebKit/main/Source/WebCore/page/EventHandler.cpp))  
   **Fix:** compare against previously recorded focus-visible state, handling both immediate and deferred changes. Record the new state so held-key repeats do not snap repeatedly. Add a real click-then-key regression test; `.focus({focusVisible:true})` does not exercise this path.

2. **blocking — [host/web-js/src/emit.rs:984](/tmp/rv-g1-2/host/web-js/src/emit.rs:984): looks do not activate the newly required input glue.**  
   A JS-target page containing a hover/focus look and a transition, but no press feedback, navigation, shortcut or pan, never calls `input()`. Its looks therefore continue easing indefinitely. The new fixture hides this integration gap by calling `createInputHandlers()` explicitly.  
   **Fix:** request the input piece when emitting looks, or when `h()` encounters `data-exact-looks`. Test a generated page whose only input-glue dependency is its look.

3. **blocking — [host/web/input-glue.js:457](/tmp/rv-g1-2/host/web/input-glue.js:457): zero-duration transitions with positive delay are skipped.**  
   For `transition: opacity 0s linear 300ms`, `moving()` returns false, leaving both look edges delayed by 300 ms. CSS starts transitions using **duration plus delay**; the repository already implements that rule in `motion/src/transition.rs:178`. ([CSS transition-start rules](https://drafts.csswg.org/css-transitions/#starting))  
   **Fix:** inspect the matched duration/delay lists and effective transition properties, including positive-delay, zero-duration transitions. Add entering/leaving coverage.

4. **should-fix — [host/web/input-glue.js:442](/tmp/rv-g1-2/host/web/input-glue.js:442): positive selector-cache entries never invalidate.**  
   After `"1"` is cached as hover-only, replacing its selectors or adding another stylesheet containing its focus-visible rule leaves focus snapping disabled until reload. Editing declaration values alone is safe; changing the available states is not.  
   **Fix:** invalidate on stylesheet generations, including CSSOM updates, or emit authoritative state metadata alongside the look identifier. Test both selector replacement and a later sheet after a positive cache hit.

5. **should-fix — [host/web/input-glue.js:460](/tmp/rv-g1-2/host/web/input-glue.js:460): shadow-host focus edges remain unheard.**  
   A focusable host with a focus look—such as a header containing the injected shadow Back button—can transfer focus into that button. Retargeting suppresses the outer focus events when target and related target become the same host; the host’s focus-visible look can disappear through its transition. `document.activeElement` also reports the host. ([DOM dispatch rules](https://dom.spec.whatwg.org/#concept-event-dispatch))  
   **Fix:** observe focus within owned shadow roots and track the actual focused element and host state. Add a host-to-shadow-child regression; otherwise explicitly include this limitation in §10.

6. **should-fix — [host/web/tests/looks.test.mjs:110](/tmp/rv-g1-2/host/web/tests/looks.test.mjs:110): the animation assertions depend on wall-clock scheduling.**  
   The 100 ms sleep followed by another browser round trip can sample after the 400 ms transition finishes, failing correct code under load. Conversely, delayed snap assertions can observe an ordinary transition already completed and pass broken code.  
   **Fix:** control animation time, inspect transition existence immediately after the edge, and seek a paused base transition to a known intermediate time.

7. **nit — [host/web/input-glue.js:431](/tmp/rv-g1-2/host/web/input-glue.js:431): “one style read for all” excludes the candidate reads.**  
   With three eligible ancestors, the implementation performs four computed-style reads per event; the pointer/mouse pair performs eight. The reads precede writes, so this does **not** imply eight independent layout flushes. Ordinary duplicate callbacks appear to add work without a separate visual failure.  
   **Fix:** correct the cost claim and deduplicate compatibility events if measurement warrants it, preserving Firefox’s mouse-only path.

The injected sheet is created once before press observation starts; subsequent snaps do not mutate it. Empty selector results are **not cached**, and inaccessible cross-origin sheets do not abort scanning other sheets. rAF precedes normal style/layout, and `stillThrough()` explicitly forces calculation, so that ordering itself is not the keyboard bug. ([Rendering order](https://html.spec.whatwg.org/multipage/webappapis.html#update-the-rendering))

I ran in-memory probes against the extracted implementation. I did not run the browser suite because it creates filesystem artifacts. No files changed.
