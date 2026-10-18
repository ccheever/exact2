# Code review: the design tool's reads during a scrub, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the first commit and round 1.
- **Method:** one brief (sha256 `8e49f79dd2c3b00f739fd1178588fcaa8dcc4cce088878cd9e1ecc4c1e446f3f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (a JS page reports no plan; source-location-only package edits; live with no plan change; the retry replacing itself).
- **Disposition:** redesigned in round 2's commit: per-answer reads as before, paced only while a scrub is open.

---

Nothing is blocking. I found no `plansAt` ordering defect or scrub-frame churn in `plans`.

1. **should-fix — [apps/design-tool/app.ts:1076](/tmp/rv-dq-2/apps/design-tool/app.ts:1076): JS-target rebuilds never change this key.** JS pages report `plan: null`; their digest is in `planDigest` ([host/web-js/dev.mjs:697](/tmp/rv-dq-2/host/web-js/dev.mjs:697)), which `buildOf()` drops. With only JS pages connected, `changes.plans` remains `""` across package rebuilds, leaving cached uses unchanged. An in-memory reproduction confirmed this. **Fix:** retain and use `planDigest`.

2. **should-fix — [apps/design-tool/app.contract:550](/tmp/rv-dq-2/apps/design-tool/app.contract:550): running plans still cannot fully invalidate uses.** Even after fixing the field, inserting a blank line above a token reference in an external package changes the returned source locations without changing the compiled plan. Those files are excluded from `changes.files`. With no builds connected, any such dependency edit is invisible to `plans`, including changes to the actual uses. **Fix:** key uses on a server revision covering its compiler inputs and source locations, independent of connected builds; a bounded periodic refresh can provide a fallback.

3. **should-fix — [apps/design-tool/app.contract:697](/tmp/rv-dq-2/apps/design-tool/app.contract:697): `live` can change without a reported plan changing.** The wasm loop recomputes `typescript`/`portableRust` after rebuilding, which changes `hello.live`. A native client can remain connected on its old generation while awaiting relaunch ([PlanURL.swift:849](/tmp/rv-dq-2/host/apple/Sources/ExactKit/PlanURL.swift:849)); no connected builds also supplies no invalidation signal. With unchanged design files, hello remains cached and Scrub availability stays wrong. **Fix:** include the loop’s `live` value or capability revision in poll answers and refresh hello when it changes.

4. **should-fix — [apps/design-tool/app.contract:699](/tmp/rv-dq-2/apps/design-tool/app.contract:699): retries can replace themselves before completing.** After one failure, `uses.ok` remains false while its retry is pending. Every successful poll answer therefore forces another request. The runtime reproduction confirmed that the earlier retry’s successful reply gets discarded. Persistent failures also restore one uses request per status answer during scrubbing. This is event-driven amplification, not a self-triggering infinite loop. **Fix:** retry through a delayed task gated by `not pending(uses)`, with bounded backoff independent of poll frequency.
