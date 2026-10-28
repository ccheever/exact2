# Moodboard build notes

Implemented the complete supplied specification with Exact: a three-column square grid, newest-first order, All plus ten board filters, native tabs and navigation, a large tile viewer with horizontal swipes and accessible previous/next buttons, persistent likes, a matching Liked grid, and selection with batch like, unlike, and confirmed deletion.

The original 600 records are imported by the app and also shipped verbatim at assets/data/tiles.json. Tile backgrounds use their original HSL values. Twelve transparent PNGs rendered from the system emoji font preserve the supplied symbols as graphical content. The app icon is generated locally.

## What was hard

- Exact caps list evaluation work. A naive intersection of 600 neighbour IDs with 600 live tile IDs exceeded that budget. Replacing nested list membership with a delimiter-wrapped string index fixed full-collection navigation while still skipping deleted tiles.
- Graphical emoji rendered as text triggered typography and contrast checks. Rendering them as bundled transparent graphics removed those inappropriate text constraints and keeps the three-column layout stable at the largest text size.
- Checking the largest text size required wrapping the filter summary and selection controls and using compact icon actions for batch editing.
- A source/asset edit made during native verification invalidated its build receipt. A complete rebuild followed by an unchanged verification run was needed.

## Lookups and decisions

Read Exact's start-here guide, started from the tabs pattern, and used Exact vocabulary lookups for grids, menus, persisted files, pan/panrelease, assets, components, and native driving. No external web research was needed.

Newest-first compares the supplied ISO dates; records on the same date retain their supplied order. Each viewer captures its opening grid order, so unliking the tile being viewed in Liked does not unexpectedly jump to a different tile. Deleted neighbours are skipped. Switching back to a tab whose open tile was deleted returns to that tab's grid. Likes and deletions are applied as queued atomic writes to app:/data/library.json.

## What would make this faster

A ready-made virtual image-grid/detail pattern, a documented set-membership primitive for large collections, a graphic-symbol element distinct from text, and clearer separation of native-menu overlay checks from background-content overlap checks would help.

## Verification and completion

Finished and installed as Moodboard, bundle ID dev.bakeoff.moodboard.b106018, on simulator 486FD498-0E3E-4B64-8E4B-152BD8265EE7. No other simulator was used. The final `bun exact.mjs ios --run` completed and launched the installed app normally.

Final `verify ios`: clean. Nine native functional tests passed; the initial screen, nine test endings, and relaunch passed light/dark checks. Eleven screens passed accessibility-extra-extra-extra-large (53-point body) checks with nothing spilling. Both swipe directions were also exercised directly with real simulator gestures. Tests cover likes/unlikes surviving reload, bulk changes, cancellation, board filtering, bounded first/last neighbours, deletion of a tile open in the other tab, and deleting the entire collection.

Scrolling verification found nothing for the app to fix. The shared host was very busy, so Exact judged scrolling physics rather than performance timing. It also reported a framework-owned status-bar scroll-to-top timing difference. There are no unfinished specification features.

The complete final verification output and unchanged property receipt are saved in `.exact/verify/final-ios-report.txt`. Screenshots and scrolling measurements are under `.exact/verify/`.
