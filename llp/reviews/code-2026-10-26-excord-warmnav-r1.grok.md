# Code review: Excord warm address-following, round 1, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `91a970edbc`.
- **Method:** one brief (sha256 `a310dfdf7f41147b756e3606c37b94f9a8e5bd81175c1cc7796559438a3403ba`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the warm-navigation commit at HEAD read-only, starting with the commit text and the routing code it changes.The commit replaces three waiters with one sequenced request. I'll read the routing contract, the open paths, and the runner's navigate glue next.The sequenced request does keep a later channel from opening late. One side effect does not: a link switches the persisted server before the request is known to be current, and a Back that cancels the open leaves that server in place.

## MAJOR

**A cancelled link keeps the other server — `apps/excord/app.contract:987`**

`follow` writes `server` before `askFollow`, and `server` is persisted (`app.contract:78`). The checked selection is `selectWant` (`routing.contract:81-85`), which runs only while `wanting` is still true. An immediate Back never gets there.

From `#general`, tap the injected DM link (`https://discord.com/channels/@me/1191168914239782912`), then Back before the clock moves — the same timing as "Back before a link's channel opens". `askFollow` pushes `/c/<dm>` and asks for it. `askBack` sees the entry beneath is still the open channel and only bumps `intent` (`routing.contract:59-60`), so the DM never opens and `open` stays general. `server` is already `@me` and nothing restores the guild. The transcript is still `#general`; the rail is Direct Messages. The next launch comes back on `@me`. The same split happens for a link into another guild: that guild stays selected, and persisted, after Back. The new tests only cancel same-guild links (`#help`, `#random`), so they stay green.

## MINOR

**A cancelled link still runs `leaving()` — `apps/excord/routing.contract:66`**

`askFollow` calls `leaving()` (`app.contract:548-563`) as soon as the link is tapped, before `openWant` has committed. `leaving()` sends `viewChannel("")` (`live.ts:535-537`) and, if an edit is in progress, drops `editText` and clears `editing`. Back then cancels the open, so `openChannel` never runs and never sends `viewChannel` for the channel that is still open. The transcript stays up (`chanVersion` falls back to `gw.sidebar`, `app.contract:228`), but `gw.channel` stays `""`: role-color updates and the member-list revision stop following that channel (`live.ts:177-181`, `live.ts:208`) until some later open. An in-progress edit is gone even though the channel never changed.

## What holds

Every open path re-checks the newest request, or makes itself newest. `askRoute`, `askBack`, `askFollow`, and `readLaunch` all go through `ask` (`routing.contract:35-42`). `selectWant` and `openWant` both require `wanting` (`wantSeq == intent`, `routing.contract:30`, `82`, `89`) at the moment they run, so a task armed for an older id does not apply it. A list tap, the switcher, and `openResume` go through `openChannel`, which bumps `intent` (`app.contract:444`). `pickServer` (`508`) and `toSignIn` (`531`) bump it too. `pendingOpen` / `pendingMessage` are derives of the current want (`routing.contract:32-33`); the old `opening` waiter in `app.contract` is gone. An address that names the channel already open only bumps `intent` (`routing.contract:52-53`, and the same for Back at `59-60`), so it does not ask again.

Condition (1) holds because an app write to `nav` is not delivered back as `navigate`. `runner/src/runner/router.rs` is the wrong citation: `host_go` / `host_back` run only when the root has no `navigate` handler (`router.rs:772-780`). The hosts apply the app's own router change directly. On the web, `commit` uses `history.pushState` / `replaceState` (`host/web/navigation.js:260-280`), which do not fire `popstate`; an app-initiated `history.go` is swallowed by `echo` (`navigation.js:344-346`). `popstate`, a same-origin link, and typing into the root are what call `navigate` (`navigation.js:297`, `host/web/input-glue.js:122-125`, `host/web/glue.js:1170-1172`). On iOS, `NavigationRules.dispatchesBack` is false for a stack change Exact wrote (`NavigationRules.swift:61-63`), so a push does not call `navigate`. Linux and the terminal dispatch `Navigate` for system Back, links, and a typed location (`host/linux/src/presenter/back.rs:212-214`, `links.rs:205`, `typing.rs:16-22`), not for an app write. A user Back between a link's `push` and `openWant` is the race that can happen; `askBack` cancels it, and both the contract test and the e2e test cover that timing.

Condition (2): `want*` is plain state. `onScreen` is the only persisted route memory (`routing.contract:22`). It is cleared when the list is asked for (`routing.contract:44-45`) and otherwise written from the resolved `open` by `rememberScreen` only while `not wanting` (`105-108`). A want in flight is not what the next launch restores.

Condition (3): `askFollow` pushes one visit (`routing.contract:67`); `openChannel` replaces when the top is already a channel (`app.contract:495`). One Back returns to A. `go` to a URL already on the stack truncates (`route/src/router.rs:228-232`) instead of pushing another.

Condition (4): composer text is `draftOf(drafts, open)` (`app.contract:284`), written per channel by the composer. `openChannel` does not clear another channel's entry. The new contract test types a draft in `#general`, opens `#help` from the link, types a second draft, and Backs to the first.

`follow` still highlights a message: `openWant` copies `wantMessage` onto `highlight` / `anchor` / `target` after `openChannel` returns (`routing.contract:90-101`), and that later write is the one that commits. Wide keeps the open channel beside the list because `ask("")` leaves `open` set and `resume` runs only when `wide` and `pendingOpen == ""` (`app.contract:337-338`, `512-514`). Narrow Back to `/` pops the channel route and does not resume. A DM link still ends at `server == "@me"` once `selectWant` runs; the launch restore still yields to any `intent != 0` and `launchRead` makes it once (`routing.contract:71-77`). `app.contract` is 1483 lines, `routing.contract` 108.

The e2e file drives the real web build: `exact.agentSettled` for taps, clock, and typing into the navigation root (`glue.js:1170-1172` calls `navigate`), and `page.goBack` / `goForward` for browser history. It is not a re-implementation of the derives. It skips unless `host/web/dist` is an Excord build, so a default test run does not execute it. The warm-nav contract tests fail on main: a link used to `replace` the current channel visit, so one Back returned to the list, and Back did not clear `pendingOpen`, so the linked channel still opened. The two-address e2e fails on main because `leave` did not open the typed channel. Those tests do not cover a cross-server cancel, a message-link highlight, sign-out mid-open, or wide (the e2e viewport is 420).

REQUEST CHANGES
