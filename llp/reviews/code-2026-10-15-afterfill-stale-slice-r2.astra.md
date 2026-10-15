# Code review: a stale slice frees what waited for it, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `e9f19841e507b4f0453f537068331ec33ee298ea1d2b7ec5a192892ea020a7a9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both P1: reset-time cover clears bound to the new generation; Astra P2: binding dropped on re-queue).
- **Disposition:** both fixed in the second-round commit (viewsEpoch, idle)

---

Two findings remain; the first is blocking.

1. **P1 — Reload cleanup can clear the replacement tree’s covers.** [Session.swift:733](/tmp/rv-af2/host/apple/Sources/ExactKit/Session.swift:733)  
   `commit()` advances the generation before `presenter.reset()`. On iOS, [NavigationIOS.swift:1060](/tmp/rv-af2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:1060) reports clears for the old views during that reset. With a fill outstanding, those clears are queued **under the new generation**, survive the stale-landing filter, and execute next turn—after the replacement tree has reported its covers. Reused node IDs therefore lose their new covers, potentially restoring hidden headers’ layout space or removing bar insets. Navigation’s `delivered` cache still records the new covers, so unchanged geometry does not resend them. Reset-generated reports need to be suppressed or associated with the outgoing presenter.

2. **P2 — Generation bindings are discarded before callbacks finish waiting.** [Session.swift:660](/tmp/rv-af2/host/apple/Sources/ExactKit/Session.swift:660)  
   Filtering followed by `.map(\.work)` removes the generation, and line 665 requeues through unbound `whenIdle(work)`. A callback valid at the stale landing can become stale before the deferred turn—or while waiting behind another fill—and still run against the replacement runtime. Eager filtering also misses a generation change caused by an earlier callback in the inline loop. Keep the original generation through requeueing and check it immediately before execution. Callbacks already stale at the initial filter are correctly dropped.

By inspection, [IdleTickTests.swift:92](/tmp/rv-af2/host/apple/tests/ExactKitTests/IdleTickTests.swift:92) now exercises the original bug: with the old dropping behavior, `free` remains zero and the test fails. It does not exercise real presenter reset, another intervening commit, or requeueing behind another fill.

I found no additional new blocker in destruction handling, unbound-work liveness, or ordinary deferred-pump ordering. XCTest was not run because this review was read-only; the working tree remains unchanged.
