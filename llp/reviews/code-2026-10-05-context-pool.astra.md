# Code review: context-menu rows in the iOS node pool (LLP 1021 §5.1), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `bc885b72e`.
- **Method:** one brief (sha256 `d1e1aaeaa90e36845c4323188aaeed9f8d2ab77315a974480b10176f8eed44e1`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken: at rest is `.possible` with no touch (`numberOfTouches == 0`), for the long press and UIKit's recognizers alike. 2 taken: a set-aside focus is returned only to the node it was taken from, checked by its incarnation and its id, so a view lent to another node gets none (test). 3 taken: a parked tree that leaves the pool (eviction, its list gone, memory pressure, `reset`) takes its interaction and its records with it (`dropped`, test); the host's `reset` clears its parked views. Fixing these found one more, also taken: UIKit leaves a `_UIClickPresentationFeedbackGenerator` interaction on a view when only the menu's interaction is removed (measured), which kept such a view out of the pool for good; the host now removes the interactions recorded beside its own (`detach`), tested.

---

1. **[P2] `.possible` does not mean a recognizer is idle** — [NodePoolIOS.swift:352](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:352). UIKit explicitly allows this state while evaluating touches, including a long press waiting for its duration. A row with only `contextmenu` has no `pressed` guard protecting that contact. If a data update replaces it, the pool can retain the recognizer and rebind its target view while the gesture is pending; `openContext` then reads the new node’s ID. Require no tracked touches as well as `.possible`, or explicitly cancel pending recognition before reuse.

2. **[P2] Set-aside focus can transfer to another node** — [NodePoolIOS.swift:329](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:329). Focus row A, then open a menu on row B. `MenuFocus.setAside()` resigns A but retains its view weakly. A now passes the pool’s focus checks and, with this change, its context-menu objects also pass. If A is retired and reused as C, [MenuFocus.restore():86](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:86) quietly focuses C: it checks neither the original incarnation nor node identity. Protect held focus during retirement, or invalidate restoration when the incarnation changes.

3. **[P2] Eviction and memory-pressure cleanup leave companion records behind** — [ContextMenusIOS.swift:157](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:157). Parking removes the interaction from `interactions` but leaves its entry in `recognizers`. Subsequently, [NodePool.drop():149](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:149) only forgets views and removes the root. Neither eviction, list removal, nor a memory warning clears those records. The weak `parkedViews` table does not clear the separate dictionary, so companion metadata can accumulate beyond the pool’s capacity until the presenter resets. Dropping a tree needs corresponding menu-host cleanup.

The snapshot excludes pre-existing objects, and I found no concrete in-tree path adding an unrelated object synchronously between its two reads. However, storing only `ObjectIdentifier`s does not establish continuing ownership if a companion is removed and its address reused; that case is untested.

The per-node traversal covers nested and multiple menu nodes, handler reassignment removes an unwanted `contextRecognizer`, and the new interaction APIs are guarded off on tvOS. The added tests cover immediate root-row reuse, but invoke delegates directly: they do not exercise actual pending gestures, UIKit dismissal, pooled focus restoration, eviction, or removal of the handler.

`git diff --check` passes. UIKit tests could not be built or run in the read-only sandbox.

**Verdict: NOT READY.**
