# Code review: LLP 1122, stage 6 kinds in the tool, step 1 (length dials write px; non-colour tokens load), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `ff4d6dc38`.
- **Method:** one brief (sha256 `0f9fb5120e7b098d119b0f5cebd32c2951a2563262f4df1ef8a5df2563668e4b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 1 low).
- **Disposition:** taken; steps 1 and 2 (the grid by kind) are now one commit, which round 2 reviews.
  - The "only this mode" scope is decided by the token's kind (snapshots keep each token's kind): a non-colour token's own `when` names no under-dark axis, since it has no sides and may not vary there, and a duration or easing has none.
  - The line cap: the panel's controls and the grid's cells by kind moved to apps/design-tool/dials.ts (app.ts is 1,356 lines, and caps passes).
  - A length dial applied end to end through the real server: the dials test drives the Gap slider to 10, applies it, and the file has `gap: length = 10px`.
  - Not taken: more cases in the stub (it doesn't check values); the real-server drive covers what the stub can't.

---

- **Medium — [app.ts:319](/tmp/rv-dt-k1/apps/design-tool/app.ts:319): non-colour tokens inherit invalid “Only this mode” scopes.** Select the new `space.gap` token in `3-dark`, choose “Only this mode,” and pin `12px`. The editor proposes `when dark-style=dim density=compact`, which the checker rejects: non-colours cannot vary on under-dark axes. Duration/easing similarly receive an option to add forbidden `when` branches. Preserve token kinds in snapshots and derive legal scopes by kind; disable mode overrides for motion tokens.

- **Medium — [app.ts:1503](/tmp/rv-dt-k1/apps/design-tool/app.ts:1503): the commit breaks the mandatory line-cap check.** `caps.mjs` counts this file as **1,504 lines**, up from 1,497; the limit is 1,500. Trim or extract code before landing.

- **Low — [app.test.contract:94](/tmp/rv-dt-k1/apps/design-tool/app.test.contract:94): coverage proves draft formatting, not end-to-end writes.** The new test checks `10px`/`6px` in state but never applies or scrubs them; the stub accepts writes without checking values. Other stage 6 grid kinds also lack assertions. Add real-server length apply/scrub-release checks, empty/negative/decimal cases, and representative non-colour cell and branch assertions.

Otherwise, the reviewed paths look correct: sliders and steppers append px once; selects and colour fields preserve their values; empty steppers remain empty; scrub/release preserve the formatted draft. Rem and named lengths are valid **tokens**, but the checker correctly refuses them as dial values.

Read-only, in-memory probes confirmed string cells for all requested token kinds and missing tuples, preserved colour residuals, and “not a colour” contrast results for non-colours. Fixture comparison found no unrelated drift: only the new dials/tokens, expected digests, and the shifted warning line. Full write-producing test suites were not run; the working tree remains unchanged.

**Verdict: changes requested for invalid branch scopes and the failing line cap.**
