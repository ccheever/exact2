# Code review: the anchored chooser on iOS (402d9d87d), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `402d9d87d`. A first run under `--permission-mode plan` ended after one sentence; this is the second.
- **Method:** one brief (sha256 `b8f5f7abb7df69098b9487d12488b1699dbcbc32c7674fb1ed3625078ccc3b42`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:**
  1. *Does not compile.* Argued. `NodeView` is a `UIView`, so an `NSObject`, which is `Equatable` by `isEqual:` (identity for views). The commit built, and 224 XCTests ran. The checks now compare by identity anyway, as the finding suggests.
  2. *Every alertdialog gains a title.* Fixed; a real regression. The sheet is titled by `aria-label` only when it is a chooser: more than one action and no text row. Block, Discard, Recover, Purge and Delete keep no title row, and the existing confirmation test asserts it. LLP 1021 "The chooser" says so.
  3. *A disabled row blocks the chooser.* Fixed, as astra's #2.
  4. *Tests.* Partly fixed. The refusal test's comment no longer claims it reads the logged reason: this presenter has no session. The raster path of `image(of:)` is still tested only through `MenuHost.rowImage`. A raster lease in a hostless test did not deliver within 60 s, and that loader wiring is unchanged by this commit.

1. **Blocker** — `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:452`. `actions.contains($0)` and `cancels.contains($0)` do not compile: `NodeView` is a class and is not `Equatable`, and `Array.contains(_:)` is only available for `Equatable` elements. The iOS host and the new XCTests fail to build, so the sheet never presents. Compare identity instead: `contains(where: { $0 === child })`. Do not add `Equatable` based on view id; a restarted session can reuse an id for a different control.

2. **Should-fix** — `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:71` (title stored in `Confirmation.init`) and `:461`. Every alertdialog now takes `UIAlertController`'s title from `accessibilityLabel`. Block Contact's popover label is `"Block Contact"` and its message is the explanatory text, so the sheet gains a title row, shows "Block Contact" again above the button of the same name, and is no longer the title-less sheet the earlier geometry work matched. Discard (`"Discard new contact"`), Recover, Purge, and the delete dialog (`"Delete selected messages"`, which has no text row) change the same way. Pass the label as the title only when the popover has no text row (the chooser). Leave the title nil when a text row already supplies the message.

3. **Should-fix** — `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:456`. If any press row fails `eligible` (disabled, inert, or not live), `openConfirmation` returns false and does not log. A chooser whose data includes one disabled provider — three rows, one `disabled` — opens nothing, and the console does not get `confirmation <id> refused: …`. The menu path would dim that row. Present the sheet anyway, with `UIAlertAction.isEnabled = false` for the disabled rows (or omit them). Log only when nothing left can be chosen.

4. **Nit** — `host/apple/tests/ExactKitTests/NativeContextsIOSTests.swift:245`. `testAChooserTheSheetCannotPresentIsRefused` says the sheet "says why" for two cancels or a non-row, but it only builds two cancels and asserts `activate == false`. That was already the result when the host required exactly one cancel, and this presenter has no session, so `session?.log` is nil. The same file at `:291` checks a hand-built `MenuHost.rowImage`, not `image(of:)` reading `img.raster`, so a broken raster wiring still passes. Point the presenter at a session that records logs, assert `confirmation c refused: …`, and build one raster-backed row through `items(of:)`.

Verdict: DO NOT LAND
