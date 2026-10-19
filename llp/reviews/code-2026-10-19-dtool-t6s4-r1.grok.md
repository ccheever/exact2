# Code review: the design tool drops the canvas grant on C2's entry word, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI (--max-turns 400 --no-plan), reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `4bcd6789bd`.
- **Method:** one brief (sha256 `c4c4b4273fb689afaf53725361ee1463113e086f5109c63a7da1d9f573612093`), the plan inline, shared by both reviewers, blind to the other review, with a security pass; commit 4bcd6789bd. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (2 SHOULD-FIX).
- **Disposition:** taken in the round's fix commit (named in round 2's records): the word counts only from the frame the tool shows (its window, where the page can find it: not a window the canvas opened) and the minted canvas's origin (Astra 1); once entered, the grant-bearing URL is dropped from memory too, a non-secret key kept (Astra 2); whether it entered is asked per key (client, server run, tick), so a Reload before its mint lands reads not entered (Astra 3); a mint a newer ask has overtaken is kept nowhere (Astra 4, Grok 2); each canvas key gets its own 40 polls, a new server run included (Astra 5, Grok 1); tests for the overtaken mint and the per-key word.

Nothing blocks. The origin check is exact, a forged word aborts entry, and it does not hand the grant to the sender. Two correctness holes are worth fixing.

## Security

The listener is on the tool window (`apps/design-tool/pairing.ts:48-51`). It accepts a message only when `data.type` is the string `exact-canvas-entered` and `event.origin` is `new URL(minted.frame).origin`. That comparison is exact string equality against the URL origin (scheme, host, port). There is no prefix match, no path check (a `postMessage` origin has no path), and `"null"` does not match an `https` origin. The browser sets `event.origin`; the sender cannot pick it. A message from the tool's own origin, from an opener on the door's origin, or from `https://evil.example` is ignored. The unit test covers that case.

Who can still say the word: any window whose origin is the canvas origin and that has a handle to the tool window.

- The framed document can. After the entry POST, the gateway burns the grant and answers 303 to `/` with `__Host-exact-canvas` (`scripts/tamber-canvas.mjs:370-383`). The document then in the iframe is the app, same origin as the entry page, and `parent.postMessage` passes the check. A nested frame on that origin can reach `top` the same way.
- A popup the app opened can too (`sandbox` includes `allow-popups` and `allow-same-origin`, `app.contract:1413`). The popup survives a later navigation of the iframe and can keep posting. `opener.parent` is the tool window.
- Another tab on the canvas origin has no handle to the tool window, so it cannot deliver a `message` event there. An opener whose origin is not the canvas origin fails the equality.

What the word does: it sets `entered` to the current `minted.frame` (`pairing.ts:50`). The next poll, within about a second while polls remain, makes `loadCanvas` return `minted.root` (`pairing.ts:68`) and the iframe `src` becomes `https://<canvas-host>:<port>/`. No reply is sent. The grant is not copied into the message.

If that happens before the entry POST, the navigation tears down the entry document and `GET /` is 404 with no session (`tamber-canvas.mjs:411-413`). The attempt is dead until Reload Canvas mints again. A popup that keeps posting can do that to every new grant. That is a broken entry, and the handler does not disclose the grant. A page on the canvas origin can already read the fragment while the iframe is on the entry URL; that is the entry page's own channel, not a new one.

After a word that follows a successful POST, the grant leaves the places an agent reads:

- `canvasAt.frame` and `canvasAt.root` become `origin/` (`pairing.ts:68`). Agent `state` is the contract resources (`host/web-js/agent.js:464`). The tree's iframe url is the `src` attribute (`agent.js:79`, `app.contract:1413`).
- There is no `canvasSettled` slot.

Until that poll (at most one interval, and only while `canvasPollsLeft > 0`), `src` and `canvasAt.frame` still hold the entry URL. The word is defined to come after the POST, and the gateway deletes the grant before the 303 (`tamber-canvas.mjs:370-371`), so that copy is already burned.

Two module strings keep the full entry URL for the life of the page: `minted.frame` and `entered` (`pairing.ts:46`, `50`, `73`). They are not contract state. Once they match, the cache returns `minted.root`, so they are not written back into `canvasAt`.

