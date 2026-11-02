# Native Home tab rail

## Rough

The maintained T3 Code clone moved iOS Home from a single floating search/filter/compose pill to a native tab rail with a separate compose control. Exact's Home chrome still rendered the older arrangement.

## Lean in

The native boundary already owned the Home route and search events, so the change stayed in `T3HomeChrome` and the existing Home event projection. UIKit owns the tab bar geometry, selected state and accessibility tree; Contract only maps tab selections to existing routes.

## Needed

The latest upstream layout uses Chats, PRs, Usage and Settings tabs, with New chat beside the rail and Search exposed as a header action. Duo devices remain on the workspace split path, where their sidebar and system rail own presentation.

## Checkpoints

- Focused Home, native configuration and layout tests pass.
- iOS simulator build installs and launches on bake-b2 (iOS 27.2).
- Captured `/tmp/t3-mobile-parity/iphone18-host-aligned.png` after porting Expo's absolute 128pt native host: the host is bottom anchored, the tab bar reserves the compact 64pt compose slot, and New chat uses the measured row height plus 8pt. The accessibility tree reports the Home chrome at y=784..874, the tab bar and New chat both at y=791..853, and header Search remains native.
- The two iPhone Duo simulators and the spare `exact-chat duo` simulator are occupied by other agent-device sessions; this change therefore has iPhone 18 visual evidence only until a Duo session is released.
