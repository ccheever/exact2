# Code review: per-subtree `color-scheme` (LLP 1034 §8), round 3, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x23`.
- **Method:** one brief (sha256 `663ab3bf403f1a86294cbcac733d0e078fbcc48b57bfab6be754cbb1d9d45e9d`), shared with grok. Round 3 (the last), blind to the other review. Reviewed the staged diff (with the round 2 fixes) in a worktree at 4ce1a103e. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Round 3 was the last (at most three rounds). 1 (the new main-queue flush could apply batches out of order) resolved by withdrawing that flush: notes made outside a batch wait for the next batch, as before this change; the first scheme's replay is now queued before `runtime.scheme` and said in that batch, after the scheme, by the existing in-batch drain. 2 and 3 NOT TAKEN, declared: §8's keyframe paragraph now says that keyframe animations of `light-dark()` colours follow only a view's first differing report, that later scheme changes leave playing (and same-commit) animations in the half they started with, that a report outside a batch waits for the next one, and that a subtree fixed to the scheme the session leaves is reported only when its views next change. Transitions resolve by the commit. These changes after round 3 were not reviewed again; iOS and macOS XCTest and the five checks were re-run on them.

---

NOT READY

1. **[Session.swift:1338](/tmp/x23/host/apple/Sources/ExactKit/Session.swift:1338) — must-fix: the new flush can apply batches out of order.** With a tick/fill in flight, `runtime.viewScheme` produces batch A after that work. `apply(A)` first lands the earlier batch at line 963; that batch’s appearance drain can produce and apply batch B. A then applies after B, overwriting newer styles or frame flags. **Fix:** drain in-flight publications before removing a pending report or calling `runtime.viewScheme`, then recheck destruction/generation. Test multiple queued reports with a tick awaiting presentation.

2. **[Session.swift:1291](/tmp/x23/host/apple/Sources/ExactKit/Session.swift:1291) — must-fix: fixed schemes that previously matched the session can remain unreported.** A light subtree in a light session has no `viewDark` entry. When the session turns dark, that subtree stays light and receives no appearance-change callback—confirmed with an AppKit probe. Starting a background-only keyframe afterward therefore uses the engine’s dark session scheme, indefinitely. This exceeds the declared same-commit gap. **Fix:** after each session-scheme change, schedule a scan of fixed-scheme views after trait propagation and report their appearances, including those whose effective appearance stayed unchanged.

3. **[1034-scheme-aware-colour.rfc.md:404](/tmp/x23/llp/1034-scheme-aware-colour.rfc.md:404) — should-fix: the declared keyframe limitation understates the behavior.** [paint.rs:283](/tmp/x23/kernel/src/motion/paint.rs:283) recolours playing keyframes only when `first` is true. Changing an already-reported dark subtree back to light leaves existing and same-commit-started animations dark even after the report. **Fix:** amend §8 and the disposition to disclose that subsequent reports do not correct those plays; “until its report” is inaccurate.

The `self.views` borrow is valid, and appearance reset now precedes new-tree application and `rebooted()`. The Linux ghost/menu and flat-fill fixes address their findings.

Read-only checks: existing Contract, Apple style, and Linux scheme tests passed. The prebuilt transition test failed, but its executable predates the final `paint.rs` edit; no rebuild or Session integration tests were run.
