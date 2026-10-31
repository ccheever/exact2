# Code review: LLP 1071.001 part B5, enumerated style bindings as runtime-built arm classes, round 3, narrow re-check (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `1065db52a0`.
- **Method:** one brief (sha256 `8826425cee8f18dadc5513c01d82b0ce8d9787d19b1fbfdce81a4aad68736046`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

- The re-grep is short a few reads. None of them let a lowered declaration change a page decision. `check.js:33` reads a property only after that element's class rules, including an arm class. `navigation.js:625` reads `--exact-animation-clock`, which `refuse_prop` already rejects as a custom property (`lower.rs:435`). `dev.mjs:772` reads `font-size` and `--exact-host-font-size` on `document.documentElement`, the same root scope as `dev.js:534`. `rt.js:824` is `normal()`'s scratch `<i>`, not an app node. `conform.mjs:451` is the conformance driver reading inline grid properties (`grid-template-columns`, `grid-template-rows`, `grid-column`, `grid-row`, `grid-auto-flow`, `justify-items`); those can be lowered, and the probe only compares the two hosts to each other.
- `gpu-glue.js:237` also saves `zIndex` next to `transformOrigin`. `z-index` can be lowered (`style.rs:1149`) and is not in `READ`. The saved value is only written back by `restoreChild` (`gpu-glue.js:205`). An empty inline assignment removes the placement override, so the class value shows again, and placement does not branch on the author's `z-index`. That is the same shape as the synchronous `width` / `flex-shrink` restore at `navigation.js:1042`, not the append that made `animation` and `transition` real.
- `animation` is an authorable attribute (`contract/lower/src/style_names.rs:298`). The byte-identical loop does not include a source fixture for it; `refuse_prop("animation")` does (`lower_tests.rs:402-413`, `lower.rs:429`). The comment that no contract attribute binds it is wrong for `animation`. The gate still refuses the row.
- `motion-glue.js:319` and `:464` (and the save at `:306`) snapshot `cssText` and later restore it. I agree that is below MAJOR. Motion nodes are refused whole, so their authored style is inline and is in the snapshot. A lowered row's arm lives on the class, which assigning `cssText` does not touch. What remains is an arm change during an in-flight raise or hold, which the restore can put back to the pre-gesture inline snapshot. That is a narrow window, not a property that is missed on every read.

**B11.** Resolved. `build.mjs:188-190` hashes the runtime array after the `modes.js` / `rt.js` transforms, and puts `lower.js`'s own unstamped source in that array. `withRuntimeStamp` (`module.mjs:360`) digests `path` plus text, and replaces only the first `"exact-runtime-unstamped"`, which is the `RUNTIME` constant (`lower.js:15`). The rule builder, the selector, and the unset path are in that source (`lower.js:30-35`). What else writes an arm's text is in the same array: `cssText` and `normal` in `rt.js` (`rt.js:821-829`), `regions.js` (called from `cssText`), `grid.js`, `font-family.js`, and `backdrop.js`. The recipe text and its module constants are already in the arm name (`lower.rs:258-269`). Files copied after the stamp are not on this path.

An unbuilt module sets `HOST` once per evaluation to `unstamped` plus `Math.random()` and `Date.now()` (`lower.js:16`). A new evaluation cannot share a host with the sheet it finds, so it clears that sheet (`lower.js:23`). The same evaluation keeps its host, and a helper cannot change underneath it without a new evaluation of this module. That is a sound fallback.

The tests call the real gate. A `lower.js`-only text change and a helper-only text change each move the digest (`lower.test.mjs:145-156`). Two stamped copies of the same files keep the old rule and add the new one (`lower.test.mjs:94-108`). A stamped copy whose helper text changed rebuilds a poisoned kept rule from the recipe (`lower.test.mjs:111-124`). Two unbuilt evaluations leave only the second rule (`lower.test.mjs:127-135`).

**B12.** Resolved. `right`, `bottom`, and `transform-origin` are in `READ` (`lower.rs:121-123`). `animation` is refused in `refuse_prop` (`lower.rs:127`, `lower.rs:429`). Those are the reads: `insetAt` destructures inline `left` / `top` / `right` / `bottom` and reads `position-area` (`navigation.js:1490`); placement saves `transformOrigin` (`gpu-glue.js:237`); an exit reads `s.animation` and appends to it (`presence-glue.js:302`, `presence-glue.js:331`). The source fixtures go through `refused()`, which requires one `GlueWritten` and JS and CSS byte-identical to the unlowered build (`lower_tests.rs:22-34`, `lower_tests.rs:364-399`). `mask-size`, `overflow-anchor`, `view-transition-name`, and `animation` are locked with `refuse_prop` (`lower_tests.rs:402-413`).

The inline reads in `host/web` and `host/web-js` are refused, or limited to a class-aware reader, the document root, the render shim, a scratch element, a refused node (`time`, pressable, motion, `$ink`), or a save/restore that does not decide from a missing class value. The omissions above do not add another one.

APPROVE
