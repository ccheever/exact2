# Native tab icon fallback checkpoint

## Rough

The compact and Duo tab contracts had portable icon tokens, but the UIKit tab bar assembled its own `UIImage(systemName:)` values. A symbol introduced after the deployment target returned `nil`; UIKit kept the tab item and silently rendered no glyph.

## Lean in

The native tab coordinator now tries the preferred and stable SF Symbols, then draws a small vector fallback for Chats, Pull Requests, Usage and Settings. The fallback remains template-rendered so UIKit owns tint and selected state.

## Needed

Drive an iPhone and Duo build once the device bridge is available. Verify all four glyphs, selected and unselected tint, and the rail placement on iOS 27.

## Checkpoints

- 2026-11-07: Added deterministic native fallbacks in `T3HomeChrome.swift`; source checks and the iOS no-install build are pending.
