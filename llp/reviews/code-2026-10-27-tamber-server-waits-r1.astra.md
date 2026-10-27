# Code review: tamber-server wire tests' event waits, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `107a359436`.
- **Method:** one brief (sha256 `fec285730d7cf20e2c5a2ec280511fc9ab241506cced2602bf8ee24e752752f1`); a quick review, one reviewer. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [scripts/tamber-server.test.mjs:304](scripts/tamber-server.test.mjs:304) — The six added lines push this file to 1,504 lines; `caps.mjs` counts 1,505, exceeding the enforced 1,500-line cap. Remove at least five lines without removing coverage.

2. [scripts/tamber-server.test.mjs:308](scripts/tamber-server.test.mjs:308) — `WIRE_TEST = 90_000` is shorter than cumulative waits: the checkout test has eight 20-second waits (160 seconds), and the held-proposal test has nine (180 seconds). The checkout test alone has seven sequential watcher events. Under sustained load, Bun can still preempt a later wait’s diagnostic. Give these tests limits covering their sequential waits plus setup/request overhead.

**Non-blocking**

none.

No changed wait uses expiry to prove an event never arrives; the negated predicates await observed file removal. No affected caller was missed: the import test already allows 120 seconds, and both `.contract` tests allow 600 seconds. The remaining five-second abort checks an immediate `wait: false` response and is appropriately unchanged.

Tests were not run; no files were edited.
