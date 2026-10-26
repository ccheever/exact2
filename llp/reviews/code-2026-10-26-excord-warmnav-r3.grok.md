# Code review: Excord warm address-following, round 3, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `ac29430998`.
- **Method:** one brief (sha256 `9f51142b3552b6a6a33c485cc741b5c00e1c07a237fb9854ae538181a4be2b66`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-check the warm-navigation delta at HEAD against the round-2 commit, read-only, and verify each claim with file and line evidence.The delta is six files. Next I’ll read the routing and app contracts around each claimed fix and check the fixture for leaks.The image viewer stays up after a successful address change or Back onto another channel. `leaving` used to clear it, and the open path that replaced that call never does.

## Claims

- A channel visit under another renders that channel's header and no transcript. The beneath branch is `app.contract:1215-1218`; the only `id="transcript"` is on the top visit (`app.contract:1277-1278`).
- A link into the open channel calls `stayOpen` (`app.contract:980-981`). That supersedes, puts the address back with `go(nav, path("channel", open))` (an entry already on the stack is truncated to, `route/src/router.rs:228-232`), and rejoins `viewChannel(open)` (`routing.contract:64-68`).
- Wide `/` and wide Back to the list supersede and keep the open channel (`routing.contract:80-82`, `98-99`). Neither calls `leaving` nor `openChannel`. `now` is stamped only by `openChannel` (`app.contract:456`) and the 60s tick (`app.contract:388-389`). The wide e2e asserts `now` unchanged (`fixture/routing.e2e.test.ts:255-263`).
- `leaving` runs only for a narrow trip to the list (`routing.contract:85-87`, `102-104`). A request for another channel only `ask`s; `openChannel` leaves the one it replaces (`app.contract:449-455`).
- `toSignIn` calls `supersede` (`app.contract:530-531`), which puts a cancelled request's server back (`routing.contract:56-59`).
- An address that is neither `/` nor `/c/…` only supersedes (`routing.contract:78-79`). With no router change, the host restores the previous entry (`host/web/navigation.js:314-318`).
- `takeDecoded` rejoins only when the decoded id is the open channel (`routing.contract:129-131`). Otherwise it corrects `wantId` (`:133`).
- `leave` only calls `askRoute` and `back` only calls `askBack` (`app.contract:540-543`). Each of those writes `nav` on at most one branch.
- The launch user-wins test holds guilds 3s (`fixture/inject.ts:26-27`, `fixture/server.ts:470-488`), then Command-K on `testId="excord"` (`app.contract:1150`, `1010-1012`) and opens Weekend rides (`app.test.contract:430-459`). `openChannel` bumps `intent` (`app.contract:444`); `openWant` applies only while `wanting` (`routing.contract:145`). The Back variant remains, commented web-only (`app.test.contract:461-463`). Default viewport is 420×900 (`scripts/agent.mjs:49-50`), so `wide` is false there.
- Forward-then-link and Back races sample 1ms ticks (`fixture/routing.e2e.test.ts:279-283`, `294-298`). New cases: `/nope` (`:234-244`) and wide `now` (`:255-263`).
- `hold-guilds` is consumed by the next IDENTIFY (`fixture/server.ts:470-471`). The delayed `GUILD_CREATE` bails when `resetVersion` changes or the socket closes (`:485-487`). Transcript world's `lazyGuildId` is `''` (`fixture/transcript-world.ts:148`), so nothing sends those guilds early. A passing run does not leave the hold for later tests.

No nav loop: app history writes go through `pushState`/`replaceState` (`host/web/navigation.js:260-280`), and a restore `history.go` is consumed as an echo (`:344-346`).

## MAJOR

**An open image stays up over the channel a successful address change or Back opens.** `askRoute` / `askBack` to another channel no longer call `leaving` (`routing.contract:92-93`, `108-109`). `leaving` is what set `viewing = ""` (`app.contract:558`). `openChannel` saves place, draft, and read state (`app.contract:449-455`) and never clears `viewing`. The viewer is a window-level sibling of the stack (`app.contract:1473-1475`), and `covered` makes the new channel inert (`app.contract:197`, `1220`). The switcher already compensates after `openChannel` (`app.contract:1032-1034`); the route open does not. Browser Back, Forward, or a typed `/c/B` while a picture is open leaves that picture on screen.

## MINOR

**A percent-encoded address for the open channel rejoins, and pushes a second history entry.** `canonical` keeps the `%` spelling (`route/src/location.rs:7-8`, `102-112`), so `go` does not find `/c/<id>` and pushes (`route/src/router.rs:228-242`). Params are decoded (`route/src/table.rs:186`). `takeDecoded` supersedes and rejoins (`routing.contract:129-131`) without popping that entry. One Back is spent on a duplicate of the channel already open (`routing.contract:105-107`).

**The beneath header is blank for a channel on another server.** It looks up `channelOf(channels.concat(dms), …)` (`app.contract:1218`). `channels` is only the current guild (`app.contract:210-213`). An edge swipe back to the server just left shows an empty title. Same-guild links resolve.

**`/nope` during an in-flight open cancels the request and leaves its address.** Unknown addresses only supersede (`routing.contract:78-79`). `askRoute` has already pushed `/c/B` (`:84`). The host restores `/nope` back to that entry (`host/web/navigation.js:314-318`). The screen is still A's (`open` unchanged) while the address is B, so the next launch's `readLaunch` opens B (`routing.contract:121-123`).

## NIT

- The beneath-visit comment still says it holds nothing (`app.contract:1214`).
- That header prints `name` only (`app.contract:1218`); the top visit prefixes guild channels with `#` (`app.contract:1224`).
- `menu` in `fixture/routing.e2e.test.ts:116-123` has no caller.

REQUEST CHANGES
