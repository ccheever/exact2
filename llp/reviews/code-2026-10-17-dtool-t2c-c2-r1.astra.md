# Code review: LLP 1122 T2c step 2 (inspect on iOS), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `81e50b725`.
- **Method:** one brief (sha256 `36ed7a5afe3e61570da27ed9af3a7db8faed3cec81c0836c586784d9bc7039fe`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (6 high, 9 medium, 1 low).
- **Disposition:** taken, with review2's early notes (07:40Z) folded into the same fixes; the Mac half (step 3) joins this commit.
  - 1: the selection keeps its generation. The same generation keeps the id. A later one of the same plan finds the node again by site and instance path (exactly one), else it is said gone.
  - 2: an event goes on to UIKit only when all its touches began on the banner. A touch begun in inspect mode is owned to its end, even after inspect mode ends (`InspectHost.owning`).
  - 3: presses are taken (a hardware Escape leaves), and entry ends editing (the keyboard goes). Tested.
  - 4: entry cancels each gesture recognizer under way (off and on again) and each control tracking a touch.
  - 5: inspect is iOS only. tvOS answers `inspect` unsupported and names no capability; `--tvos --no-install` compiles.
  - 6: a window with no overlay gets one on its first event in inspect mode; an overlay whose window went is dropped.
  - 7: the heading is the header's node with `accessibilityRole` `heading`.
  - 8: the title box is the visible label showing the top route's `navigationItem.title`.
  - 9: each bar item made records its node (`itemNodes`, inspect builds only).
  - 10: a segmented control or native tab bar standing for a tablist answers its tab under the point (`SegmentHost.tab(at:in:)`). The tab controller's bar is read from its buttons in order (no accessibility client makes elements); tested both ways (a tab selected, the bar's space nothing).
  - 11: the grouped-list lookup runs only at a cell.
  - 12: outlines skip a view with a hidden ancestor, on iOS and the Mac.
  - 13: the local exit is kept (`Inspect.left`) until the server hears it; a replay of that command on registering again is the exit again.
  - 14: `refresh` with `status: true` sends the status again first.
  - 15: the report's budget is 60 KiB, under the server's 64 with the plan and nonce added.
  - 16: the inspect-only metadata (`groupId`, `shownHeader`, `itemNodes`) is behind `EXACT_INSPECT`.
  - Not taken as asked: the tests do not synthesize UIEvents (UIKit gives no public way to make a touch event); interception is proven end to end on the simulator (a real touch through `--touch platform`), and by the gate.

---

1. **High — Refresh can select an unrelated node after rebuilding.** [Inspect.swift:156](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/Inspect.swift:156) trusts an existing numeric view id. A new runner restarts id allocation, so that id can now identify another node; neither the saved plan digest nor session generation is checked. Store the generation, reject changed plans, and re-identify uniquely by site and instance only within the same plan.

2. **High — Touching the banner lets other touches reach the app.** [InspectIOS.swift:68](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:68) returns `false` for the entire event when any touch is inside the banner. Hold one finger there and tap an app button with another: `ExactWindow` forwards both to UIKit. Handle the close interaction within inspect mode, without forwarding mixed events; track touch ownership from its beginning.

3. **High — Keyboard input and presses remain active.** [InspectIOS.swift:65](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:65) intercepts only `.touches`, and entering inspect leaves the app’s first responder intact. Hardware Return can activate a focused button; an already-visible software keyboard can continue editing or submitting through `UIKeyInput`. Redirect focus to the inspector and gate app keyboard, shortcut, and press delivery.

4. **High — Entering inspect does not cancel existing gestures.** [Inspect.swift:125](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/Inspect.swift:125) only changes state and installs overlays. Start a long press, then enable inspect before its recognition deadline: the armed recognizer can still invoke the app. Swallowing subsequent releases also leaves tracking unfinished. Cancel existing recognizers/control tracking on entry and suppress intercepted touch sequences through their termination, including after closing inspect.

5. **High — Development tvOS compilation is broken.** [InspectIOS.swift:165](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:165) uses `.close`, and [InspectIOS.swift:120](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:120) uses `frame(in:)`, inside a guard including tvOS. The installed SDK explicitly marks both APIs unavailable on tvOS. Restrict the implementation and its dispatch/capability guards to iOS, or provide supported tvOS implementations.

