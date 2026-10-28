# Inbox — build notes

Completed and installed as `dev.bakeoff.inbox.b97459c` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final build is running on Inbox. No other simulator was used.

## Implementation

- Programmatic UIKit app delegate, scene delegate, navigation, self-sizing table cells, and scrollable message reader. No storyboard or SwiftUI.
- The original 320-message dataset is shipped unchanged as `Resources/mail.json` and copied into the application bundle.
- Newest-first mail, unread indicators and count, leading read/unread swipes, trailing archive/flag swipes, inline flag buttons, context menus, unread/flagged filters, and an Archive screen.
- Opening a message marks it read. Reader toolbar actions support archive/restore, flag/unflag, and marking unread.
- Pull-to-refresh adds two locally generated unread messages. IDs and dates remain unique and ordered even on repeated refreshes or if the clock moves backward.
- State and local arrivals are saved atomically in Application Support after each mutation. Failed writes roll back the mutation. Archive excludes messages from the Inbox unread count. Restore and archive undo are included.
- Search covers sender, subject, and preview. Keyboard layout guide, return-to-dismiss, and drag-to-dismiss keep the list usable while searching.
- Adaptive light/dark colors, Dynamic Type, accessibility labels, and haptics. At accessibility text sizes the filter becomes a menu and message dates move below sender names.
- A locally drawn envelope app icon is included.

## Validation

- `Tests/main.swift`: 28 passing checks covering the dataset, sorting, read/unread, flags, archive/restore, filter interactions, search, repeated refreshes, persisted arrivals, invalid IDs, failed-write rollback, and corrupted-save recovery.
- Simulator UIKit checks: 17 passing checks exercising table contents, filters, reader navigation/read state, the inline flag control, archive, undo, restore, empty search, swipe configurations, and the actual refresh handler/spinner lifecycle.
- A separate simulator process launch verified that read, flagged, archived, and refreshed-message states survived relaunch.
- Simulator screenshots inspected for standard light mode, dark unread mail, the largest accessibility text size, the reader, empty Archive, and the search keyboard. Images are in `QA/`.
- UIKit checks invoke the same controls and action methods used by the interface; physical swipe gestures were not driven by a touch automation service.
- Final production compilation and installation succeeded without warnings; its Info.plist passes `plutil -lint`. The conditional simulator test harness is excluded from this final build.

Run `Tests/run.sh` to reproduce the automated checks and reinstall the production build. Normal build command:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

## What was hard / decisions

- Simulator service access needed execution outside the default command sandbox. All simulator operations remained restricted to the supplied UDID.
- The largest text size exposed truncated segmented labels and crowded sender/date rows. Those now use adaptive layouts. Decorative reader initials use a fixed font so they fit their avatar.
- A table background empty state initially overlapped the scrolling header. Moving it to a sized table footer fixed the overlap.
- The supplied data has previews but no separate body or recipient address. The reader displays the complete supplied preview as its content and labels the recipient as “You”; it does not invent additional original-message text.
- Refresh messages use rotating fictional local examples. Filters apply to arrivals, so an active flagged filter continues to show flagged mail only.
- No external libraries, services, or web lookup were required. UIKit API compatibility was verified by compiling against the installed SDK.

## Unfinished / faster next time

No specified feature remains unfinished. A simulator touch/accessibility automation command would make direct gesture and VoiceOver traversal testing faster. An initial UIKit scene-manifest template and a ready-made simulator screenshot/test runner would also reduce setup time.
