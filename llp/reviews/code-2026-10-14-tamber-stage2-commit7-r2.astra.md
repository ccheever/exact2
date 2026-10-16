# Code review: LLP 1121 stage 2, commit 7, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `14e4e5c142db6204b1d1058940593ff7319e626bd1109146364cfb47be8bade4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, two low; Grok: three low.
- **Disposition:** all fixed in the round-2 commit

---

No high-severity findings. Two medium and two low remain.

1. **Medium — CLI validation disagrees with app compilation.** [contract/cli/src/tamber.rs:63](/tmp/rv-s2c7r2/contract/cli/src/tamber.rs:63) calls `exact_tamber::check` without the lowerer’s design restrictions. A token `top = #000` passes this checker but compilation rejects it as `lower-theme-name`; an axis-varying `Canvas` override similarly bypasses `tamber-role-axis`. This makes “checked” misleading. **Fix:** share the design admission checks between compilation and the CLI, and test both paths against the existing refusal fixtures.

2. **Medium — Token inspection is missing from the default web target.** [runner/src/agent.rs:540](/tmp/rv-s2c7r2/runner/src/agent.rs:540) adds metadata only to the Rust inspector. The JS target independently constructs rows in [host/web-js/agent.js:140](/tmp/rv-s2c7r2/host/web-js/agent.js:140), returning only `value` and `source`. Thus the documented web `layout <id>` has no token names, tuple or boundary flag. An in-memory invocation confirmed this. **Fix:** emit equivalent metadata through the JS inspector and add web coverage for authored, dynamic and boundary rows.

3. **Low — The round 1 channel filter rejects `alpha` references.** [scripts/no-tells.mjs:162](/tmp/rv-s2c7r2/scripts/no-tells.mjs:162) omits `alpha` from its keyword sets. With `brand` declared, `rgb(from brand calc(alpha * 255) g b)` is reported as a literal, although the evaluator accepts `alpha` as a source-channel keyword. **Fix:** include `alpha` in relative-channel recognition and add a regression test retaining the literal-channel rejection.

4. **Low — Nested token-derived origins are reported as literals.** [scripts/no-tells.mjs:169](/tmp/rv-s2c7r2/scripts/no-tells.mjs:169) accepts only a single identifier after `from`. Consequently, `rgb(from color-mix(in srgb, brand, surface.card) r g b / 0.5)` produces a finding despite deriving entirely from declared tokens. **Fix:** recognize balanced origin expressions recursively, with tests covering nested token expressions and nested literals.

The other listed round 1 fixes look sound by inspection. Caps, `git diff --check`, eight selected read-only JavaScript tests, Tamber help and recipe lint passed. Rust builds and tests were not run because they would write artifacts. No files changed.

The apparent RFC deletion in the requested diff is baseline divergence: `origin/main` contains sibling documentation commit `cc08e3c50`. Neither reviewed commit deletes those notes.
