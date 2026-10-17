# Code review: Tamber 1.5, the inspect event (LLP 1122 T2b), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `c6382d7a3` (compacted once mid-review).
- **Method:** one brief (sha256 `ae3d5112a631b2360b4b31b59be8374c74d62cf20225beded3f7819454bfc42e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve (2 low).
- **Disposition:** taken, but the away-build low in part, as in round 2. This was the last round.
  - The stub's off events carry `to` as the server's do, and so does the contract test's takeover.
  - docs/tamber-protocol.md's `selected` report lists `inspect`, its command's number (*1.5*).
  - tamber-inspect.test accepts a selection naming the current command.
  - The page drops the node it selected under a command when another command comes (another owner, or off and on), so a later refresh does not report a click the server refused (Grok's low).
  - In part: after a missed poll, an away build this tool owns shows only when it registers again.

---

The prior pass left two possible lows. I’ll confirm those against the current code, then write the review.No High or Medium findings. Round 2 holds: the server publishes `inspect` exactly where ownership moves, the tool's fold matches that, and the 1.5 bump does not strand a 1.4 client. Two lows remain.

## Low — The stub's off event omits `to`, so an observer's selection ends on a takeover

`apps/design-tool/stub/tamber-stub.ts:110`, `apps/design-tool/app.ts:1158`, `apps/design-tool/app.test.contract:948`

The stub's comment says owners move as the server says them. `told` puts `to` on every off (`scripts/tamber-builds.mjs:136`). The stub logs `{event:'inspect', on:false, builds}` with no `to`. The tool treats a missing `to` like `to: null` (`!e.to`), which is a real end.

Ada is inspecting and another tool is only watching, with a live selection on that build. The agent takes the build through the stub. The watcher is not the client who lost it, so its own-loss clause does not apply, but the missing `to` still sets "inspect mode was turned off there" and Edit disappears. On the production channel the same takeover is `{to:'agent'}` followed by the agent's `on`, and that selection stays editable. The contract takeover at `app.test.contract:948` injects the same off with no `to`, and it injects it before any selection exists, so the test still passes.

**Fix:** Set `to` on the stub's off (the new inspector, or null when inspect mode ended). Inject `to: "agent"` in that contract event and assert a second tool's selection stays editable while the client who lost the build goes stale.

## Low — A click the server refused stays the page's selection, and a later refresh publishes it

`host/web/dev.js:488`, `host/web/dev.js:347`, `host/web/dev.js:662`, `host/web/dev.js:514`, `scripts/tamber-builds.mjs:284`

The page names the command: after `selectedReport` returns, `select` keeps the click only when inspect is still on under the same `seq`, then stores it in `inspecting.selected` and queues the POST. `report` never reads the status, so the new 409 is not retried and does not block the queue. That part is right. The hole is the POST already queued when a newer command arrives with inspect still on.

The page is on seq 1 and the click's POST is in flight with `inspect: 1`. Another client takes the build. The page gets `{on:true, seq:2}`. `inspect` stores the new seq and calls `setInspect(true)`, which returns immediately because inspect is already on and does not clear `inspecting.selected`. The POST is 409 at `tamber-builds.mjs:284` and publishes nothing; that 409 also does not send `{refresh:true}`. The refused node is still `inspecting.selected`. A later `{refresh:true}` (a stale-status re-ask, or `inspect {refresh:true}`) calls `select` on that node under seq 2, and the server publishes the click it already refused. Stop and a presence lapse do not have this hole: those frames set `on: false`, and `setInspect(false)` clears the node. A frame that arrives during `selectedReport`'s await is dropped by the seq check and never stored.

**Fix:** When an inspect frame changes `seq` while staying on, drop `inspecting.selected` recorded under the previous seq. Leave the stale-selection 409 alone, so a refresh can still re-report a node the server asked for again.

## What holds

`told` (`scripts/tamber-builds.mjs:134`) is the only `inspect` publisher. It runs after every change to `inspectors`: on and off, including this client's away builds (`349`); a registration that replaces or evicts a build (`225`, `230`, `242`); a page exit whose `inspect` matches the current command (`276`); and `clear` when presence lapses (`368`). Offs go out before ons, and each off carries `to` (the new owner, or null). A refresh, a repeat on, and a same-id re-register leave the map unchanged, so nothing is sent. Closing a stream announces `builds` and leaves the inspector in place. Frames are written before `told`. A matching page exit publishes `inspect` off and then `selected: null` in that order. An accepted click publishes `selection` before `selected`. A replaced command's exit returns 204 and publishes nothing (`277`). A non-null `selected` with no inspector, or with an `inspect` number other than the current command, is 409 before the join (`284`), so it is not published and the page is not asked to refresh. Omitting `inspect` is still accepted while someone is inspecting; that is what a 1.4 report looks like, and the wasm page sends the number whenever `seq` is set (`host/web/dev.js:473`).

The tool adds a build only on its own `on`, drops it on its own `off` or on someone else's `on`, and leaves it in place on someone else's `off` (`apps/design-tool/app.ts:1157`). Its own off stales the selection even when `to` names the client who took over. Anyone else's off stales it only when `to` is null, including when a later `on` shares the answer. `selected: null` no longer changes `inspectOn`; it marks the selection "its page left inspect mode". `sendInspect` does not write `inspectOn` (`1417`). A new name clears the set and resyncs from the records (`1085`). A new epoch clears the set and the selection (`1111`). A miss stales the selection and clears the set; hello puts back `inspector` on connected builds only (`1115`, `1173`, `582`). An away build this tool still owns stays off the toggle until it registers again. That is the accepted partial from round 2, and the code still matches it: Stop is not offered for that id until then, and the server drops it when this client's presence lapses. The toggle and Outline follow `inspectOn.size > 0`. Edit follows staleness, so another tool keeps Edit across a takeover whose `to` is set.

`PROTOCOL` is `tamber/1.5` (`contract/cli/src/tamber_serve.rs:67`). The protocol page, the hello field list (`inspector` at `docs/tamber-protocol.md:137`), and the JS and Rust hello assertions all say 1.5. `tamber/1.4` remains the history line and the RFC. The design-tool stub still greets with `tamber/1.1`; the tool does not branch on the protocol string.

`scripts/tamber-builds.test.mjs:425` covers `to` on takeover, page exit, `clear`, and off-everywhere, and a silent repeat and refresh. `scripts/tamber-inspect.test.mjs:123` covers the replaced command's 409 and a silent exit. The first inspector contract test waits `clock +1` before expecting "Stop inspecting" (`app.test.contract:789`). The rename test expects "Inspect" after Ada becomes Grace (`968`). Register replace, eviction, a same-id re-register, Outline, a miss, and a new epoch have no direct test; those paths are the ones above. Tests were read, not run.

**Verdict:** Approve — round 2 holds and the event tracks ownership; two lows, the stub's off omits `to`, and a refused click stays the page's selection for a later refresh.
