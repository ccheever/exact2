# Code review: Excord warm address-following, round 2, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `86e0d85457`.
- **Method:** one brief (sha256 `96af19cad19259441167a39c7c3e43c2eaaa374ef792dbe632d8209d19ff94cf`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

The review is ready to write once the cited lines are confirmed against the current files. I'll re-read the sign-out path, the empty beneath-visit, and the same-open route compare so each finding has an exact location.Round 2 holds for the round-1 blockers, the server restore on `ask` and `supersede`, `leaving` on a real channel change, the tick-by-tick race tests, and the decoded id once a route request is in flight. Conditions (1)–(5) hold in behavior. Three holes remain in the new cancel and visit rules.

The plan file `~/x2review/x2p4-excord-warmnav-plan.md` is not on disk. Conditions below are the ones in the request and in the HEAD comment.

**Claims that check out**

- Only the top channel visit renders `id="transcript"` (`app.contract:1216-1218`, `1275-1276`). A covered channel visit is an empty column, so a scroll can no longer hit a second transcript.
- `follow` calls `supersede()` when `channelId == open`, then jumps (`app.contract:982-985`). `supersede` increments `intent` (`routing.contract:55-58`), so a later `selectWant` / `openWant` sees `wanting` false.
- `selectWant` is the only request path that writes `server`, and it records `wantServerWas` first (`routing.contract:104-108`). `ask` and `supersede` copy that back before dropping the request (`routing.contract:42-43`, `56-57`). `follow` no longer assigns `server`. A completed `openWant` clears `wantId` before `openChannel` (`routing.contract:115-116`), so the server it selected stays. `pickServer` writes the chosen server and then increments `intent` (`app.contract:506-508`), so a later `ask` does not overwrite that choice. A list tap is `openChannel` (`app.contract:443-444`, `1215`), which keeps the server already on screen; that list is the selected server’s. No restore loop: after `ask` or `supersede`, `wanting` is false, so `selectWant` cannot reapply.
- A same-open address or Back calls `supersede` and one `viewChannel(open)` (`routing.contract:66-68`, `75-77`). Another channel or the list calls `leaving` once (`routing.contract:69-71`, `78-80`), and that sends one `viewChannel("")` (`app.contract:561`). `askFollow` does not call `leaving` (`routing.contract:83-85`); `openChannel` sends one `viewChannel(id)` (`app.contract:492`). No action on these paths sends `viewChannel` twice.
- The e2e model steps 1 ms and rejects a tick whose `open` or `onScreen` is the cancelled id (`fixture/routing.e2e.test.ts:93-101`, `168-184`). The server-restore case (`186-206`), the open-channel link (`208-221`), and narrow 420×900 versus wide 1100×760 (`223-239`) are in the file. By inspection the first fails without `supersede`’s restore and the second fails without `follow`’s `supersede`. This pass did not edit the tree to re-run them with the fix removed, and did not execute the suite.
- `routeDecoded` / `takeDecoded` copy the router’s decoded `top(nav).params.channel` onto an in-flight route request (`routing.contract:96-100`). Launch and Back already read that param (`routing.contract:74`, `92`).
- Condition (1): recognition is `id == open` (`routing.contract:66`, `75`). There is no timed ignore flag. App history writes go through `pushState` / `replaceState`, and an app-driven `history.go` is consumed as its echo (`host/web/navigation.js:247-281`, `339-352`).
- Condition (2): `onScreen` is written by `ask("")` and `rememberScreen` (`routing.contract:52-53`, `128-131`). `rememberScreen` stores `open`, which only `openChannel` sets and `toSignIn` clears. `wantId` is not persisted.
- Condition (3): a route open replaces when the top visit is already a channel (`app.contract:495`). `askFollow` pushes once (`routing.contract:84`).
- Condition (4): drafts are keyed by channel. `leaving` and `openChannel` stash the edit only while `editing != ""` (`app.contract:451-452`, `549-550`). A link A→B leaves A’s draft for `openChannel` to stash when B resolves.
- Condition (5): relaunch tests are still at `app.test.contract:362`, `385`, and `408`, and the scroll-ack test at `802`. The delta does not skip them.

**MAJOR** — Sign-out leaves a cancelled request’s server selected, and a wide window can open that server’s channel before the gateway is signed out.

`selectWant` persists `server` as soon as the channel resolves (`routing.contract:106-108`, `app.contract:78`). The comment says a cancelled request puts that server back (`routing.contract:15-16`). The copy-back runs only at the start of `ask` and `supersede`. Sign-out goes through `toSignIn`, which increments `intent`, sends the router home, and clears `open` (`app.contract:530-534`). `wanting` is then false, so nothing later restores `wantServerWas`.

Scenario: the account is on guild A. A link or address whose channel is not open yet runs `selectWant` and the sidebar becomes `@me` or guild B (`wantServer` true). The reader signs out from the account menu (`app.contract:1196-1207`) before `openWant`. The next sign-in opens on B. On a wide browser (`app.contract:217`), `toSignIn` also leaves `pendingOpen` empty and the top visit at home while `signedOutNow` is still false (`app.contract:311`, true only once `gw.state == "signed-out"`). `resume` then calls `openChannel(resumeChannel)` for B’s remembered channel (`app.contract:337-338`, `219-220`, `512-514`) and sends `viewChannel`. The HEAD message notes that sign-out mid-open is untested; it describes the `intent` bump and the `onScreen` clear, and says nothing about `server`.

**MAJOR** — An iOS edge swipe back to a channel reveals an empty page for the whole gesture.

After a link, the stack is home, channel A, channel B. A is not the top visit, so it renders an empty column (`app.contract:1216-1218`). UIKit shows that view controller during the interactive pop and paints it from `willShow` (`host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:1194-1220`). The contract’s `back` runs only when the pop completes and `dispatchesBack` is true (`NavigationIOS.swift:1320-1333`, `NavigationRules.swift:61-63`). Until then the revealed page has no header, transcript, or composer. A swipe back to the list still shows the directory, because a home visit is rendered in full (`app.contract:1180-1215`). When the pop does complete, the destination becomes top and draws `open` (`app.contract:1275-1276`), which is still B until `openWant`. The page under the finger is blank, then briefly B, then A.

**MAJOR** — An address the router refuses still runs `leaving()`, and the channel left on screen is no longer the live view.

`leave` always calls `askRoute` after `go` (`app.contract:540-542`). A location with no route makes `go` return the same router and a refusal (`route/src/router.rs:223-227`); a refused verb answers its input (`semantics/README.md:842-843`). `askRoute` then takes the else branch for an empty id and calls `leaving()` (`routing.contract:64-71`), which can `markRead` and always sends `viewChannel("")` (`app.contract:558-561`). `view` clears `live.viewing` (`apps/excord/live.ts:535-537`). The host, seeing no router change, puts the address bar back (`host/web/navigation.js:283-318`). Nothing after that sends `viewChannel(open)`.

Scenario: the reader is in `#general` and edits the location to `/nope`. The bar snaps back to `#general` and the transcript stays, because `open` is unchanged and rows still load from it (`app.contract:237`, `271`). The gateway view is now empty. On the next READY, `live.subscribed` is cleared (`live.ts:160`) and `subscribe` returns without an op 37 because the guild is empty (`live.ts:521-524`). The channel on screen receives nothing until some later navigation calls `viewChannel` again. `keepOnScreen` does write `onScreen` back, because the top visit is still that channel (`routing.contract:128-131`).

**MINOR** — A percent-encoded address for the channel already open misses the same-open short-circuit.

`askRoute` compares the raw path piece with `open` (`routing.contract:64-66`). The decoded param is applied a millisecond later by `takeDecoded` (`routing.contract:96-100`), after `leaving()` has already run. Scenario: `#general` is open and the reader types `/c/` plus the percent-encoded snowflake into the navigation root (`host/web/glue.js:1168-1172`). `leaving()` sends `viewChannel("")` and may `markRead`; `takeDecoded` then points the request at the same channel, and `openChannel` reopens it, clearing the reply, the unread bar, and the saved place (`app.contract:457-492`). A same-origin link click is already decoded (`host/web/input-glue.js:118`), so this is a typed or history URL whose spelling stays percent-encoded.

**NIT** — The condition-(1) comment names one file for three different host entries.

`routing.contract:8-11` cites `host/web/navigation.js` for `popstate`, a same-origin link, and a location typed into the root. `popstate`, `pushState` / `replaceState`, and echo consumption are in that file. A same-origin link click calls `navigate` from `host/web/input-glue.js:112-125`. A location typed into the root is `exact.navigate` in `host/web/glue.js:1168-1172`. The UIKit half of the comment matches `NavigationRules.dispatchesBack`.

REQUEST CHANGES
