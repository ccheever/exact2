# Code review: context menus with a preview and a commit (LLP 1021 §5.1), round 4, confirmation of the landed 7a1ac6826, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `7a1ac6826`.
- **Method:** one brief (sha256 `a67041dbdcdd20c848a1d620553b0f2cf78246739c259f99d09878770a53b926`), shared with grok: review the landed commit, confirm each taken finding of rounds 1 to 3. Blind to grok's round-4 review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Round 4 was asked for by the lead after landing, to confirm the unreviewed round-3 fixes; the fixes below land with the focus fix (MenuFocusIOS) and were verified by the UIKit tests and a simulator recording, not by a fifth review. 1 taken: the non-navigating invalidation path now syncs, so the held interaction goes and the recognizer comes back (test: a press that clears its source's popover). 2 taken on both web targets: the timeout rechecks that the node is connected, enabled and not inert. 3 taken: the providers recheck the source's eligibility after its action (`current`).

---

1. **[P2] A non-navigating commit can leave the source’s long press disabled.** [ContextMenusIOS.swift:260](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:260). If the preview’s press clears `contextPopover`, the round-3 interaction hold skips removal during the synchronous batch. This branch then restores the row and clears `open` without calling `sync()`. Subsequent `willEndFor` also returns because `open` is nil. The obsolete interaction remains installed and the source’s ordinary `contextmenu` recognizer stays disabled until another batch. Cleanup must reconcile the held interaction here too.

2. **[P2] Both web targets still open for a source its action disables or inerts.** [rt.js:727](/private/tmp/x2-peek-review/host/web-js/rt.js:727), [glue.js:431](/private/tmp/x2-peek-review/host/web/glue.js:431). The new eligibility guards run before the node’s own action; the timeout never repeats them. With the normal opener-first registration order, an action that synchronously disables the source or makes its ancestor inert still reaches `showPopover`. I reproduced those calls using both actual listener bodies. This needs eligibility revalidation inside the timeout and is separate from the deferred-commit deferral.

3. **[P2] iOS providers also accept a source made ineligible by its action.** [ContextMenusIOS.swift:147](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:147). Configuration checks `menus.eligible(source)`, but after `fire(owner)` the providers only check liveness and the unchanged popover name. An action that disables or hides its source therefore still returns the menu and lifts the preview. The later commit guard protects only the preview press. Recheck source eligibility before either provider supplies content, as macOS and the agent now do.

I checked every taken disposition. Apart from these remaining eligibility holes and the interaction-cleanup regression, the taken fixes appear correct by inspection, including all four round-3 Astra fixes. The recorded deferrals remain justified.

`git diff --check`, both presence-loader tests, and web syntax checks passed. Listener checks ran in memory; native tests were not run in this read-only checkout. No files changed.

**Verdict: NOT READY.**
