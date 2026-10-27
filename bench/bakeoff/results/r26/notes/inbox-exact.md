# Inbox — build notes

Finished and installed as **Inbox** (`dev.bakeoff.inbox.b95e4ba`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final command was `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios --run`.

## What ships

- All 320 original messages from `data/mail.json`, embedded through the bundled data module. No network or sending capability.
- Newest-first Inbox and Archive, native navigation and search, unread indicators and count, unread/flagged filters, and flag indicators.
- Native right swipe to toggle read/unread; left swipe to archive, with Flag as another swipe action. Archive supports restoring to Inbox. Long-press menus and reading-screen controls offer the same actions.
- Opening a message marks it read. Read, flagged, archived, and locally refreshed messages persist in an atomic JSON file. All writes share a queue so overlapping refreshes and message actions cannot overwrite each other.
- Pull to refresh adds exactly two locally fabricated messages. Each refresh assigns unique IDs and timestamps newer than existing messages. A toolbar refresh button is also available.
- Native colors, typography, controls, dark mode, Dynamic Type, and a custom mail icon. Previews wrap naturally; accessibility sizes grow the rows instead of truncating text.

## Decisions and assumptions

The supplied data has a `preview` field but no separate body. The reading screen therefore shows the complete provided preview as the message content, alongside the sender, subject, and date/time. Refresh messages are intentionally fictional and local. The unread count refers to messages currently in the Inbox.

Filters use a native menu, with the active filter shown beneath the title. A segmented control did not fit at the largest text size. Empty filtered results can clear the filter/search; an empty Archive returns to Inbox.

## Validation

- `bun exact.mjs test web`: **8 passed, 0 failed**.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs test ios`: **8 passed, 0 failed**.
- Final `bun exact.mjs verify ios` on that simulator: **verify: clean**. First screen, eight test ends, and relaunch checked in light and dark with no findings. Ten screens checked at accessibility-extra-extra-extra-large (53-point body): nothing spills.
- Real simulator touches confirmed full swipes to mark unread, archive, and restore. A native pull to refresh changed 320 messages to 322 and 109 unread to 111.
- Relaunch tests cover saved read, flag, archive, and refreshed-message state, including repeated refreshes. Tests also cover filters, search, empty Archive recovery, and a stale message link.
- Native scrolling verification found nothing for the app to fix. The machine was heavily loaded, so Exact did not judge frame timing; it checked physics and reported its own status-bar scroll-to-top difference separately.

## What was hard / what I looked up

Exact required lookup for its date formatting modes, literal route names, conditional event handlers, icon declarations, and native swipe wiring. The initial compiler command needed access to the toolkit's build cache. The cold native build took several minutes; changing assets during prebuild invalidated its bake receipt and required another build. Incremental builds were fast afterward.

Native reload restarts at the Inbox, while web reload retains its route. The persistence tests were corrected to inspect saved records and reopen screens explicitly. UIKit search also retains its search presentation after clearing a query; tests must respect that native presentation rather than assume the browser's toolbar remains tappable.

The real-touch driver starts drags at a target's center unless given `from x y`. Full swipes needed offsets near the relevant edge to stay inside the viewport. A pull must start from the top and travel far enough beyond it; the shorter first attempt only scrolled back toward the top.

`bun exact.mjs verify` was run during development. Its web layout report includes hidden swipe-action coverage and menu-overlay findings in wide/max-text browser views. Native iPhone verification is clean; the browser version was not a deliverable.

## Unfinished work

No requested iPhone feature is unfinished. No other simulator was booted, erased, or used.

## What would have made this faster

A warm native toolkit cache, clearer guidance to finish assets/grants before packaging, a mail-style pattern with both swipe directions, and native test examples covering reload navigation and active search would help. Swipe documentation should show an edge-start `from x y` gesture. Web layout checks could better recognize deliberately hidden native swipe actions and menu overlays.
