# Feed — build notes

Finished, built, installed, and launched as `dev.bakeoff.feed.b7985fa` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A` with `bun exact.mjs ios --run`.

All requested features are implemented. The app bundles the original 400 posts, the image-dimension index, and all 120 images under `assets/data/`. No network is used. A queued mutation writes likes and locally added posts atomically to app storage. New posts carry absolute timestamps; a persisted anchor lets the supplied relative times age across launches.

## What was hard

- Making nested horizontal carousels coexist with a virtualized vertical feed, post navigation, and independently tappable images and likes.
- Removing an extra native list inset while keeping full-width images and correct aspect ratios.
- Giving large text enough room without truncating paragraphs, author names, or carousel titles. Author rows wrap, action rows wrap, and all interactive targets have appropriate touch areas.
- Getting gesture tests right: native pull-to-refresh needed a sufficiently long downward pull, and carousel drags had to stay inside the screen.
- A timestamp test initially asserted an exact millisecond value that depended on platform animation timing. It now checks that the displayed age advances and stays correct after relaunch.

## Lookups and decisions

Used Exact’s required guide and list-detail pattern, then looked up asset bundling, file grants, virtualized lists, native routes and sheets, fullscreen presentation, pan gestures, refresh controls, and `exactTime`.

Sorted the supplied posts by `minutesAgo`, with original IDs resolving ties. Refresh creates three text posts locally, rotating through six invented updates. The compose sheet rejects whitespace-only text. Photo viewing uses a dark fullscreen route, with both a Close button and downward-swipe dismissal.

## Validation

- `bun exact.mjs test ios`: **8 passed, 0 failed** on the specified simulator.
- `bun exact.mjs test web`: **8 passed, 0 failed**, with no compiler warnings.
- Native `verify ios`: light/dark layout checks found no problems; largest accessibility text size showed no spills. Its one failing assertion was the platform-specific timestamp assertion described above, subsequently fixed and confirmed by the complete iOS test rerun.
- Native scrolling physics matched UIKit. The benchmark declined to judge performance timing because the host machine was heavily loaded.
- Real simulator touches confirmed pull-to-refresh, horizontal carousels, opening photos, downward dismissal, and composing with the software keyboard visible.
- Audited all bundled references: no missing images or dimension records.
- Web verification passed normal-size layout checks. At the largest web font size, its checker flags the intentionally partially visible next carousel card; the native accessibility checks reported no spill.

## What would make this faster

A ready-made local feed pattern with bundled media, carousels, persisted timestamps, and gesture tests would help. Clearer documentation of native list insets and the web host’s lack of pull-to-refresh would also save time, as would recognizing partial carousel previews in the web layout checker. Starting the native prebuild early was useful.
