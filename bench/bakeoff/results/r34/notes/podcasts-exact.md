# Podcasts — build notes

Completed and installed as `dev.bakeoff.podcasts.ba13e5e` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` with `bun exact.mjs ios --sim EEFC0029-1010-45E4-8242-42CF67E16C96`.

## Delivered

- All 420 episodes, newest first, with show, title, date, duration, and played status.
- Persistent Unplayed Only switch, live search, and empty-search feedback.
- All 12 shows with live episode and unplayed counts, and individual show lists.
- Episode details, summaries, Play/Pause state, and played/unplayed controls.
- Native episode action menus accessible from each list row and by long press.
- Persistent Up Next queue with duplicate prevention, drag handles, Edit-mode move controls, and removal.
- Native navigation, tabs, search field, switch, grouped lists, menus, haptics, dark mode, Dynamic Type, and app icon.
- Original `data/episodes.json` imported into the data module and shipped byte-for-byte as `assets/data/episodes.json` inside the app bundle.

## Validation

`bun exact.mjs verify ios --sim EEFC0029-1010-45E4-8242-42CF67E16C96` completed cleanly:

- Five feature tests passed, including played-state and queue persistence across relaunch, and changing an initially played episode back to unplayed.
- First screen, all five test ends, and relaunch checked in light and dark mode: no findings.
- Seven screens checked at accessibility-extra-extra-extra-large, body 53 pt: nothing spills.
- Real-touch list scrolling passed the UIKit physics check. The machine was heavily loaded, so the tool did not judge performance timing.
- Additional real-touch drives confirmed native UIKit menus, queue move buttons, and dragging a queue item.
- Bundle metadata and the shipped dataset were inspected. The original and bundled JSON share SHA-256 `e7a1c9a18c16f2f0a50fd8533767bf5b215002ab7bbf10dcfe9dd83188dbb855`.

The default app state was reset to the supplied library after testing, before the final installation.

## What was hard / looked up

- Learned Exact's action syntax, optional array indexing, tab routes, menu presentation, persistence limits, and virtualized reorder-list requirements from its patterns and vocabulary documentation.
- The shared Exact compiler cache initially needed filesystem permission. Native builds then worked normally.
- An initially unsized detail symbol expanded too far; screenshot inspection led to a rem-sized symbol that grows with text size.
- Disabling a menu item during its own press interfered with dismissal in the driver. The add action now remains idempotent and dismisses normally; duplicate additions do nothing.
- Browser checks reported hidden swipe-delete controls as covered in a sidebar. The target iPhone verification is clean.
- Exact's supplementary synchronous property explorer does not cover relaunch and some string methods; actual simulator feature tests cover those paths.

## Decisions where the specification was silent

- Shows sort alphabetically. Equal-date episodes sort by ID for a stable order.
- Played state is the supplied initial state plus a persisted set of changed IDs. The queue stores an ordered list of episode IDs; both fit comfortably within Exact's persisted primitive-list limit.
- Play immediately marks an episode played. Pause keeps it played. Playback itself is a temporary, single-episode UI state with no audio.
- Playing an episode leaves it in Up Next until the person removes it.

## Unfinished work

No required iPhone features remain unfinished.

## What would make this faster

A podcast/list pattern with native row menus and queue reordering, an example of symbol sizing in a detail card, a documented mutate-and-dismiss menu recipe, and clearer separation of browser-only overlay reports from native iPhone findings would reduce iteration time.
