# Code review: LLP 1071.001 part B5, enumerated style bindings as runtime-built arm classes, round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `1065db52a0`.
- **Method:** one brief (sha256 `8826425cee8f18dadc5513c01d82b0ce8d9787d19b1fbfdce81a4aad68736046`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none.

B11 is resolved: [build.mjs:190](host/web-js/build.mjs:190) hashes lower.js’s unstamped source alongside the transformed runtime array. That covers the rule builder and the declaration helpers I traced. [lower.js:16](host/web-js/lower.js:16) gives each unbuilt module generation a fresh host identity; the mismatch clears both rules and cache at line 23. This fallback is sound for the stated use.

B12 is resolved: [lower.rs:112](host/web-js/src/lower.rs:112) covers right, bottom and transform-origin; animation is separately refused at line 429. The table-driven fixtures call a helper that compares both generated JS and CSS byte-for-byte with lowering disabled ([lower_tests.rs:22](host/web-js/src/lower_tests.rs:22)). My re-grep found no additional unsafe property beyond the acknowledged general cssText follow-up.

**Non-blocking**

1. **The commit’s read inventory is not literally complete.** These omitted reads have existing protection or safe scope:
   - [gpu-glue.js:237](host/web/gpu-glue.js:237): `z-index` — its dynamic paint emission triggers R5 ([paint.rs:195](host/web-js/src/paint.rs:195)).
   - [navigation.js:625](host/web/navigation.js:625): `--exact-animation-clock` — R3.
   - [geometry-glue.js:62](host/web/geometry-glue.js:62): the alias reads **height and transition**, already refused; overflow-anchor is the separate read at line 65.
   - [rt.js:824](host/web-js/rt.js:824) and [motion.js:174](host/web-js/motion.js:174): scratch declaration reads.
   - [check.js:33](host/web-js/check.js:33): class-aware lookup.
   - [navigation.js:1011](host/web/navigation.js:1011) and [dev.mjs:772](host/web-js/dev.mjs:772): document-root font reads.

2. **B11’s tests are meaningful synthetic coverage, with a boundary.** [lower.test.mjs:110](host/web-js/lower.test.mjs:110) exercises actual rule rebuilding after a changed stamp, while lines 94, 125 and 135 cover reuse, unbuilt invalidation and digest changes. The “helper change” changes supplied hash inputs, not executed helper code; these tests do not directly protect build.mjs’s runtime-list wiring.

3. **Agree the held/raised-row follow-up is below MAJOR.** [motion-glue.js:319](host/web/motion-glue.js:319) concerns refused motion targets. The reorder snapshot at [motion-glue.js:464](host/web/motion-glue.js:464) restores cssText without replacing className, so it does not itself roll back a lowered arm. Active-reorder/arm-change coverage remains a reasonable follow-up.

Reviewed only HEAD; no edits, network, tests or builds.

APPROVE
