# Build notes

Implemented the seven requested features using Exact's native iOS navigation, lists, tabs, search fields, switches, and buttons. All 420 supplied episodes are bundled, and a byte-for-byte copy of the source JSON is included at `assets/episodes.json`.

## Decisions

- Episodes are sorted by date descending, then by numeric ID for a stable order on the same day. Date-only values are formatted in UTC so they cannot shift to the previous day.
- Initial played flags come from the dataset. Persisted overrides preserve subsequent played/unplayed changes. The ordered Up Next IDs, filter, and search text also persist.
- Up Next rejects duplicate additions. Edit exposes accessible Up, Down, and Remove controls; native swipe-to-remove also works. Playing does not automatically remove an item from this planning queue.
- Play and Pause show a simulated playing state, with no audio, as requested.

## What was hard / looked up

- Used the local Exact start guide, tabs pattern, component reference, navigation, swipe, persistence, asset, and Dynamic Type documentation.
- Exact's immutable array syntax requires slice/concat rather than indexed mutation. Its generated source types enforce exact resource shapes.
- Native symbol layout differed from the web preview. Rem-based bounds preserve appropriate size and accessibility scaling.
- Long title words needed room at the largest text size. Titles wrap by word and can hyphenate a word that is wider than the screen. Native text nodes ignore trailing visual whitespace, so the word layout also needs an explicit gap.
- iOS relaunch returns to the main tab. Persistence tests must reopen the relevant screen before checking its saved values.

## Validation

- `bun exact.mjs verify ios`: 9 tests passed; light/dark, relaunch, and the largest accessibility text size passed with no findings.
- A final targeted iPhone drive checked the last queue-title change, queue Up/Down/Remove controls, and queue/detail screens at a 53-point body size in both schemes: no findings.
- `bun exact.mjs verify`: 9 tests passed; light/dark, relaunch, and largest-text checks are clean. One earlier browser screenshot request timed out; the final retry passed.
- Native scrolling reported UIKit physics. The host was heavily loaded, so the tool did not judge timing; this is a testing limitation rather than a known app defect.
- Final `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs ios` succeeded. Podcasts (`dev.bakeoff.podcasts.b1ec9f3`) is installed on that simulator.

No requested features remain unfinished.

## What would make this faster

An app-local compiler cache would reduce sandbox permission friction. Native symbol sizing that follows its text style by default, a documented accessible list-reordering primitive, and clearer native-versus-web relaunch guidance would reduce iteration. The combined light/dark, large-text, and scrolling checks were useful.
