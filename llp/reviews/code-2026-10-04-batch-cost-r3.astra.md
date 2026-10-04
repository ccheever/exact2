# Code review, round 3 (final): iOS host batch cost (0d118c2d7), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, a detached worktree at `0d118c2d7`.
- **Method:** one brief (sha256 `e6c8ae9d06d0d330e8aa67b38861e43068d72358f7405894e8c6547fc39f4650`), shared by both reviewers. Round 3, the last fix round, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final findings and verdict, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition: superseded, not landed.** After three rounds the batch-cost branch (f3de06ddd..0d118c2d7) was abandoned on the coordinator's decision, 2026-10-04. Its idea was to skip work inside the presenter's pass. The empty batches came from the app-timer path, the one path without the session's existing "nothing changed" predicate (fills and list feedback already skip theirs). The replacement skips the whole pass there, and keeps r3's `controls` flag for the viewless-children case this round found. It is commit "Session: an idle timer tick only moves the clock", with its own reviews (`code-2026-10-04-idle-tick.*`). This round's blocker, that fast paths ignored `controls`, is addressed there: `changesNothing` and `applySnapshots` read the flag.

---

Static review of `0d118c2d7`; no files changed or tests run. **Round 2 is partly resolved:** ordinary viewless-content updates now set the flag, and controls observe their own sizing traits. The flag still does not reliably reach synchronization.

1. **Blocker — The collection fast path discards control invalidation.**  
   [PresenterIOS.swift:659](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:659) accepts a batch containing only collection operations without checking `batch.controls`, then returns before the new gate at [PresenterIOS.swift:889](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:889).

   A concrete case is changing only a fixed-size native button’s title inside a virtualized row. Changing a row-body dependency advances the collection revision ([collection/mod.rs:524](/tmp/x11-review/runner/src/instance/collection/mod.rs:524)). The title update sets `controls: true`; unchanged geometry leaves collection metadata as the only operation. `applySnapshots` consumes that batch without refreshing the button. The flag is not retained in `controlsStale`, so subsequent empty batches cannot repair the stale title, accessibility label or intrinsic measurement.

   **Fix before landing:** require `!batch.controls && !controlsStale` before taking the snapshot fast path. The session’s discard predicates at [Session.swift:629](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:629) and [Session.swift:667](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:667) also omit `batch.controls`; make those respect the new signal too.

   Add a regression that changes only a viewless face/menu input while producing collection metadata, then verifies the native control updates without an unrelated operation. Exercise serialized flag decoding. The current Rust test changes the button and option together ([controls.rs:25](/tmp/x11-review/host/apple/tests/it/controls.rs:25)); the Swift test assigns the flag directly ([NativeButtonsIOSTests.swift:79](/tmp/x11-review/host/apple/tests/ExactKitTests/NativeButtonsIOSTests.swift:79)). Neither covers this early return.

The requested projection audit otherwise checks out statically:

| Path | Result |
|---|---|
| Create/update of an option, label, face text or run | Suppressed creates and updates set the flag at [paragraph.rs:211](/tmp/x11-review/host/apple/src/paragraph.rs:211) and [paragraph.rs:279](/tmp/x11-review/host/apple/src/paragraph.rs:279). |
| `option_part` walk; symbols inside buttons | The walk climbs text ancestors and checks for a button parent **before** stopping at a non-text node, so direct image/symbol children are covered ([paragraph.rs:11](/tmp/x11-review/host/apple/src/paragraph.rs:11)). |
| Children changes; `when` swapping a face | Control child-list suppression sets the flag ([paragraph.rs:334](/tmp/x11-review/host/apple/src/paragraph.rs:334)). Changes beneath an option or face text first pass through the touched-node update loop ([host.rs:1195](/tmp/x11-review/host/apple/src/host.rs:1195)). |
| Destroy of an option, face child or run | Destruction touches the surviving parent ([txn.rs:568](/tmp/x11-review/kernel/src/txn.rs:568)), which triggers invalidation. Viewless identities also retain keys and produce destroy operations ([host.rs:1169](/tmp/x11-review/host/apple/src/host.rs:1169)). A separate destroy-site flag is unnecessary here. |
| Kernel-driven changes | Inherited changes enter receipts; environment changes explicitly call `update`. Layout-only changes that bypass it emit changed frames ([layout.rs:158](/tmp/x11-review/host/apple/src/layout.rs:158)); unchanged geometry does not itself alter face/options data. I found no additional missing producer invalidation. |

The per-control registration resolves round 2’s observer-placement problem ([ControlsIOS.swift:101](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:101)), although the tests still manually set `controlsStale` rather than exercising a descendant-only trait override.

Deferred geometry replay, appearance invalidation and exit cleanup remain accounted for. Finer per-control invalidation and skipping synchronization on additional nonempty batches can be **DEFERRED**. Delivering the new flag through existing fast paths cannot. Since this is the final fix round, the remaining blocker requires a human decision or descoping the skip.

Verdict: DO NOT LAND
