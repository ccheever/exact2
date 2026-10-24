# Code review: tamber/1.16 inherited rows' fromAt and inspect select, round 1 of 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 1 of 3; HEAD `079ae5324b`. The author (Claude) is not a reviewer.
- **Verdict:** both: one BLOCKER (Apple hosts read a select frame as inspect off); with seven SHOULD-FIXes and NITs, all taken in a0fd4a2ae4.

I'll review the tamber/1.16 wire only: the HEAD commit, protocol shapes, and the fromAt / select-up path. Read-only, no tree changes.The commit is in. Next I’ll read the wire path: inspect scope, chain order, page select-up, and whether `fromSite` survives the join.One blocker: an `inspect` `select` frame turns inspect mode off on iOS and Mac.

1. **BLOCKER** `host/apple/Sources/ExactKit/Inspect.swift:131` (routed from `BuildChannel.swift:166`). `{kind:'inspect', select:{up, sites}}` has no `on` and no `refresh`, so `inspectCommand` takes `on` as false and calls `Inspect.set(false)`. That clears the remembered selection and `InspectHost.show` takes the overlay down, and neither side reports `selected: null` or `unsupported`. The server has already answered `{builds:[id]}` and still records that client as the inspector. The wasm page (`host/web/dev.js:914`) and the JS page (`host/web-js/inspect-dev.js:551`) are the only hosts that handle `select`.

2. **SHOULD-FIX** `scripts/tamber-server.mjs:796`. With a plan map and no stored selection, `sitesOf` evaluates `s.site` while `s` is missing and throws. The request catch turns that into **400** and a JavaScript message. The 1.16 clause says **409** when there is no selection to move. `selectUp`'s inspector check never runs, because `sitesOf` is first.

3. **SHOULD-FIX** `scripts/tamber-inspect.mjs:191` (published at `scripts/tamber-builds.mjs:504`). `scrubbed` keeps `fromSite`, so the client-visible `selected` event still carries it. `join` drops it on the `selection` rows (both early returns and the inherited-row destructure). Held rows do not carry it: the web glue and `HeldState.rows` copy only `name`, `value`, and `token`.

4. **SHOULD-FIX** `host/apple/Sources/ExactKit/Inspect.swift:64`. An untrimmed Apple report copies the runner row, so `fromSite` is present and `join` can name `fromAt`. Past 64 KiB the trim whitelist is `name`, `value`, `source`, `from`, `token`, and `fromSite` is gone. Both web pages keep it through that trim (`host/web/dev.js:642`, `host/web-js/inspect-dev.js:273`). `source::write` (`runner/src/agent/source.rs:37`) is the only node-row writer, so wasm, iOS, and the Mac all emit it before that trim.

5. **NIT** `host/web/dev.js:910`. Each ancestor is a full `node` op (every style row, tokens, looks, props, frames), and the walk always runs to the root rather than `up` steps. The `4096` cap stops a parent cycle, but a hit that already matched is then selected, so the cap can land inside the copy. The JS walk (`inspect-dev.js:305`) is a DOM parent walk and does not have this cost.

6. **NIT** `scripts/tamber-pairing.mjs:80`. Any `inspect` with `refresh: true` is `read`, and `inspect()` handles `select` before the below-write branch. `{refresh:true, select:{up}}` from a read client enters `selectRequest`. It still fails closed: a read client is not the inspector, so `selectUp` returns 409. A select without `refresh` is `write`, the same as `inspect` `on`.

Chain order is innermost first (`contract/cli/src/map.rs:269` emits the current call, then its parent). `sitesUp`'s `chain.slice(n)` is that ancestor's chain; past the last call the empty chain is the app's own nodes, and the page's outermost match is the root. Nested nodes of one copy share that chain, and both pages overwrite `found` while walking up, so the copy's root wins over an inner node of the same copy. `fromAt.file` is a `file` field, so `namedPaths` in `send` (`scripts/tamber-server.mjs:184`) and `remember` (`:254`) make it app-relative for every client, remote included. The editable reason uses only the basename.
