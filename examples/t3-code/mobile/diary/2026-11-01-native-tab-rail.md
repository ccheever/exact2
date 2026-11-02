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
- Captured `/tmp/t3-mobile-parity/final-iphone18.png` with the new rail and accessibility nodes for all four tabs plus New chat.
- The local Duo is occupied by another session, so its final screenshot must be captured by the owning session before claiming Duo image parity.
