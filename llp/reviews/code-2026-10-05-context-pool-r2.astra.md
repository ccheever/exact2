# Code review: context-menu rows in the iOS node pool (LLP 1021 §5.1), round 2, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the reviewed commit.
- **Method:** one brief (sha256 `3ea3f70245bfa10deefda4ed0c65042a8a3ec6b8fda51c70549f3bfa779b8777`), shared with grok, pointing at round 1 and its dispositions. Blind to grok's round-2 review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Round 2 of at most two; the fixes were verified by the UIKit tests, not by a third review. 1 taken: an interaction whose view is gone has its record removed in `sync`. 2 taken: the host's `reset` detaches the parked views' interactions, with UIKit's beside them, before it clears the records (test: a parked view keeps nothing of the menu after the presenter's reset). 3 taken: the two classes moved to files of their own names (`ContextMenuLifetimeIOSTests.swift`, `ContextMenuPoolIOSTests.swift`), so `build.mjs --test --ios` runs them. The round-1 disposition's claim that NodePoolIOSTests covers a foreign recognizer was wrong; `testAForeignRecognizerStillKeepsTheRowOut` covers it now.

---

1. **[P2] Ordinary destruction leaks companion records** — [ContextMenusIOS.swift:89](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:89). Records are removed only inside `detach`, which requires a surviving view. An unpooled node can deallocate during the destroy ops, before `menus.sync()`; [`UIInteraction.view` is weak](https://developer.apple.com/documentation/uikit/uiinteraction/view). Sync then removes `interactions[id]` but leaves its `recognizers` entry indefinitely. Repeated destruction of menu-bearing views can accumulate metadata beyond the pool’s bounds. Remove the record even when `interaction.view` is nil. The new tests retain their source views, so they miss this path.

2. **[P2] Full reset discards the information needed to clean parked views** — [ContextMenusIOS.swift:387](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:387). Parked interactions are absent from `interactions`, so the preceding loop does not detach them. Reset then clears their companion records. [PresenterIOS.swift:354](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:354) calls `menus.reset()` before `pool.reset()`, meaning the subsequent `dropped` calls receive an empty companion set and leave the documented click-feedback interaction attached. A retained parked view therefore still carries UIKit companions after presenter/session teardown. Detach parked interactions before clearing their records. The added drop test calls only `p.pool.reset()` and checks only `UIContextMenuInteraction`, so it cannot catch this.

3. **[P2] None of the new pool tests run through the documented test command** — [ContextMenuIOSTests.swift:360](/private/tmp/x2-peek-review/host/apple/tests/ExactKitTests/ContextMenuIOSTests.swift:360). The tests belong to `ContextMenuPoolIOSTests`, but [build.mjs:1418](/private/tmp/x2-peek-review/host/apple/build.mjs:1418) derives class names from filenames and passes explicit `-only-testing` selectors. This file selects `ContextMenuIOSTests` alone. I executed that selection logic and confirmed the pool class is excluded. Consequently, the standard command and async lane run none of the new reuse, cleanup, or focus regression tests.

The round-1 pending-touch and focus-transfer fixes look correct. The new per-view drop hook fixes eviction, list-removal, and memory-warning cleanup; full reset remains incomplete as described above. Handler reassignment removes an unwanted `contextRecognizer`, and the per-node traversal handles nested and multiple menu nodes. I found no additional concrete tvOS regression.

The snapshot excludes existing host objects, and I found no in-tree callback that adds an unrelated object during `addInteraction`. Continuing ownership based solely on recycled object addresses remains unproven. Contrary to the disposition, `NodePoolIOSTests` contains no foreign-recognizer rejection test. The menu-ending test also invokes delegates directly; it establishes host-state cleanup, not actual UIKit dismissal.

`git diff --check` passed. UIKit tests were not built or run in this read-only sandbox. No files changed.

**Verdict: NOT READY.**
