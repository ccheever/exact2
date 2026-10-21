# Code review: a hidden tablist over a stack's root hides the tab bar (LLP 1075.003 §3.7 as amended), round 3, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x24`.
- **Method:** one brief (sha256 `d25cd2d5e8ebada5e3357f76f744e2d4cbffdb7da93219bebc0e1e23766cfcd5`), shared with grok. Round 3 (the last), blind to the other review. Reviewed the staged diff (with the round 2 fixes) in a worktree at 5111d86af. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Round 3 was the last (at most three rounds). Both reviews were NOT READY on one item, the same in each: a native route hook that sets the tab bar in a projection made during a pop onto a root whose tablist is hidden (or during a tab container's rebuild) runs before the arrival is reconciled, and the reconciliation on the next turn overrides it. Not fixed: recorded in LLP 1075.003 §3.7 as a known gap with the fix both reviews named (defer such a hook until the arrival is reconciled). It needs a native route hook that writes the tab bar during that pop; no app in the repository has one (the census). The other round 2 items were confirmed resolved by both reviews. No code changed after this round.

---

NOT READY

1. [NavigationTitleIOS.swift:364](/tmp/x24/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:364) — **must-fix:** Round 2’s hook-ordering finding remains. During `didShow`, `pendingSync` runs synchronously at `NavigationIOS.swift:723`. While the coordinator remains set, this guard skips reconciliation without consuming the arrival. However, `prepareRoutes` still runs an updated root hook at `NavigationBarIOS.swift:417`. The queued `settleTablist` subsequently sees an unconsumed arrival and overwrites that hook.

   Concrete case: pop Detail onto a still-hidden Home while queuing root metadata whose changed hook shows the bar. The hook runs, then deferred settlement hides the bar again. An intervening projection does not protect it: that projection consumes the arrival **after** the hook already ran.

   **Fix:** defer reconciliation and the affected route-hook delivery together until the coordinator clears, applying the tablist default before the hook and consuming the arrival once. Add a regression with metadata changed during the pop; the current test’s hook overrides occur only at rest. LLP’s ordering guarantee should describe that implemented sequence.

The other round-2 fixes are present, including container-state cleanup, hide ownership, and the expanded push/pop coverage. UIKit tests were not rerun in this read-only review.
exit 0