6. **High — Newly attached windows bypass inspection.** [InspectIOS.swift:18](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:18) reconciles windows only when an inspect command arrives. Move a session to another `ExactWindow` afterward: it has no overlay, so `take` returns `false` and app input resumes despite `Inspect.on`. Detached windows also leave overlays and their display links retained until another command. Attach and remove overlays through window/session lifecycle hooks; missing overlays must not permit input.

7. **Medium — Plain titles select the header instead of its heading.** [InspectIOS.swift:135](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:135) checks `role` and `ariaLevel`, but the presenter stores `accessibilityRole` and `accessibilityHeadingLevel`. Consequently, ordinary titles fall back to `header.id`. Use the existing `isHeading` property or retain the heading selected by `HeaderRule`.

8. **Medium — Back text can be mistaken for the title.** [InspectIOS.swift:158](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:158) chooses the widest `UILabel` anywhere in the navigation bar. With a long Back label and short current title, tapping Back’s text selects the app header, violating the boundary rule. Identify the actual title presentation and exclude item/back labels and hidden ancestor subtrees.

9. **Medium — Authored bar items require an optional test id.** [InspectIOS.swift:121](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:121) recognizes only `BarPress` targets or a global `testId` match. Menu items have no target, and readout items likewise lack one; without `testId`, they are rejected as UIKit content. Repeated test ids can select another route’s node. Retain the node association on each projected item/custom view directly.

10. **Medium — Standalone native tab bars cannot select their tabs.** [InspectIOS.swift:144](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:144) searches only the navigation container’s tabs. A standalone symbol-and-label tablist projected by `SegmentsIOS.swift` into `ExactTabBar` therefore selects nothing. Resolve ownership from the particular bar encountered, including the segments presenter’s item mapping.

11. **Medium — Grouped-list lookup skips nearer nodes.** [InspectIOS.swift:108](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:108) calls `GroupedListsLink.part` at every step, but that helper walks all the way to its enclosing cell. A hit inside a custom row’s native text field therefore selects the row before reaching the field’s nearer `NodeView`. Consult grouped ownership only when the walk actually reaches that boundary.

12. **Medium — Outlines use hidden source geometry.** [InspectIOS.swift:46](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:46) resolves sites exclusively through `presenter.views`. Native tabs can be filtered out as hidden; headings and grouped-row descendants can pass because only their ancestor is hidden, producing boxes at invisible authored positions. Reveal can also scroll the hidden source container. Resolve each node to its visible projection and native scroll container, and check ancestor visibility.

13. **Medium — Closing while disconnected is undone on reconnect.** [Inspect.swift:143](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/Inspect.swift:143) forgets the local exit after attempting one report. With no nonce, that report is dropped; registration then replays the server’s same `on` command and reopens inspect. Remember the locally exited command number and resend its exit when replayed, as the web implementation does.

14. **Medium — Refresh ignores the requested status repair.** [Inspect.swift:123](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/Inspect.swift:123) ignores `status:true`. If a status POST fails while the stream survives, the server requests status followed by selection; this handler resends only the stale selection, leaving inspection rejected until another status change. Force a fresh status ahead of the refreshed selection, bypassing status deduplication.

15. **Medium — Trimming permits oversized wire reports.** [Inspect.swift:48](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/Inspect.swift:48) measures against 65,536 bytes before `reportSelected` adds `plan` and `PlanURL` adds `nonce`. A 65,500-byte report passes locally but exceeds the server’s limit and has its connection dropped. Measure the complete payload or reserve overhead, as the web’s 60-KiB budget does.

16. **Low — Inspect-only metadata remains outside the production gate.** [NavigationTitleIOS.swift:202](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:202) and [NavigationIOS.swift:24](/tmp/rv-dt-ios/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:24) compile `groupId`, `shownHeader`, and their assignments without `EXACT_INSPECT`. Guard these declarations and writes to satisfy the complete compile-out requirement.

The [new tests](/tmp/rv-dt-ios/host/apple/tests/ExactKitTests/InspectIOSTests.swift:41) construct ordinary `UIWindow`s and call inspect helpers directly. They do not prove event interception, banner handling, pending gestures, reconnects, refresh identity, outlines, or window cleanup. I found no unguarded `Inspect`/`InspectHost` type reference; channel delivery uses the main queue and retains the runtime gate. Builds and tests were not run under the read-only constraint.

**Verdict: Request changes—input isolation and selection identity are incorrect, and development tvOS compilation is broken.**