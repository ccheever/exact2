# Native Home tab rail

## Rough

The maintained T3 Code clone moved iOS Home from a single floating search/filter/compose pill to a native tab rail with a separate compose control. Exact's Home chrome still rendered the older arrangement.

## Lean in

The native boundary already owned the Home route and search events, so the change stayed in `T3HomeChrome` and the existing Home event projection. UIKit owns the tab bar geometry, selected state and accessibility tree; Contract only maps tab selections to existing routes.

## Needed

The latest upstream layout uses Chats, PRs, Usage and Settings tabs, with New chat beside the rail and Search exposed as a header action. Duo devices remain on the workspace split path, where their sidebar and system rail own presentation.

## Checkpoints

- Focused Home, native configuration and layout tests pass.
- iOS simulator build installs and launches on the fresh `t3-native-tabs duo` (iOS 27.2, iPhone19.4).
- Captured `/tmp/t3-mobile-parity/t3-native-tabs-duo-final.png`. UIKit reports the four system rail buttons at x=396 with 44x58 frames at y=410, 460, 510 and 560; New chat and Search remain native navigation-bar actions above the rail.
- Standard iPhone Home now uses the same `UITabBarController` owner, so UIKit centers and sizes its horizontal bar without app-authored offsets. Fresh evidence is `/tmp/t3-mobile-parity/t3-standard-native-tabs.png` (402x874); native tab selection drives PRs to Archived threads.
