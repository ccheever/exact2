# Timetable build notes

Finished, built, installed, and launched as **Timetable** (`dev.bakeoff.timetable.bab969f`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` using Exact. No other simulator was used.

## What ships

- Whole-day departure board, sorted by scheduled minute and grouped into native pinned hour sections.
- Searchable station sheet, five-line filter plus All lines, initial/explicit Now positioning, and dimmed past departures.
- Stars and a Saved tab containing departure details and originating stations.
- Persistent station selection and an atomic, queued JSON store for Saved departures.
- Native navigation, menus, search/keyboard, sheets, tabs, adaptive row wrapping, light/dark colors, and a transit app icon.
- The original dataset is imported into the data module and also bundled intact at `assets/data/departures.json`. The supplied file contains **2,698** departures, 20 stations, and five lines; I preserved all of it rather than inventing records to match the specification's approximate count.

## What was hard

- File order starts in the morning and places after-midnight services near the end. Sorting before grouping was essential.
- Native collection views initially estimate offscreen row sizes. Now first reveals the destination, then aligns it again after the next commit. Initialization is guarded by route, station/filter identity, and a completed revision so sheets and tab changes cannot trigger a stale scroll.
- Whole-row opacity made line badges and secondary text difficult to read. Past times and destinations instead use an explicit accessible dimmed color. The source line colors use fixed theme tokens so dark mode does not change their identities.
- A custom station button needed an explicit 44-point minimum hit area. Rows wrap at large text sizes and move stars below the departure content when needed.
- Exact's frozen test clock needs `clock settle` to execute the one-millisecond initialization tasks. Asset changes during verification invalidate the native build, so the final asset set was built before the final runs.

## What I looked up or decided

I used the supplied Exact guide, tabs pattern, and vocabulary/recipes for lists and sections, scrolling, persistence, file grants, timers/local time, search, sheets, themes, assets, and simulator driving. I also inspected Exact's native list/scroll implementation to confirm UIKit supplies plain-list header pinning and accounts for pinned headers during alignment. No external service or web research was needed.

Decisions where the specification was silent:

- Display scheduled times in compact 24-hour format.
- Treat the file as a daily timetable in the device's local time zone.
- Preserve delayed/cancelled statuses verbatim, and retain cancelled services in the schedule and Now ordering.
- Interpret Now as strictly after the current time; do not add a delay to a service's scheduled position.
- Keep Saved services across all stations, globally ordered by scheduled time. Use immutable source indices as stable identifiers, including for otherwise identical records.

## Validation and remaining limitations

- Final `bun exact.mjs test ios`: **9 passed, 0 failed**, covering whole-day grouping, station search/selection/relaunch, empty search, line filtering, saving/removal/relaunch, cross-station Saved details, end of service, local midnight rollover, and the current-minute Now boundary.
- `bun .exact/check-data.ts`: checked every departure, every station, all 100 station/line combinations, unique identities, original field preservation, and saving/removing all 2,698 records.
- Ran both `bun exact.mjs verify` and `bun exact.mjs verify ios`. All feature tests passed. Native scrolling measurements reported **nothing for the app to fix**, including no dropped lift frame and UIKit-like decay physics. The shared machine was busy, so the toolkit did not judge absolute timing.
- Manually drove real simulator touches through scrolling/pinned headings, Now, and station search with the keyboard. Inspected light/dark screenshots and the largest body size (53 points); the board passed both light/dark checks after Now at that size.
- **The broad automated visual reports are not completely clean.** The last native report retained 29 geometry findings (26 at the largest text size), involving content under native glass/pinned headers and offscreen cell frames; the web report retained five extreme-text geometry findings. Several offscreen stars are reported at the same position even though they belong to different native cells. I did not hide these with broad suppressions. One specific station-control overlap is documented as intentional native glass underlap in `app.json`. Reports and screenshots remain in `.exact/verify/`, `.exact/verify-ios/`, and `.exact/verify-web/`.
- No required functional feature was left unfinished. The source is a static timetable, not a live feed.

The final build was installed and launched with:

```sh
EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios --run
```

## What would make this faster

A departure-board example combining native pinned sections, measured-row Now alignment, and a persistent Saved collection would save substantial discovery time. A shared compiler cache writable by the normal build process would avoid initial sandbox escalation. The verifier could distinguish intentional glass/header occlusion and native offscreen cell projections from real content overlap, especially after changing Dynamic Type size.
