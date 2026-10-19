# Review round 3, LLP 1121 stage 9 E (held rows name their tokens), Grok 4.7 (xhigh)

**RIGHT.** Round 1 and round 2 are in the tree, and this pass did not find a new correctness bug.

Rounds 1 and 2, as they stand now:

- A look clear is `{"value":"unset"}` with no token (`runner/src/agent/looks.rs:31-43`). The layout test locks that.
- An empty `data-view` is not node 0. `heldOf` uses `$exactViewId`, and `hold.test.mjs` expects 41 on the link.
- The join checks the held node's file, chain, and part files. The moved `style.contract` case is read-only.
- The JS held answer takes `instance` from the inspect table. A site the table does not have yields no path; a real empty path stays `[]`.
- `provenance: "incomplete"` is only when a `var(--x-…)` count exceeds the names `modes.token` returned.
- `hello` is `tamber/1.12` (`contract/cli/src/tamber_serve.rs:72`), including the guide's intro and the 1.12 bullet. Older 1.11 lines are history.
- A role override is named from `modes.js`'s table. A development build writes that as `var(--exact-canvas-text)` (the token is substituted to a bare `var()`, which is what `modes.test.mjs` locks). `--exact-accent` stays unnamed.
- Linux's child layout asserts the card's `node`, `site`, and `instance`. Both QUEUE entries are gone.
- `:active` rows rename through `AUTHORED`, matching `ACTIVE_ROWS` (`press_background_color` → `background-color`, and the other three). A direct press row has no `state` on the binding, so it keeps the wire name and the node's span. Look rows are not `Origin::State`, so they stay on the node.

Runner `looks` is valid JSON beside `style` (`hover` / `focus_visible`, codes 1 and 2). `look_token` uses the look's tuple and restores it. Hosts keep `active` to the five press rows and pass tokens through. `el` is non-enumerable. Touched sources are within 1499 lines (`host/web/glue.js` is exactly 1499).

No findings.
