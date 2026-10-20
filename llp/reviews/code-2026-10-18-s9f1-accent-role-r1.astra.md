# Code review: LLP 1121 stage 9 F1, AccentColor under an authored accent on the painters and iOS, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the F1 commit.
- **Method:** one brief (sha256 `dcd021d4b128e2a6084a957856edfde9d07e56aefac4f3ede5cb445f79bf846f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (Android's retained rows; iOS across presentations; the ancestor's appearance; the lifted ghost).
- **Disposition:** fixed in round 1's commit.

---

Nothing blocking found. Four **should-fix** findings from static review; no builds or tests were run.

1. **Should-fix — Cached Android rows can retain the old accent.** [host/linux/src/paint/control.rs:159](/tmp/rv-f1-1/host/linux/src/paint/control.rs:159)  
   Put spinner rows inside a scroller declaring `accent-color="AccentColor"`, beneath an ancestor whose accent changes from green to red. The first recording paints green, but inheritance invalidation stops at the scroller’s declaration ([txn.rs:1425](/tmp/rv-f1-1/kernel/src/txn.rs:1425)). Retained-row validity never checks the newly consumed outer accent, so Android can keep replaying green. **Fix:** track the effective accent in retained-row validity or propagate its paint invalidation through `AccentColor` declarations. Add a recorder test changing only the ancestor’s accent.

2. **Should-fix — iOS loses the authored accent across presentation boundaries.** [host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:77](/tmp/rv-f1-1/host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:77)  
   A green ancestor containing a dialog that declares `AccentColor` works before reparenting. Once the dialog moves into its sheet, the next spinner sync cannot find that ancestor through `superview` and falls back to the design/platform colour. Presented routes and lifted popovers have the same problem. **Fix:** follow logical ancestry using the existing sheet, popover and navigation parent mappings. Test after presentation and a subsequent sync.

3. **Should-fix — iOS resolves the kept accent using the ancestor’s appearance.** [host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:53](/tmp/rv-f1-1/host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:53)  
   With a light ancestor declaring `light-dark(green, red)` and a dark descendant declaring `AccentColor`, the spinner receives green: `authoredAbove(...).channels(...)` defaults to the ancestor’s traits. The painter resolves the same value against the spinner’s dark appearance and gets red. **Fix:** resolve the ancestor’s colour value using the owner’s appearance, contrast and elevation. Add a mixed-scheme regression test.

4. **Should-fix — Isolated ghost painting starts without the required ancestor scope.** [host/linux/src/paint/control.rs:159](/tmp/rv-f1-1/host/linux/src/paint/control.rs:159)  
   A grouped reorder row containing a spinner beneath `AccentColor`, with green authored outside the row, paints green normally but loses it while dragged. Ghosts paint after the root walk has dropped its scopes ([lift.rs:148](/tmp/rv-f1-1/host/linux/src/paint/lift.rs:148)); opening the ghost’s role-valued scope preserves `None`. **Fix:** seed isolated subtree painting from the logical ancestors’ effective accent. Test the spinner before and during a grouped drag.

Both direct callers of `authored_accent` execute inside `node()`’s scope; the gaps concern missing ancestry and cached results. Role-valued scopes resolve without recursion because `painted_accent` temporarily clears the scope. iOS bars and other controls already use the owner’s tint resolution; Mac progress tinting is explicitly unsupported by D9. The added tests cover initial ordinary nesting, leaving the cases above uncovered.