## Correctness

One grant per `(client, epoch, tick)` is stored only after `canvasFrame` accepts the reply (`pairing.ts:70-74`). A transport error or a rejected URL leaves `minted` unchanged, so a failed mint is not pinned. The runtime will not re-ask the same arguments; Try Again and Reload Canvas both run `reloadCanvas`, which increments `canvasTick` and sets `canvasPollsLeft` back to 40 (`app.contract:852-854`). Both buttons are disabled while `pending(canvasAt)` (`app.contract:1410`, `1418`), so a double click does not overlap the mint. A new tick is a new key and a new grant, and `entered === minted.frame` is false because the new fragment differs (`pairing.ts:78`).

The poll is 40 armed waits of 1000 ms (`app.contract:686`, `822-823`, `849-851`), which covers the grant's 30 s (`tamber-canvas.mjs:58`) with margin. The watch drops its timer while `canvasIn` is pending and re-arms when the gate becomes true again, including after Reload. It stops when the frame no longer contains `#g=` or the budget hits 0. There is no resource cycle: `canvasIn` depends on `canvasTick` and `canvasPoll` only; `canvasAt` reads `canvasIn.entered`; the watch is the only edge back, and an unchanged `entered: false` does not re-run `loadCanvas` (arguments compare equal, `host/web-js/rt.js:494`).

A server restart mid-entry, with polls still left, follows the new epoch. `hello.epoch` is part of the key, so `loadCanvas` mints again. The in-memory grants and sessions of the old process are gone (`tamber-canvas.mjs:508-512`). A word already stored for the old URL does not match the new `minted.frame`. `canvasIn` does not depend on epoch, so its `entered: true` can sit until the next poll; the mint path ignores that argument and still returns the new entry URL, because the key missed the cache. The new frame contains `#g=`, the watch stays armed, the next poll reads `canvasEntered` as false, and the new entry page's own word is what later swaps `src` to the root. A restart whose old `canvas` call fails does not write `minted` (`pairing.ts:70`).

The swap is one new iframe. `not pending(canvasAt)` removes the element while the cache-hit promise settles, then mounts another whose `src` is the root (`app.contract:1412-1413`). The session cookie is on the host, not the browsing context, and the two ports are the same site, so the new document sends `__Host-exact-canvas`.

The new test (`scripts/design-tool-scope.test.mjs:144-168`) locks the listener: wrong origin, wrong type, one mint, the returned record has no grant, a new tick mints again and is not entered. It does not reach the contract poll, an epoch change, or two overlapping mints.

## Findings

1. **SHOULD-FIX** — A new server run does not refresh the poll budget. `canvasPollsLeft` starts at 40 and is reset only in `reloadCanvas` and `saveName` (`app.contract:686`, `852-854`, `881-883`). The watch refuses to arm at 0 (`app.contract:822`). After a silent entry burns the 40 waits, the tool page still has that 0. A later `hello.epoch` (the dev server restarted, including the restart that finally has C2) mints a new entry URL and shows it. The entry page can POST, get the cookie, and post the word; the listener stores it (`pairing.ts:50`) and nothing asks `canvasEntered` again. `canvasAt.frame` and the iframe `src` keep `#g=` for the rest of the page. Reload Canvas is the only way out. Reset the budget when the mint key changes, not only when the tick actions run.

2. **SHOULD-FIX** — A slower `loadCanvas` still overwrites `minted` after a newer mint has won. The assignment at `pairing.ts:73` runs for every successful call. The runtime drops a stale ticket's reply (`host/web-js/rt.js:521`) and does not stop the async function. Two successes overlap when Continue runs during a slow mint: `saveName` increments `canvasTick` with no `pending(canvasAt)` check (`app.contract:881-883`), while the name control stays available in the header. If the older call finishes last, `minted` is the frame the iframe is not showing. The visible entry page's word sets `entered` to that other URL, so `canvasEntered` becomes true (`pairing.ts:78`). The following `loadCanvas` for the visible key misses the cache and mints a third grant, returning that entry URL (`pairing.ts:74`) and navigating away from the session that just started. Ignore a completion that is no longer the latest ask before writing `minted`.
