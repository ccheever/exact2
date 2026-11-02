# Code review: four main-red fixes (game test drift, the starter's hunger, the root launcher's Bun) (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `948fb21ab5`.
- **Method:** one brief (sha256 `844ed758a5e9d23e06cfbef1003c6de6c65fe3ed9cf6bde8578c019bdd4f26c2`); a quick review of the fixes the lead approved. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** the MAJOR is addressed in the amended launcher commit: a bun on PATH wins, and ~/.bun/bin/bun is the fallback when PATH has none.

- **MAJOR — [scripts/exact.mjs:2](/Users/admin/x2scratch/fb1-rv-reds/scripts/exact.mjs:2): an older default installation now overrides a working Bun on PATH.** With `BUN` unset, no `~/.bun-1.4.2/bin/bun`, Bun 1.3.x at `~/.bun/bin/bun`, and 1.4.2 on PATH, the previous launcher worked; this change selects 1.3.x. `ensureBun` checks the selected executable and throws—it does not retry PATH. Preserve PATH precedence over this fallback, or validate candidates before selecting one. Add coverage for this mixed-version setup; the existing stripped-PATH test misses it.

No other findings:

- **aa6b44d953:** Both expectation updates match the cited upstream changes. The new imports are used, and Beacons’ added files are authored motion support. The assertions retain their strictness; these are minimal drift fixes.
- **7f0c38c230:** Correct and minimal. Pinned rustfmt, run through stdin, reproduces the previous long-name failure and accepts the changed source with short and long names. Multiplication, casts, division and clamping are unchanged. No serialized state or save schema changes; the starter’s empty pins and proof assertions need no adjustment.

Validation was read-only, using source/history inspection and in-memory checks. Full generator tests and proofs were not run because they write files.

VERDICT: CHANGES
