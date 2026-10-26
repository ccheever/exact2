# Podcasts build notes

Completed and installed as **Podcasts**, bundle ID `dev.bakeoff.podcasts.80332d`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. No other simulator was used. The final app was launched with `bun exact.mjs ios --run`.

All requested features are implemented: 420 episodes sorted newest first, an unplayed filter, searchable episodes, 12 shows with live counts and show-specific episode lists, episode summaries, simulated Play/Pause, list-level played toggles, and an Up Next queue with reordering and removal. Played overrides and queue order persist across relaunches. The original data is imported by the data module and also shipped verbatim at `assets/episodes.json` in the app bundle.

## Validation

- `bun exact.mjs verify`: clean; seven tests passed.
- `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios`: clean; seven tests passed.
- Native screens checked in light and dark mode and at accessibility-extra-extra-extra-large Dynamic Type (53-point body). No layout findings.
- Real UIKit touches exercised episode navigation, Play, adding to Up Next, and tab navigation.
- Confirmed the packaged Info.plist has the exact requested name and bundle ID, and the bundled JSON contains 420 episodes.

## What was hard and what I looked up

Exact's manifest schema rejected the required bundle ID because its final segment starts with a digit. Apple permits this identifier. `bunfig.toml` loads `tooling/exact-schema-compat.cjs`, which corrects the schema's bundle-ID pattern locally and carries that correction into Exact's Bun subprocesses. The shared SDK and generated Exact launcher were left untouched. Normal `bun exact.mjs` commands work with this project-local compatibility fix.

I used the Exact start guide and tabs pattern, then looked up native switch syntax, semantic symbols, navigation, persistence limits, bundled assets, and simulator driving. The initial starter fingerprint required `pattern tabs --force`. Largest-text verification exposed heading and long-word sizing; semantic text styles and flexible rows resolved it. Native relaunch returns to the first tab, so the persistence test explicitly reopens Up Next before checking its contents.

## Decisions and remaining work

Queue reordering uses accessible Earlier/Later buttons in Edit mode. Queue items can be removed with a native swipe or the Edit-mode remove button. Equal-date episodes use ascending ID as a deterministic tie-breaker. Dates are formatted in UTC to prevent a local time-zone shift from changing their calendar date. The played and queue identifier lists are small enough for Exact's native persisted state limits.

Nothing in the specification remains unfinished. Play/Pause intentionally simulates playback without audio, as requested.

## What would have made this faster

A bundle-ID validator matching Apple's supported syntax, documentation commands that do not need a writable shared build lock, a built-in native list-reordering primitive, and an explicit note that native relaunch resets the selected tab would have saved the most time. Exact's native navigation pattern and combined functional/dark-mode/Dynamic-Type verification were useful.
