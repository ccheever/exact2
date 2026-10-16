# Code review: LLP 1121 stage 2, commit 4, round 3 (last), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `bc0f9b16c9a07f32c2ab99de5896edeb8df9fea3400a3f4d9d89faae0670f1e6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** applied without re-review in the round 3 commit

---

**MINOR — Empty mode replies can be stale** ([web/glue.js:1151](/tmp/rv-s2c4/host/web/glue.js:1151), [Agent.swift:437](/tmp/rv-s2c4/host/apple/Sources/ExactKit/Agent.swift:437)). Both retain the preflight tuple for `mode: {}`. A page update can mount an autofocus field whose focus action calls `setMode`; the reply then reports the earlier tuple. Re-read modes after applying the other facts.

The web glue/focus probe reproduced this: reply `black`, current mode `dim`, with the wasm boundary stubbed. Apple has the same ordering.

No other findings in the scoped fixes. Seven targeted Bun tests and ten glue probes passed. Rust/Swift tests weren’t run: the read-only worktree has no build artifacts. No edits.

VERDICT: fix (MINOR: host/web/glue.js:1151, host/apple/Sources/ExactKit/Agent.swift:437)
