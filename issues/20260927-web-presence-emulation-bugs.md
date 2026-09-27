# The web's layout-transition emulation diverges from native: resize animates, springs read wall time, paint freezes, shrinking clips early, same-name exits never end

**Status:** Fixed: items 1–4 and 6 now snap resize, use animation time, refresh paint/anchors and lift shrinking clips; Chrome regressions and agent drives pass. Item 5 did not reproduce in Chrome 154. Supplemental web smoke reports Chrome process diagnostics.
**Systems:** Web host (`host/web/presence-glue.js`, `host/web/glue.js`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1063 D6; design question: View Transitions / `@starting-style` instead of the emulation

Each item is confirmed by reading unless marked.

1. **A resize animates layout on the web.** `exact_resize` goes through the ordinary batch (`host/web/glue.js:307`), and `presence.after` animates every moved box. Apple sets `presence.snap` (`host/apple/src/host.rs:913`), and Linux snaps too. LLP 1063 D6 says a resize takes new boxes with no animation on every host.
2. **Interrupted springs read the wall clock.** `going()` computes the leftover offset and velocity from `document.timeline.currentTime` (`presence-glue.js:125-129`). The agent's `seek` pauses animations and sets their `currentTime` (`glue.js:1089-1098`). After `clock +N`, re-aiming a spring depends on how much real time passed, not on the frame on screen.
3. **The stand-in freezes the box's paint.** Background, border, shadow, opacity and transform are copied once into a sibling surface (`presence-glue.js:152-163`), and the element's own paint is held transparent (`:186`). A card that grows while also changing colour, fading or being pressed shows frozen paint and then snaps. Apple re-reads the presented paint every frame (`Surface.swift:94`).
4. **A shrinking box that clips cuts its children early (P3).** The element's own overflow clip is already at the final size, and the inset is clamped to 0 while shrinking (`presence-glue.js:195`). Apple lifts the clip during the move (`Surface.swift:140`).
5. **An exit reusing a running animation's name never ends (plausible, high).** CSS matches duplicate names from the end of the new list, so with `pulse 1s infinite` running and an exit `pulse 200ms`, the exit matches the old infinite animation. `Promise.all` then never resolves and the ghost stays (`presence-glue.js:240-245`). Native appends a fresh play (`motion/src/engine/animate.rs:216`). Chrome's behaviour has not been driven.
6. **The stand-in's anchor is skipped by reorder (plausible, weak).** The anchor carries `data-exiting`, which child reconciliation skips, so it can sit at a stale index until the next move rebuilds it.

**Fix:** items 1–4 have direct fixes (snap on resize; sample `animation.currentTime`; keep paint on the element or re-sync it; move the clip to the surface). Items 5–6 need exits to have their own identity and lifetime. Whether to keep the emulation at all is the open design question in the review.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Grok 4.7 xhigh, Astra max (code and design), Opus 5.5 max (design). Verification: 1–4 confirmed by reading; 5–6 plausible.

Fixed in `host/web/presence-glue.js`, with resize marking in `navigation.js`
and the resize/agent-seek calls in `glue.js`:

- Resize batches carry their snap instruction through the lazy-load queue.
  Active layout moves stop and new boxes take effect without starting a move.
- Spring interruption samples `Animation.currentTime`, less its delay, for
  both displacement and velocity.
- While size motion runs, the surface reads presented paint each browser
  frame, batch and agent seek. Paint animations retain their clocks; their
  paint is suppressed on the element only while the surface draws it, and
  restored before style changes and on cleanup. CSS paint transitions,
  opacity, shadows, borders and pressed transforms are covered.
- The element's overflow clip is lifted temporarily; the animated inset can
  extend outside the final box while shrinking. Cleanup restores overflow.
- Item 6 reproduced in block layout: swapping equal-height siblings around a
  resizing card leaves its box at y=60 but moves its skipped anchor to y=240.
  Repositioning the surface after each batch keeps it at y=60 even when its
  owner's box did not move.
- Item 5 did **not** reproduce in Chrome 154.0.8037.58, with either the agent's
  paused clock or a running clock. The old `pulse` retains its 1-second,
  infinite play and current time; the appended exit is a fresh finite
  200-millisecond animation whose finish removes the ghost. No exit-identity
  change was made.

`host/web/tests/presence.test.mjs` runs the production controller, resize
entry and child reconciliation in Chrome. Items 1–4 failed before the fix;
the block-reorder case for item 6 failed against the original controller.
All eight browser tests pass after the fix. Built presence fixtures were
also driven with `bun scripts/agent.mjs web`: `clock +150` then retarget kept
the spring-driven row at y=21.67; a shrinking card at `clock +500` showed its
intermediate purple paint and children beyond its final box, and settled
after another retarget. The loader's two regressions, adjacent focus/press/
agent tests, Rust checks, web host tests, caps and boot also pass.

The supplemental `bun scripts/smoke.mjs web` completed its functional
scenarios but reports one failure from headless Chrome's macOS keychain
access (`-25308`), unavailable password-store encryption and `Invalid
first_paint` diagnostics; no page exception or failed functional scenario
was reported.

Charlie may still replace the emulation with View Transitions or
`@starting-style`; these fixes make no ruling on that design question.
