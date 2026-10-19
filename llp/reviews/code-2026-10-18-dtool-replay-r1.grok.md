# Code review: the hold replay by call-site chain (LLP 1122 D8, stage 8 E), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `175aecb882`.
- **Method:** one brief (sha256 `1ed0774ffd27ab3069124106ca7aa37fae75b2141988c0be8e6328e6bce5efb9`), shared by both reviewers, blind to the other review; commits d058feff01. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high; smaller points).
- **Disposition:** taken in `710b805698`: correspondence by copies, one copy a call-site chain whose sites (a `button "…"` and its label) are kept in order; the same calls in both plans take the same chain; as many copies whose calls moved take the same rank only where each pair is of one shape and the copies share calls alike, so a regrouped nest is not ranked onto another; otherwise the copy whose calls did not move, so a call added or removed beside a held one keeps it; calls moved onto each other's lines go by rank. The listing doc now says `held` is its frame's. Not taken, recorded in the protocol doc: a slot filled at two outlets is one copy, its outlets told apart by their order; with copies added or removed, a line another call now has is taken as unmoved.

**[P1] A `button "…"` hold clears when another call is added or removed — `scripts/tamber-inspect.mjs:316`**

`button "Post"` is two plan nodes at one span: the parser stamps the label with the button's span (`contract/syntax/src/parser/view.rs:197`), and both nodes carry that instance's call chain (`contract/cli/src/map.rs:198`). Two PostButtons are four nodes, and the two that belong to one call have the same chain, so `exact.length` is 2 and the function does not return. Rank then counts every node at that span with that chain shape. Deleting the other call makes that count 4 against 2; adding a third makes it 4 against 6. The hold on the call that did not move comes back null, and the server clears it.

The one-node model in `scripts/tamber-inspect.test.mjs` keeps that call (`correspondingSite(was, plan(4), 1)` is 1). The same edit on a button-plus-label pair returns null. A lone pair whose only call moved still matches, and so does a pair when every call stays and only lines shift. The miss is an unmoved call beside a sibling that appeared or disappeared. A grouped list has the same shape: the synthetic column reuses the list's span (`contract/lower/src/grouped.rs:617`).

Rank those exact-chain nodes by themselves, in site order, when both plans have the same number of them. Leave the shape-wide rank for chains whose lines actually moved.

The rest of the ruling holds. An unchanged rebuild re-holds each copy. A line inserted above every call re-holds by rank, including a PostButton nested in a Card called twice (the inner call is shared, so order follows the outer call). A call in another file is a different shape and is not taken. List rows are one site — the compiler map gives the `for` items `a` and `b` the same node (`contract/cli/tests/it/source_map.rs:173`) — and `replayHold` keeps the instance path, so one row is not replayed as the other. Equal counts still cannot tell a call that moved from one that was deleted and replaced; that is the rank rule as written.

Clearing `held` on every new hold frame matches the clients. The design tool and its stub already treat a listing's `held` as null until the build reports on that frame (`apps/design-tool/holds.ts:8`, the stub sets `held: null` when it records a hold). The agent CLI's `hold-state` talks to the host and never reads the listing. Nothing there still depends on the old retention.

Smaller: `docs/tamber-protocol.md:177` still calls `held` the build's last report. The Holds section (`:986`) says it is null until a report names the new frame. Those two sentences should agree. The server test still only replays a node with an empty chain and a unique line, so it does not catch the pair.
