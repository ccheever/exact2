# Reconnecting cache wake

## Rough

The iOS simulator retained a valid shell cache with twelve projects and ninety-nine threads, but Home still showed its empty state while the saved environment was reconnecting.

## Lean in

The reconnecting transport has not named its environment yet, so the focused client remains empty during the first root snapshot. The cache fleet can still hydrate that saved identity as a disposable display. Its asynchronous hydration installed the display after first pixel without changing a canonical revision, leaving Home on the earlier empty projection.

Fleet hydration now bumps the focused render revision only when its display set changes. This wakes the root projection while preserving the transport client and keeping cached rows read-only until the connection names and synchronizes the environment.

## Needed

Run the rebuilt simulator with the saved environment offline and confirm cached Home rows appear during reconnecting launch. Native scroll traces with a populated Home remain needed for the parity benchmark.

## Checkpoints

- 2026-11-06: added a pending focused reconnect regression and a post-first-pixel revision wake; focused fleet and Home source tests pass.
