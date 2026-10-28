# Moodboard build notes

Completed and installed as `dev.bakeoff.moodboard.b45fcc3` on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final standalone app was launched with `bun exact.mjs ios --run` using that simulator explicitly.

All requested features are finished. The 600 source tiles are bundled through the imported `data/tiles.json`; their original likes seed the library. Likes and deletions are saved atomically to the app's private JSON file. Board choices and each tab's viewer order also survive relaunch. Dates are formatted in UTC to preserve the supplied calendar day. Equal-date tiles retain their source order.

## What was hard

- The largest actual iOS accessibility size behaves differently from setting only the authored root font size. The full iOS verifier found tiles scrolling beneath pinned controls. Moving the board filter into the scroll and bulk actions into the native navigation bar fixed the overlap and colour-contrast issues.
- Emoji are artwork here, but text-based symbols triggered text contrast and fixed-size warnings. Twelve transparent PNGs rendered with AppKit preserve the supplied symbols, while the tile buttons provide accessible title, board, liked, and selection labels.
- An early asynchronous native build detected that artwork changed after its bake receipt was read. Rebuilding after asset generation resolved that guard.
- Native header recognition was clearer when the detail header was outside the tile data loop. Lookup functions supply its title and heart state.

## What I looked up or decided

I read Exact's start-here guide, started from its tabs pattern, and used its vocabulary and driving documentation for native navigation, menus, symbols, file persistence, grid layout, gestures, and confirmation alerts. Left/right navigation uses pan deltas with a per-contact threshold, preserves the filtered order, and stops at either end. Each tab retains its own viewer order; unliking in a detail does not abruptly remove the current tile. Deleting a tile open in another tab gives that detail a clear return-to-grid action.

Deletion is permanent and confirms first. Selection is temporary and clears on relaunch or a tab change. No requested feature was left unfinished.

## Verification

- `bun exact.mjs verify`: clean; 7 tests passed, no light/dark or largest-text layout findings.
- `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios`: clean; 7 tests passed, no light/dark findings, nothing spills at accessibility-extra-extra-extra-large.
- iOS scrolling verdict: “like UIKit.” Physics passed; the verifier declined to judge timing under high host load.
- Real UIKit touches exercised the board menu and multi-selection. Simulator drags verified both swipe directions.
- Tests cover likes/unlikes, persistence, bulk liking/deletion, cancellation, filtering, per-tab neighbors, selection clearing, deleting an entire board, and a detail deleted from another tab.

## What would have made this faster

A single bidirectional gallery-swipe recipe, a more obvious interactive command for the simulator's real content-size category, and a documented artwork-versus-text accessibility pattern would reduce discovery. The compiler's actionable diagnostics and the final web/iOS verifiers were useful throughout.
