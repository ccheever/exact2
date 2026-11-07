# 2026-11-07 — tab icon availability

## Rough

The iOS tab bar showed only Settings' gear while Chats, PRs and Usage fell
back to title-only items. The host asked UIKit for a single SF Symbol name;
when a symbol was unavailable in the installed OS symbol set, its image was
nil and the tab still rendered without a glyph.

## Lean in

The native tab and Duo rail hosts now resolve Expo's preferred symbol first and
fall back to a stable equivalent. Chats uses a bubble fallback, PRs uses the
pull-request triangle before the branch glyph, and Usage falls back to the
plain bar chart. The selected Chats item has the same fallback chain.

## Needed

A fresh iPhone and Duo drive should confirm all four tab items show glyphs and
that selected/unselected tint remains native. The local iOS app build compiled
the updated Swift module successfully.

## Checkpoints

- Focused workspace, geometry and scroll tests.
- Signed iOS app compile from this checkout.
- EAS TestFlight archive and submission from the icon-fallback commit.
