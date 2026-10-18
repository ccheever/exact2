# Code review: the Apple dev client waiting for its program instead of stopping, round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `9c697349e031f17c5d4d3e7313c775f0b1a2e7e8230cd0f5e9552cdd8bfa2ee6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (an unpinned client resuming into another program; a rejected envelope pinning; design sync cancelled).
- **Disposition:** in the first-round commit.

---

One blocking safety issue and two P2 findings:

- **P1 — An unpinned client can resume into another program.** [PlanURL.swift:554](/tmp/rv-s4-17/host/apple/Sources/ExactKit/PlanURL.swift:554): `revision()` accepts missing/invalid `program` as `nil`, leaving [Session’s program pin unset](/tmp/rv-s4-17/host/apple/Sources/ExactKit/Session.swift:234). Scenario: initial generation omits `program`, then `{rebuilt}` arrives, followed by program B. `acceptProgram(B)` adopts B and this block clears `terminal`; B’s payloads can now reach the old binary. Previously `{rebuilt}` prevented that. Recovery must require a match against an already established program.

- **P2 — A rejected envelope can permanently pin the wrong program.** [PlanURL.swift:648](/tmp/rv-s4-17/host/apple/Sources/ExactKit/PlanURL.swift:648): the new predicate calls the mutating `acceptProgram` before validating the manifest identity or events URL. On first connection, an envelope naming B with an incorrect generation digest pins B despite being rejected. Subsequent valid A generations wait forever; neither Reload nor reconnect clears Session’s pin. Validate before adopting.

- **P2 — Mismatched announcements can lose live design updates.** [PlanURL.swift:835](/tmp/rv-s4-17/host/apple/Sources/ExactKit/PlanURL.swift:835): every mismatch calls `beginResolution()`, cancelling design snapshot fetches and discarding queued frames. Scenario: while waiting for A, a stale design batch starts snapshot recovery; another B announcement arrives before completion. Recovery is cancelled, and no accepted generation follows to restart it. The design stays stale until another triggering event. Preserve or restart design synchronization while waiting.

No hot resubscribe/re-registration loop found. Normal pinned-program rejection, reconnect backoff, and close guards remain intact.

Source review only; no files changed or tests run.
