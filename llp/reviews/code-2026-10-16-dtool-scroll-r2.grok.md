# Code review: a selection's outlines and scrolling (LLP 1122 T2b follow-up), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f287163e1`.
- **Method:** one brief (sha256 `d14b247ad907e8b78e24f23b990c3acf2f5fc04ba555f59d6a11201830d75f04`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 medium, 1 low).
- **Disposition:** taken, but the tests' finding in part.
  - Only a pointer selection stamps (`select(id, true)` from pointer-up); a refresh does not.
  - The stamp is the selection's site and plan and its time. Only an outline on that plan whose sites include that site, within 10 s, takes it, and that one scrolls nothing, even if the copy clicked has gone meanwhile. Any other outline (the tool's, an unrelated one, an empty one) leaves the stamp and scrolls its first copy in, per D4.
  - Leaving inspect mode or a `clear` drops the stamp. A report that is not a selection no longer clears another press's stamp. A refused report's stamp expires after 10 s.
  - In part: the theme fixture has no repeated copies, so the e2e still counts `scrollIntoView` calls on one copy (none for the click's outline, one for the tool's). The three-copy feed case was seen in the gate on the clone.

---

The click path is right only when that copy is still mounted and its outline is the next frame that reaches the scroll line. A refresh, a frame that is not that answer, a refused report, and an unmounted copy all share one flag, and the test never builds the three-copy case.

**Medium** — `host/web/dev.js:635`. `inspecting.clicked` is cleared by the next `outlineSites` that gets past the early returns, and the scroll is skipped only when that frame's mounted copies include the element. The selection's outline and the tool's Outline are the same `{kind:'outline', sites, plan}` (`scripts/tamber-builds.mjs:335`).

Click the second mounted copy of an author name. `select` sets `clicked` and queues the report (`host/web/dev.js:489`). Before that POST is answered, an outline already in flight is applied: the previous click's outline, or the tool's Outline of a token that node uses. That frame takes the flag. When the clicked view is among its copies, it does not scroll, so D4's first-copy scroll is dropped. The outline that answers this click then runs with the flag clear and calls `found[0].scrollIntoView`, which is the feed jump. A `{sites:[]}` frame (locations that map to no site, `scripts/tamber-server.mjs:545`) does the same. `{clear:true}` (`host/web/dev.js:626`), a plan mismatch (`host/web/dev.js:629`), and inspect already off (`host/web/dev.js:627`) return without clearing the flag. A stale report is the same hole from the other side: the server returns 409 and sends no outline (`scripts/tamber-server.mjs:193`), or it publishes a selection with no source and sends none (`scripts/tamber-server.mjs:205`). `report` never reads the status (`host/web/dev.js:347`), so `clicked` stays that view id and the next tool Outline whose copies include it does not scroll. Leaving inspect clears `selected` only (`host/web/dev.js:594`).

Fix: stamp the report pointer versus refresh, and echo that on the one outline frame that answers it. Skip `scrollIntoView` only for the pointer frame, including when the view is already gone. Leave every other frame on D4's `found[0]` scroll. Drop the stamp when inspect turns off and when that report's POST is refused or dropped.

**Medium** — `host/web/dev.js:489`. The flag is set in `select`, and `refresh` always re-enters `select` (`host/web/dev.js:512`), including after a generation change re-finds one copy (`host/web/dev.js:510`).

Select a copy, scroll the feed until it is outside the port, then send `inspect {refresh:true}` (the test does this at `scripts/tamber-server.test.mjs:524`). The server outlines that selection. The page treats the outline as a click and does not scroll, so the copy stays off screen. D4's "scrolls the first into view" does not hold for that outline. The same follows a rebuild: the copy `refresh` rediscovers may not be the one that was under the pointer.

Fix: set the stamp only from the pointer-up that called `select` (`host/web/dev.js:533`). A refresh's outline keeps `found[0].scrollIntoView({ block: "nearest" })`.

**Medium** — `host/web/dev.js:632`. The clicked view is disconnected before the outline, which a live list can do during the report round trip; inspect gates input, and the app's timers still run. `el?.isConnected` drops it, `views.get(clicked)` is not in `found`, and `found[0].scrollIntoView` runs on another copy. The feed jumps to the first site-order copy.

Fix: the pointer selection's own frame does not scroll, whether or not that view is still in `found`. A missing element is still that answer, not a signal to scroll a different copy.

**Medium** — `scripts/tamber-server.test.mjs:482`. The new assertion counts `Element.scrollIntoView` calls after clicking the single, already-visible "Swap pins" text (`apps/theme-fixture/app.contract:25`): `0` for its outline, `1` after the later tool Outline (`scripts/tamber-server.test.mjs:501`). The pre-fix line called `scrollIntoView` on that one element, so this fails the old code here. It stays green for a check of only `found[0]` against the clicked element. That is the three-copy failure: two mounted copies, the site-order first outside the port, click the other, and `found[0]` is not the node under the pointer. Nothing in the test sends a refresh, a `clear`, a foreign plan, or a 409, and nothing disconnects the clicked view or reads a scroller's `scrollTop` or `scrollLeft`. The tool half does outline several `surface.card` views and only checks that the call count became 1.

Reload holds. The flag is not restored (`host/web/dev.js:668` restores the inspect sequence only), and registration replays inspect and the pin, not an outline (`scripts/tamber-builds.mjs:227`), so a new document does not inherit the decision.

Fix: mount two copies of one site, put the site-order first outside a nested scroller, click the other, and assert that scroller's offsets stay put and that `scrollIntoView` is not called. Then assert the tool's Outline calls it on that first copy. Add a refresh once that copy is off screen (must call) and a stale report that produces no outline, followed by Outline (must call).

**Low** — `host/web/dev.js:489`. A report whose kind is not `selected` assigns `clicked = null`. Two presses overlap (`void select` at `host/web/dev.js:533` is not serialized). The one that returns `unsupported` clears the flag the other press set, and that press's outline scrolls.

Fix: only the press that set the stamp may clear it, and only its own answer or a refusal of its own report.

Verdict: Request changes — the one-shot flag suppresses whichever outline arrives next, a refresh re-arms it so an off-screen copy stays put, a disconnected clicked copy still scrolls the first site, and the assertion never mounts two copies.
