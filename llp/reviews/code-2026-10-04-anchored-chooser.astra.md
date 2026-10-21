# Code review: the anchored chooser on iOS (402d9d87d), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `402d9d87d`.
- **Method:** one brief (sha256 `b8f5f7abb7df69098b9487d12488b1699dbcbc32c7674fb1ed3625078ccc3b42`), shared with grok. Round 1, blind to the other review. Requested by Charlie. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all three fixed.
  1. *Changed arguments under an old title.* Fixed. The owner records each action's title and enablement as presented. If either changes, or the row leaves its popover or stops closing it, the sheet ends. Tested in `testAChooserWhoseRowChangesItsTitleCloses`.
  2. *One disabled choice blocks the chooser.* Fixed. A disabled action is presented with `isEnabled = false` and refused at selection. The sheet is refused, and logged, only when every action is disabled. Tested in `testAChooserIsASheetWithAnActionPerChoice`.
  3. *The test bypasses UIKit's handlers.* Fixed. `testAChoosersEachActionPressesItsOwnRow` runs each `UIAlertAction`'s own handler, for the first action, the third and the cancel. It skips explicitly, never passing silently, where the test host has no window scene (as on this Mac's simulator run). Real selection was driven on the simulator: `tap open-in-sheet`, then `tap open-in-sheet-google`, set `opened` to `google` once.

Static review only; no edits, builds, or tests run.

1. **should-fix — [MenusIOS.swift:236](/tmp/chooser-review/wt/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:236): A retained row can dispatch changed arguments under its old title.** A chooser row keyed by a stable slot can change from `name="Apple Maps", providerId="apple"` to Google’s data while retaining its `NodeView`. The alert still displays “Apple Maps,” but validation checks only identity, eligibility and containment; `press(action.id)` evaluates the current arguments and opens Google. Capture and revalidate the presented action binding, and invalidate the sheet when its displayed choice changes.

2. **should-fix — [MenusIOS.swift:456](/tmp/chooser-review/wt/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:456): One disabled choice silently prevents the entire chooser from opening.** With Apple Maps enabled and another provider disabled, `actions.allSatisfy(eligible)` returns false before presentation, without calling `refuse`. The enabled option and Cancel become unavailable, and the promised diagnostic is absent. Separate presentation validation from selection eligibility: represent disabled choices with `UIAlertAction.isEnabled = false`, reject their dispatch, and log any remaining unsupported conditions through `refuse`.

3. **should-fix — [NativeContextsIOSTests.swift:239](/tmp/chooser-review/wt/host/apple/tests/ExactKitTests/NativeContextsIOSTests.swift:239): The chooser test bypasses the callbacks whose binding it claims to verify.** Calling `menus.activate(p.views[5])` directly supplies the correct node to `finish`; none of the new `UIAlertAction` closures executes. The test would still pass if every UIKit action dispatched the first provider—or dispatched nothing. Exercise actual UIKit selection for each choice and Cancel, including reset between selection and deferred dispatch. Use an explicit skip when no scene is available instead of silently passing without exercising selection.

LAND WITH FIXES
