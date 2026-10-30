# Code review: the Mac window bar's read set, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `19aa31ba9c`.
- **Method:** one brief (sha256 `43ebea116b2c4000db28d6342b49a837fb0371a92b1cccb0ba0a079b1d770fab`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Round-1 items (1)–(8) are fixed in HEAD, and the tests that lock them are in `BarReadsMacTests.swift`. Two present-time colour paths still leave a stale bar, so this round does not approve.

Round 1, checked against the current source:

1. **Fixed and tested.** A first child with `semanticTag == header` is subtreed whether the rule takes it or not (`ToolbarReadsMac.swift:180-184`). `testAHeaderTheRuleRefusedIsStillReadAndTakenBackWhenItCanBe`.
2. **Fixed and tested.** `named()` records every view of that id, taken or refused (`ToolbarReadsMac.swift:151-156`). `testANamedViewThatBecomesAPopoverIsRead`.
3. **Fixed and tested.** `accessibilityLabelledBy` names are walked, and a `props` id that matches a watched name presents (`ToolbarReadsMac.swift:86`, `:164-166`). `testAnExternalLabelsWordsAreRead`.
4. **Fixed and tested for the record.** `shadowCheck` puts back `reads`, `inputs`, and `presents` (`ToolbarReadsMac.swift:209-217`). `testTheGuardLeavesTheRecordAsItWas`. The guard still runs `sync()`, so the live bar is the re-presented one when it returns; see finding 4.
5. **Fixed and tested.** `shadowState` covers search label, placeholder, enabled, and accessibility label; segment label, selection, tab title, and enabled; custom-view image hash; tint; tooltip; toolbar accessibility children (`ToolbarReadsMac.swift:231-259`). `systemColorsChanged` presents (`ToolbarReadsMac.swift:266-269`). `testSystemColoursPresentTheBar`.
6. **Fixed.** Back controls come from `chrome.named[name]` (`BackMac.swift:31-36`). No test asserts that lookup. The whole-page sort is gone from this path.
7. **Fixed and tested.** A `present` of `opacity` on a read node presents (`ToolbarReadsMac.swift:94-96`). `testAPresentedOpacityOnAReadoutPresents`.
8. **Fixed.** The unplaced comment is at `ToolbarReadsMac.swift:67-69`.

The rest of the read set holds. `declaration` carriers are subtreed whole. `visible` climbs `superview` and the record also climbs `authoredSuperview`, which is the chain `inert` uses. Invocation targets cover `opensFromTheBar`, `isMenuShaped`, `alerts.plan`, and `usesToolbarAnchor`. Native faces present through `Batch.controls`. Search and segment updates read the header subtree. An `.svg` op paints the node's layer; the item shows a symbol, a `MacBarFace`, or `node.image`, so ignoring `.svg` does not stale the bar. Mac zoom and root size arrive as style ops on the header text, which is in the record. Mac Bold Text is hard off, and there is no Dynamic Type notification on this host.

VERDICT: CHANGES

1. **MAJOR** — A dark-mode or Increase Contrast change leaves prominent tint, authored toolbar ink, and scheme-dependent titles on the colours sampled at the last present. `ExactView.viewDidChangeEffectiveAppearance` reports the scheme and does not present the bar (`ExactViewMac.swift:421-424`). `prominence` stores a concrete `backgroundTintColor` from `channels("accent_color")` (`ToolbarMac.swift:369-376`). `ToolbarInkMac.button` stores concrete title and symbol colours from `drawsDark` (`ToolbarInkMac.swift:9-27`). `MacBarFace` captures the light and dark channel pair, and the string from `accessibleText`, at init (`BarFaceMac.swift:18-28`). The window title and subtitle do the same (`ToolbarMac.swift:696-699`, `TextInteraction.swift:55-58`). `Agent.systemScheme` and `systemContrast` set `NSApp.appearance` (`AgentMac.swift:1293-1324`). When the dark bit is unchanged, the kernel scheme batch is empty (`kernel/src/motion/paint.rs:249-250`). A static `light-dark()` row stays in the style, so `concerns` does not see it. The present hook added this round runs only from `NSColor.systemColorsDidChangeNotification` (`ToolbarReadsMac.swift:266-269`).

2. **MAJOR** — A theme `.roles` batch does not present the bar, so a live `AccentColor` edit leaves the same baked tint and faces. The op carries no id (`host/apple/src/batch.rs:115-116`), `BatchOp.id` is then 0 (`ExactCore/Batch.swift:127`), and the default arm presents only a read node (`ToolbarReadsMac.swift:99-101`). `Session.apply` calls `applyThemeTint` (`Session.swift:1108`), which retints page nodes (`ThemeRoles.swift:134-145`) and does not call `toolbar.sync()`. `prominence` has already resolved the accent into `backgroundTintColor` (`ToolbarMac.swift:375-376`).

3. **MINOR** — Every skipped batch rebuilds `BarInputs`, and `canGoBack` calls `locationBeneath` whenever the pushed route has no authored Back control (`ToolbarReadsMac.swift:121`, `BackMac.swift:76-81`, `NavigationBack.swift:53-56`). That is an owner-thread hop (`Bridge.swift:646-651`, `Owner.swift:98-104`). On an idle owner it is one wake. While the owner is inside a job, main waits for that job. While main is already serving the owner, the call returns nil and the chevron can present disabled until a later batch reads a real location.

4. **MINOR** — The shadow can still miss, and it leaves a re-present on screen. `shadowCheck` restores the record only (`ToolbarReadsMac.swift:213-217`). While `shadowing` is set, `present` skips layout (`ToolbarMac.swift:706-709`), so `syncAccessibility` returns when the toolbar's accessibility children are not the visible items yet (`ToolbarMac.swift:759-766`). The accessibility lines in `shadowState` then compare the pre-layout tree. A miss that exists only there produces no mismatch, and any visual change `sync()` did make stays.

5. **NIT** — A `.language` op has no id, so it misses the record the same way `.roles` does (`ToolbarReadsMac.swift:99-101`). `applyLanguage` runs in `apply`'s defer, after the bar has already decided (`PresenterMac.swift:1056`, `DocumentLanguage.swift:9-16`). Paragraph edits of read labels still present. The Back item's label, tooltip, and symbol stay the ones set at creation (`ToolbarMac.swift:89-90`), and an empty search field keeps the placeholder captured when the item was made (`HeaderMac.swift:274-280`).
