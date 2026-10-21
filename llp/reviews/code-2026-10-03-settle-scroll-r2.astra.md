# Code review, round 2: clock settle and smooth corrections (f3e676311), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `f3e676311`.
- **Method:** one brief (sha256 `5cd1dfbba79924a9beb2f5c8439e7916802e0ef211c9099a0f30c035e58bd9ea`), shared with grok. Round 2, blind to the other review. The author is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (landed):**
  - 3 (skip on configuration): taken. The test skips only under `ExactEnv.agentFreezes` and otherwise requires the animation.
  - 1 (settle does not re-check requests and motion after the native wait): recorded, not fixed. It is the shared settle loop's existing order, and it applies equally to the navigation, modal and keyboard waits that came before. It belongs with LLP 1035.003 D5's loop.
  - 2 (a test through `clock settle` itself): recorded with 1.
  - Grok's round-2 verdict on the same commit is LAND.

---

Static review only; no files changed or tests run.

1. **Should-fix — recheck the fixed point after native scrolling finishes.** The macOS predicate at [AgentMac.swift:67](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/AgentMac.swift:67) correctly detects animations, but its caller checks pending requests and runner motion **before** the native wait, then returns `settled: true` without checking again ([Agent.swift:446](/tmp/x9-review/host/apple/Sources/ExactKit/Agent.swift:446), [Agent.swift:472](/tmp/x9-review/host/apple/Sources/ExactKit/Agent.swift:472)).

   During that wait, scrolling delivers authored handlers through [NodeViewMac.swift:601](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:601) and [Session.swift:713](/tmp/x9-review/host/apple/Sources/ExactKit/Session.swift:713). A handler can start an Exact transition or request. Once the scroll finishes, the reply claims settlement while that newly created work remains outstanding. The same shared-path issue affects iOS.

   **Fix:** after native quiescence, drain collection feedback and reevaluate requests, runner motion, and canvas work; continue settlement if necessary, preserving the bounds. Cover a smooth scroll whose handler starts another transition.

2. **Should-fix — the revised test still does not exercise settlement.** [SettleScrollAnimationIOSTests.swift:42](/tmp/x9-review/host/apple/tests/ExactKitTests/SettleScrollAnimationIOSTests.swift:42) through its final assertion checks only `nativeInFlight()`. It now meaningfully tests a real correction and wheel cancellation, but cannot detect finding 1, premature success, or a broken timeout. There is also no test of the newly active macOS settle path.

   **Fix:** use a booted list fixture and call `agent.clock(["settle": true])`. Scheduling the manual landing/completion during that call avoids relying on UIKit’s display link. Assert the returned settlement status, destination offset, and cleared tracking; cover interruption and the timeout reply. Exercise macOS’s caller too.

3. **Nit — skip on configuration, not the result under test.** [SettleScrollAnimationIOSTests.swift:41](/tmp/x9-review/host/apple/tests/ExactKitTests/SettleScrollAnimationIOSTests.swift:41) treats any failure to start animation as frozen timing. A regression that prevents the correction from starting therefore skips the remaining assertions.

   **Fix:** skip explicitly when `ExactEnv.agentFreezes`; otherwise require `animating.contains(1)`.

The round-1 dispositions are:

| Round-1 finding | Assessment |
|---|---|
| Astra 1 / Grok 1: interrupted correction | **Resolved for the agent wheel.** `dragging: true` clears the animation, owed target, and serial before the offset write. The direct-superview lookup matches ownership established by [NodeViewIOS.swift:1153](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1153), including nested lists. Other plain writers are acceptably recorded as pre-existing follow-up in [the disposition:8](/tmp/x9-review/llp/reviews/code-2026-10-03-settle-scroll.astra.md:8). |
| Astra 2 / Grok 2: macOS predicate | **Predicate resolved.** Completion and cancellation clear its tracked state. The caller still needs finding 1’s fix. |
| Astra 3 / Grok 3: test quality | **Partially resolved.** Real-list and wheel coverage are useful; full settlement coverage remains outstanding as finding 2 explains. |
| Grok 4 / Astra citation note | **Resolved.** The comment cites §11 and D5; [D5:187](/tmp/x9-review/llp/1035.003-reproducible-native-gestures.rfc.md:187) and [the clock row:226](/tmp/x9-review/llp/1012-agent-api-v1.spec.md:226) now describe smooth-correction settlement on both Apple hosts. |

Verdict: LAND WITH FIXES
