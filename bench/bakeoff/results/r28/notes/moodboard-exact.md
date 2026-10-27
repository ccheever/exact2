# Moodboard

Built with Exact and installed as `dev.bakeoff.moodboard.ba212a3` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.

## Completed

- All 600 supplied tiles, sorted newest first, in a virtualized, full-width three-column grid.
- All ten board filters plus All Boards, using a native menu.
- Large tile view with title, board, full date, horizontal swipe navigation, and accessible previous/next controls. Navigation stops at the collection boundaries.
- Like/unlike, a Liked tab, multiple selection, select/deselect all within the current filter, bulk liking, and confirmed bulk deletion.
- Likes and deletions saved with atomic local file writes. An empty saved collection remains empty after relaunch.
- Native navigation and tab bars, dark mode, accessibility labels, selection feedback, empty/error states, and large-text layouts.
- Custom app icon. The original JSON is imported by the data module and also bundled unchanged at `assets/data/tiles.json`.

## Validation

Final `bun exact.mjs verify ios --sim EEFC0029-1010-45E4-8242-42CF67E16C96` was clean:

- 10 tests passed, none failed.
- No light/dark layout findings across the first screen, test end screens, and relaunch.
- Nothing spilled at accessibility-extra-extra-extra-large (53-point body text), across 12 screens.
- Relaunch persistence passed.
- Real-touch scrolling physics matched UIKit. The host was heavily loaded, so the toolkit did not judge comparative timing.

Also ran web verification, real simulator menu taps and swipe gestures, and manually inspected screenshots. Web-only large-text checks reported menu/background overlay intersections and a heading measurement issue; the target iPhone verification is clean.

After verification, ran `bun exact.mjs ios --sim EEFC0029-1010-45E4-8242-42CF67E16C96` to install the final standalone build rather than leaving only an agent-injected plan.

## What was hard

The first implementation exceeded Exact's 65,536 list-step evaluation budget through a quadratic membership check. Removing that check and deriving filtered collections once fixed it. Virtualizing groups of three keeps the complete collection responsive.

Exact's initial list styling inset the grid; explicitly authored list appearance and zero padding produced the required full-width tiles. A pinned multiline board summary overlapped the grid at the largest text size. Moving that summary into the scrolling list and the filter into the navigation bar fixed the issue.

The automatic contrast checker measures emoji as text. Each tile now supplies a luminance-based black/white fallback foreground while retaining the supplied HSL background and multicolour symbol.

## Lookups and decisions

Read the Exact start guide and looked up tabs, navigation, virtualized lists, grid tracks, menus, pan gestures, file grants, confirmation dialogs, and simulator driving. No outside product research or additional dependencies were needed.

Kept input order for tiles added on the same date because no tie-breaker was specified. Honoured all initial liked flags. Deletion is permanent and confirmed. Board filters and selection are temporary UI state; likes and deletions persist. Each tab retains its own browsing neighbours during a session, and unliking the tile being viewed does not abruptly dismiss it.

## Unfinished

None of the requested iPhone features are unfinished.

## What would have made this faster

A ready-made virtualized image-grid pattern, runtime evaluation errors pointing to Contract source expressions, and clearer guidance about pinned controls at accessibility text sizes would have shortened the iteration cycle. Web popover-aware overlap checks would reduce unrelated findings when targeting iPhone.
