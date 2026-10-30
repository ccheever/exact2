# Code review: the Mac window bar's read set, round 3 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8915e61041`.
- **Method:** one brief (sha256 `508423039276796f4a1d9d9dd9caa2f1e4b1d3d7377425030a06bf0abe9d9a8f`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The six round-2 fixes hold in the current tree, and the skip path does not walk the page. Each one is covered by an unguarded test in `BarReadsMacTests.swift` (the guard is turned off inside the test, so it cannot paper over a miss).

1. A paragraph run whose `id` is a name the bar resolved is a hit when that name is added (`concerns` looks at the run props) and when it is removed (the paragraph owner was recorded, so the next paragraph op hits). `testARunTakingAButtonsNameIsRead` takes and drops `commandfor` `"guard"`.
2. Every `toolbarPlacement` carrier’s shown and inert bits are in `BarInputs`. A second navigation root re-selecting hides the declared toolbar with no op on a node the bar read. `testASecondNavigationRootHidingADeclaredToolbarPresents`.
3. The viewport’s effective appearance name is an input, and `viewDidChangeEffectiveAppearance` presents immediately. Increase Contrast is a different appearance name (`accessibilityHighContrastAqua` / `DarkAqua`); the present rebakes `ToolbarInkMac` and the `MacBarFace` / `BadgeFace` channels from the current contrast. `testTheWindowTurningDarkResolvesTheBarsInkAgain` covers the empty-batch appearance path for a `light-dark()` title tint.
4. An op with no node presents, except the listed bookkeeping kinds. `.roles` and `.router` present once. `.language` presents inside `apply`, then `languageApplied` presents again after `applyLanguage`. `testOpsThatNameNoNodePresent` expects that second present.
5. The skip check uses `backFacts`. `locationBeneath` runs only when the bar actually presents or AppKit validates Back.
6. The shadow compares the label and enabled state `syncAccessibility` would write, and it does not force layout. On a mismatch it keeps the old record, leaves the new pixels, and fails under XCTest.

A list-sized batch is still a pass over that batch’s ops and the chrome index (carriers, header routes, Back controls). It does not walk the page, and it does not call the runner. Menu rows are filled from the live popover when the menu opens, so a skipped present does not freeze them.

VERDICT: APPROVE

No findings.
