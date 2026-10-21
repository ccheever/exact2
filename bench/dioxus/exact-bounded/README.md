# Heavy list — exact2, bounded answer

The heavy list (`../../heavy-list/SPEC.md`) on exact2 with LLP 1027.004's bounded answer, built from the checkout
this directory is in. Bundle id `dev.exact.heavybench.bounded`. It is `../../heavy-list/exact-heavylist` with the
list's resource changed; the rows, the Contract below the list and the DataSource's feed, reactions and live mode
are the same.

- `resource feed = feed(cursor)` answers at most 200 messages around a cursor (a message id; empty is the top).
  The virtualized list's `reachstart` / `reachend` move the cursor to the window's first or last row.
- `react` and `tick` answer `{ ok }` through `mutation changed as shape Ack refreshes feed`, so neither answers the
  whole 10,000-row feed (on the web, a whole-feed answer costs 80–96 ms per tap; this one 16–24 ms).
- The DataSource parses its copy in `activate` on wasm (after the first pixel), not on the first window shift.
- `web/` is the web build's crate; `apple/` the static library. `data/messages.json` and `assets/` are copied by
  `../prepare.sh` (not committed), as is `Cargo.lock` (the root's, resolved by the first build).

The trade: the scrollbar spans the window (200 rows), not the feed, and an offset jump cannot reach row 9,000;
the heavy list's device series keeps the whole-feed app for that reason.

```sh
bench/dioxus/prepare.sh
EXACT_APP_DIR=$PWD/bench/dioxus/exact-bounded EXACT_IDENTITY=- bun host/apple/build.mjs exact-bounded-apple --run   # macOS
EXACT_APP_DIR=$PWD/bench/dioxus/exact-bounded bun host/apple/build.mjs --ios exact-bounded-apple --run              # simulator
PART=bounded bench/dioxus/web/build.sh                                                                              # web, production
EXACT_APP_DIR=$PWD/bench/dioxus/exact-bounded bun bench/dioxus/native/shiftcheck.mjs macos                          # forward-only shifts
```
