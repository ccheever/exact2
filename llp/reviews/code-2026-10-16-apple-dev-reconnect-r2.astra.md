# Code review: the Apple dev client waiting for its program instead of stopping, round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change and the first round.
- **Method:** one brief (sha256 `a63ab2e0045bdce337c146aead6612784e2831dbd43e9eca157f5d43c74a54ff`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a replaced connection stranded; a foreign announcement pinning; the status cleared early).
- **Disposition:** in the second-round commit.

---

Two P2 issues remain; I’d fix them before landing.

- **P2 — Replacing the connection can permanently strand it.** [PlanURL.swift:655](/tmp/rv-s4-18/host/apple/Sources/ExactKit/PlanURL.swift:655): Connect once to program A, then use Connect again while the server serves B. `Session` retains A, but the new `PlanURL.adopted` is nil, so the subscription-only branch is skipped. `observe()` rejects B and increments `resolution`; `failed()` schedules its retry against the previous resolution. Nothing subscribes or retries, even when A returns.

- **P2 — A rejected announcement still adopts its program.** [PlanURL.swift:553](/tmp/rv-s4-18/host/apple/Sources/ExactKit/PlanURL.swift:553): After an unnamed generation and `{rebuilt}`, a B announcement calls `acceptProgram(B)` before the waiting guard rejects it. [Session.swift:234](/tmp/rv-s4-18/host/apple/Sources/ExactKit/Session.swift:234) therefore permanently pins B. If A subsequently returns, even Reload or Connect rejects A until relaunch. The non-adopting envelope fix does not cover the stream’s hello.

- **P3 — Obsolete announcements clear the rebuild warning.** [PlanURL.swift:557](/tmp/rv-s4-18/host/apple/Sources/ExactKit/PlanURL.swift:557): After B triggers waiting, a stale A announcement clears `terminal` before failing the sequence/retired-epoch check. `connectionStatus` then incorrectly reports “live.”

The design-snapshot restart fix looks correct. On the normal waiting path, repeated foreign announcements retain the stream without repeated registration; I found no automatic foreign-program boot or hot subscribe loop.

Static review: Swift execution was blocked by read-only module-cache permissions. Worktree unchanged.
