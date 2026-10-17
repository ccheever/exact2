# Code review: a selection's outlines and scrolling (LLP 1122 T2b follow-up), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `6929efc40`.
- **Method:** one brief (sha256 `0ab64d8ca20ba1279a6042b152c469ceed74b1354c75d0d2e3adee8bfc286764`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 1 low).
- **Disposition:** in part; this was the last round. The single click and its own outline (the gate's case) hold, and any frame not on the stamped site keeps D4's scroll. Two cases are left, because closing them needs the outline frame to name the report it answers (a token through `selected`, the server's join and `outline`), which is a wire change for theme2 to decide:
  - another outline of the same site arriving while the stamp is live (a second click, or the tool's Outline of a token that node uses) takes the stamp first, so the click's own reply scrolls;
  - a click report that draws no outline (409 with no retry, no map) leaves the stamp for up to 10 s, and a tool Outline of that site in that window does not scroll.
  The worst outcome of either is the old behaviour (a scroll) or one missed scroll. The e2e still counts calls on the fixture's single copy.

---

A single pointer selection, followed by the outline the server sends for that report, leaves the page where it is. After that frame is applied, the tool's Outline calls `scrollIntoView` on the first mounted copy. The stamp is set only from a pointer selection (`host/web/dev.js:490`, called at `:534`); a refresh calls `select` without it (`:513`); leaving inspect clears it (`:605`); a reload does not restore it (`:669` keeps only the inspect sequence). A frame for another plan returns at `:630` with the stamp unchanged. An outline that lists the stamped site skips scrolling even when that view is already disconnected (`:633`–`:639`).

**Medium** — `host/web/dev.js:637`

`ours` is any outline on the stamp's plan whose `sites` contain the stamp's site, inside 10 s, and that frame clears `inspecting.clicked`. The selection join and the tool's Outline are the same `{kind:'outline', sites, plan}` (`scripts/tamber-builds.mjs:335`).

Double-click the later copy of one site. Each `pointerup` calls `select(id, true)`. Both `selectedReport` calls finish in the page, and both reports are posted, before either outline comes back. The first frame takes the stamp and does not scroll. The second finds the stamp empty and calls `found[0].scrollIntoView`, which scrolls the feed to the first copy in site order. The same split happens when a tool Outline whose locations include that site is applied while the stamp is still set: the tool frame skips D4's scroll, and the selection's frame then performs it. `{kind:'outline', clear:true}` (`host/web/dev.js:627`) drops the stamp as it passes, so a selection outline still queued behind that clear scrolls too.

Fix: give each pointer report a token and keep the outstanding set. Copy the token onto only the outline emitted for that report (`scripts/tamber-server.mjs:205`). Skip `scrollIntoView` only for a frame whose token is outstanding, including when the clicked view is gone, then retire that token. A refresh that re-posts the selection re-sends the same token. Every other frame, the tool's Outline included, keeps `found[0].scrollIntoView({ block: "nearest" })`.

**Medium** — `host/web/dev.js:490`

The stamp is set before the POST. `report` drops the response on the floor (`host/web/dev.js:347`), including a registration that never accepts the body (`:346`) and the 10 s abort. A stale report is 409 with no outline (`scripts/tamber-server.mjs:193`). A join with no source publishes the selection and sends none (`:205`). The stamp then stays armed until the 10 s elapse. The next tool Outline that lists that site takes it and does not scroll, so D4's first-copy scroll does not hold for that Outline. The 409 path that does join is sound: `refresh` re-posts the same site, that outline matches the stamp, and the page stays put. The hole is the report that never draws an outline.

Fix: retire the token when that report settles with no outline on the way (the node is gone in `refresh` at `host/web/dev.js:510`, a 409 that does not ask again, a POST that never reaches the server). A retry that will re-post the token keeps it.

**Low** — `scripts/tamber-server.test.mjs:482`

The new assertions count `Element.scrollIntoView` after a click on the single on-screen "Swap pins" text, then after a later tool Outline: 0, then 1 (`:501`). That fails the old unconditional call on this fixture. It stays green for both holes above. The fixture has one copy of that text; the test waits for its outline before sending the tool Outline; nothing double-clicks, clears, or sends another plan while the stamp is live; nothing forces a 409 or a missing map; nothing disconnects the view; nothing reads a nested scroller's offsets. The reload later in the test checks that inspect mode comes back.

Fix: mount two copies of one site, put the site-order first outside the feed port, click the other, and assert that port's `scrollTop` and `scrollLeft` stay put. A second click before the first outline returns must still not scroll. A tool Outline that includes the site, sent before the selection outline, must call `scrollIntoView` on the first copy, and so must a 409 that is followed by no outline.

Verdict: Request changes — one pointer selection's own outline stays put, but a second frame for that site still scrolls the first copy, and a report that draws no outline suppresses the tool's Outline for 10 s; the test covers neither.
