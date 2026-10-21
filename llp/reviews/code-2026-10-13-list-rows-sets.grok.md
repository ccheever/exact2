# Code review: onlyListRows decides a batch's creates once, and QUEUE's one-pass entry, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `300ef82ad197f669a05bd912deb5272bab70e0443b239322dd043b543593a574`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** The MINOR (both: the QUEUE entry's "each non-empty one" overlooks `applySnapshots`' collections-only path and `applyUnlessEmpty`'s skip) taken in the landing commit.

---

**MINOR** — `QUEUE.md:828`

The new entry says each non-empty batch pays `Presenter.apply`'s fixed pass no matter what it holds, and names `NavigationHost.prepare` / `batchApplied`, `onlyListRows`, `syncAccessibility`, `ControlHost.sync`, and `CollectionHost` begin/end. A non-empty batch whose ops are all `.collections` never reaches those calls when nothing is waiting: `applySnapshots` (`host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:737`) runs `beginBatch` / `endBatch` and returns. That comment says this is one batch in two or three while a list fills. On the scroll path the gate is also not "non-empty": `applyUnlessEmpty` (`host/apple/Sources/ExactKit/Session.swift:975`) still calls `apply` for a batch that `changesNothing` while a fill, tick, or canvas is in flight, which is the same condition that makes `landFill` run inside that `apply` (`Session.swift:991`).

The rest of the entry matches the code and is plain. `landFill` (`Session.swift:638`) drains the owner's queued slices from inside `Session.apply` so they apply before that batch, which is the T4 order (`Session.swift:990`). Each slice that applies is its own `Presenter.apply`, so the calls above run again; one pass over those slices would run them once. Doing that has to keep that order. `navigation.prepare` (`NavigationIOS.swift:274`), `navigation.batchApplied` (`NavigationTabsIOS.swift:342`), `onlyListRows` (`PresenterIOS.swift:802`), `controls.sync` (`PresenterIOS.swift:991`), `syncAccessibility` (`PresenterIOS.swift:1027`), and `collections.beginBatch` / `endBatch` (`PresenterIOS.swift:752` and the defer at `777`) are all in that pass.

`onlyListRows` in `7114cff` is equivalent to the old dictionary.

- `created` still means "some `.create` in this batch used this id". The set is insert-only, as the dictionary never dropped a key.
- An id created more than once still resolves to the last `.create`: `createdProjected` inserts when `projected` is true and removes when it is false (`ListRowsIOS.swift:43`).
- The old `own` check in `inRows` (`ListRowsIOS.swift:83`) and the `.children` clause (`ListRowsIOS.swift:96`) both read that last-create bit. `created[id] == nil` is `!created.contains(id)` (`ListRowsIOS.swift:84`).
- `projected(_:kind:)` (`ListRowsIOS.swift:20`) only reads `props`, the unused `kind`, and `back` / `scrolls`, which are fixed just above the loop. It is safe to call on the first pass. The `.create` / `.props` arm still calls it on that op, unchanged.

The commit claims fewer copies and lookups per row. It does not claim a measured speed-up.

READY
