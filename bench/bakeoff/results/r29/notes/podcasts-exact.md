# Podcasts build notes

Finished and installed as **Podcasts** (`dev.bakeoff.podcasts.b2beaa3`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.

All requested features are implemented: 420 episodes newest first, an unplayed filter, 12 shows with live counts and individual episode lists, summaries and simulated playback, direct played/unplayed controls, and an Up Next queue with removal, drag reordering, and accessible Move Up/Move Down controls. Played changes and queue order persist across relaunches. Search is also included.

The original dataset remains in `data/episodes.json`. Its content is imported by the data module, and an identical raw copy is explicitly shipped in `assets/data/episodes.json`. The installed app bundle was checked for that file. The app works offline and has a custom waveform icon.

## What was hard

- Learning Exact's immutable expression syntax, option-based array indexing, and the distinction between grouped and virtualized lists.
- Making long episode titles and adjacent controls work at the largest text size. Rows now wrap their content and actions, and detail artwork grows with text size.
- Discovering that `reorderFor` names the list ID, rather than an episode key. An interaction-gallery example resolved this; drag reordering was then verified with real simulator touches.
- A segmented filter beneath the navigation bar overlapped scrolled rows and produced a dark-mode label finding on iOS. Moving the filter into the native navigation bar fixed both.
- Keeping native packaging and verification in sync. Changing bundled assets during a verification run invalidated that run; the final checks ran after a fresh build and installation.

## What I looked up or decided

Used Exact's start guide, tabs pattern, vocabulary, list/navigation/persistence documentation, iOS-driving instructions, and a drag-reordering example. Chose alphabetical show order, deterministic episode ID order for dates that tie, and UTC date formatting to prevent calendar-day shifts. Played state is saved as IDs that override each episode's bundled initial value; queue IDs retain their explicit order.

## Verification

- `bun exact.mjs verify`: clean; 8 tests passed. Light/dark checks at phone and wide sizes passed; largest-text checks across 18 screens found no spills.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: clean; 8 tests passed. Light/dark layouts and 10 screens at accessibility-extra-extra-extra-large passed.
- Real-touch simulator testing confirmed queue drag reordering. The final scrolling check reported UIKit-like physics. The verification tool declined to judge timing because the host machine was heavily loaded.
- Final native build and installation completed with `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios`.

Nothing requested remains unfinished. Playback deliberately has no audio, as specified.

## What would have made this faster

A complete reorder example in the `reorderFor` vocabulary entry, a clearer guide to native build freshness before `verify ios`, and a pinned-filter pattern explicitly tested after scrolling in dark mode would have reduced iteration time. Exact's native controls, persistent primitive lists, and combined functional/layout verification saved substantial implementation work.
