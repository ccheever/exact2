# Code review: the interim conform marker for apps whose wasm reference binds no packages (photo-editor, recorder, markdown), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `ad9dbc5c52`.
- **Method:** one brief (sha256 `6ab1fc994bb89902a2fa96357c91d0ae8632539c53a00f4e6a49aba2bfafc904`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. The rejected-reference-tap blocker is resolved: [conform.mjs:586](host/web-js/conform.mjs:586) calls the helper, which records an ordinary failure on marked targets at [conform-marked.mjs:43](host/web-js/conform-marked.mjs:43). Without `difference: true`, that failure survives marking and makes `--strict` fail.

[`differ`](host/web-js/conform.mjs:358) is currently the sole caller supplying the difference flag. All seven comparison sites use it; Linux comparisons remain ordinary failures. I found no additional marker-induced suppression of rejected wasm/JS drive operations.

**Non-blocking**

- **The source test is nonvacuous but incomplete.** Its seven-site assertions are meaningful, but [conform.test.mjs:192](host/web-js/conform.test.mjs:192) counts only `differ(step,…)`. Changing a scripted error from `fail(line, e.message)` to `differ(line, e.message)` would incorrectly mark it while passing this guard. Likewise, [line 197](host/web-js/conform.test.mjs:197) misses multiline `fail` calls and other fourth-argument expressions. Check all actual call expressions, regardless of formatting or first-argument name. This is a regression-coverage gap; the current calls are correct.
- **An unconditional “no skipped operation can end green” remains too strong.** [conform.mjs:413](host/web-js/conform.mjs:413) still converts Linux operation exceptions into skips without failing. This is the acknowledged, pre-existing follow-up, not a new blocker in these commits.

Static review only; no files changed, network, tests, or builds.

APPROVE
