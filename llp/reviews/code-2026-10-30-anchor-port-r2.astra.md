# Code review: the JS target's port of #1095's collection anchor, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `d5608abb78`.
- **Method:** one brief (sha256 `476735e0299b7df85349d848bc557c1f131aff5c290a21987449f9d8cd542d61`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. The changed anchor fields, restoration branches and viewport arguments match `7b0352262f`; I found no runtime defect in the port.

**Non-blocking**

1. [host/web-js/list.test.mjs:822](host/web-js/list.test.mjs:822): **The traversal can pass vacuously on `NaN`.** If restoration returns `NaN` or `undefined`, `Math.abs(restored - want) > 1e-6` is false, leaving `misses` empty—even for all 2,000 iterations. Reject nonfinite values explicitly before comparing the difference.

The two round-1 requests are addressed, with these qualifications:

- **Opening test:** The wait at [list.test.mjs:783](host/web-js/list.test.mjs:783) resolves the controller-import race, and the comment no longer promises subsequent growth behavior. Setting `c.atEnd = true` exercises the real [anchor calculation](host/web-js/list.js:338). Reverting `anchorOffset` to `extent` would produce 300 pixels of overshoot and fail its assertions. This is a meaningful focused regression test. It bypasses the lifecycle transitions and directly tests neither `reportAnchor` nor when opening ends. Also, because the settled offset already equals the maximum, an incorrect identity implementation of `anchorOffset` could pass. An off-end input would strengthen that distinction; the existing [opening integration test](host/web-js/list.test.mjs:439) exercises measurement during opening.

- **Traversal formula:** [list.test.mjs:821](host/web-js/list.test.mjs:821) is correct for this unchanged, unpadded index, including zero-height rows. With `max >= 0`, a rowless overshot anchor also satisfies it: `clamp(raw) + beyond = max + (raw - max) = raw`. Leading-padding offsets without overshoot clamp to zero; trailing-padding offsets likewise agree.

  It is **not universal when `max < 0`**. The existing [padded fixture](host/web-js/list.test.mjs:748) has total 500, leading 92, trailing 83 and viewport 600, giving `max = -17`. At offset zero, the formula predicts 17; both [JS](host/web-js/list.js:207) and Rust correctly restore raw zero. The traversal does not generate that geometry; the separate rowless assertions cover it.

Static review only; no edits, network, tests or builds.
