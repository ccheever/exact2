# Code review: conform --linux: synthetic-decorations' rangeFrame held to the Linux host's specified 'unavailable' (theme2 04:27), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `1cdb3d192c`.
- **Method:** one brief (sha256 `3fc654272dbbb9c2113470b680a9be5075b947cb948e13202198b16869fb5305`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking or should-fix; one nit (LINUX_APART's comment said only stop-before)
- **Disposition:** taken in 984a608543

No blocking or should-fix findings.

- **Nit — [conform.mjs:757](/tmp/rv-deco1/host/web-js/conform.mjs:757):** “stops before that step” now conflicts with asserted entries. Fix: describe two-element entries as stop-before, and three-element entries as execute, assert, then stop.

The requested checks hold by inspection:

- `boot`, `reset`, and `replace` retain Linux comparisons. `measure` is the last tap; only the final `clock +60000` comparison is dropped after divergence. Wasm/JS comparisons continue.
- [conform.mjs:396](/tmp/rv-deco1/host/web-js/conform.mjs:396) awaits the action and settlement before reading the correct `state().slots.geometry`. The fixture sets that string from `caret.unavailable`.
- Existing two-element entries still stop without executing; null-step entries still skip Linux at startup.
- Wrong/missing values and action, settle, or state-read errors call `fail()` under `tap measure`. The Linux session then closes and `L` becomes null.
- [QUEUE.md:1573](/tmp/rv-deco1/QUEUE.md:1573) is accurate: Apple is the only production `InlineGeometry` implementation; Linux’s `text/field.rs` contains the proposed geometry primitives.

Read-only review; no builds or tests run.
