# Code review: a stale slice frees what waited for it, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `cfa73c2762daca7023fa03c2e49572df596aa337f36f9bf570c6b04063e4d638`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocker (Astra P3; Grok P2).
- **Disposition:** Astra's P3 applied without re-review; Grok's P2 not taken, with the reason in the third-round commit

---

No blocking findings.

- **P3 — Destruction does not invalidate the view binding.** [Session.swift:574](/tmp/rv-af3/host/apple/Sources/ExactKit/Session.swift:574) checks only `viewsEpoch`, while [destroy():1491](/tmp/rv-af3/host/apple/Sources/ExactKit/Session.swift:1491) resets the presenter without incrementing it. If one deferred callback destroys the session, subsequent bound callbacks can still execute: the deferred loop checks destruction only once. Increment the epoch after this reset or check destruction per item. This is nonblocking because the runtime refuses calls after destruction and `apply` guards presentation.

The regression test exercises the original lost-work bug and would fail against the old discard logic. It does not exercise actual reset callbacks or requeueing behind another fill. The binding survives that second requeue by inspection.

No tests were run: the checkout has no built test artifacts, and the review remained read-only.
