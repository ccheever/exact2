# Code review: Tamber 1.5, the inspect event (LLP 1122 T2b), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `d382797d3` (compacted once mid-review).
- **Method:** one brief (sha256 `03ad137b98f2a644cfae8ff9eefec5c5b04cfd33c874318be995a212b6771d94`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve (2 low).
- **Disposition:** taken, but Grok's first low in part.
  - A selection report names the inspect command it was made under. One from a command since replaced (Stop then Inspect, another client taking over) is refused, 409, and not published.
  - An off event says who took the builds (`to`), or null where inspect mode ended. A selection goes stale on every real end, `to: null`, whatever comes after it in the same answer, and on this tool's own loss. A takeover leaves other tools' selections alone.
  - A rename is another client: the tool reads what it inspects again from the records.
  - The first inspector test waits for the event.
  - The hello schema in docs/tamber-protocol.md lists `inspector`.
  - Tests: tamber-builds.test (`to` on each off), tamber-inspect.test (a replaced command's selection refused), contract (a rename, the lapse with the record).
  - In part (Grok low 1): after a missed poll, a build this tool owns that is away shows only when it registers again (hello lists builds with a stream). Until then Stop is not offered for it. Its ownership still ends when this tool's presence lapses.

---

Round 1’s fixes hold, and the `inspect` event is published exactly where ownership moves. Two lows remain: a missed poll drops ownership of a build that is away, and the hello schema omits `inspector`.

## Round 1

- A page exit whose `inspect` seq is not the build’s current command returns 204 at `scripts/tamber-builds.mjs:275` and leaves the log untouched. A matching exit deletes that owner, publishes `inspect` off, and then `selected: null`. The seq stays put, so a later owner’s command (a new seq) still ignores the old exit.
- A non-null `selected` while that build has no inspector returns 409 at `scripts/tamber-builds.mjs:280`, before the join. The log gets no `selected` or `selection`, and the build is not asked to refresh. The page discards that response: `host/web/dev.js:347` awaits `fetch` and ignores the status, so a 409 is not retried. The click is also dropped on the page when inspect mode or the seq has already moved (`host/web/dev.js:486`). A click that was on its way when Stop or a lapse ended inspect mode does not become a selection.
- `shown()` names `inspector` on every record (`scripts/tamber-builds.mjs:124`). Hello, `builds`, and `build-status` all use it.
- The toggle follows those records and the `inspect` events. `sendInspect` (`apps/design-tool/app.ts:1415`) leaves `inspectOn` alone. `selected: null` leaves it alone too (`apps/design-tool/app.ts:1164`) and marks the selection stale with “its page left inspect mode”.
- This client’s own off stales the selection at once (`apps/design-tool/app.ts:1160`), including when a later `on` in the same answer is another client taking the build. Everyone else stales at the end of the answer only if that build is still in `offNow` (`apps/design-tool/app.ts:1168`). A later `on` removes it from `offNow`, so an observer keeps the selection across a takeover.

## Where the event is published

Every change to `inspectors` snapshots the map and calls `told` (`scripts/tamber-builds.mjs:132`): inspect on and off, including this client’s away builds (`345`); a registration that replaces or evicts a build (`223`, `228`, `240`); a matching page exit (`276`); and `clear` when presence lapses (`367`). Offs go out before ons. A refresh, and a repeat that moves nothing, leave the map unchanged, so `told` sends nothing. Closing a stream announces `builds` and leaves the inspector map as it is; the next registration replays the frame from that map (`238`).

Command frames and `clear`’s frames are written before `told`. A page exit publishes `inspect` off and then `selected: null`. `send` wakes a waiting long poll, and the answer runs after that stack returns, so those events share one answer in that order. An accepted click publishes `selection` (and the outline frames) before `selected`.

## The tool’s fold

Own `on` adds the build, own `off` removes it, another client’s `on` removes it, and another client’s `off` leaves `inspectOn` alone (`apps/design-tool/app.ts:1157`). A new epoch clears the set and the selection (`1109`). A miss stales the selection with “some changes were missed” and clears the set (`1113`); hello then applies `inspector` on the connected records (`1176`, `582`). The toggle and Outline both follow `inspectOn.size > 0` (`1414`, `apps/design-tool/app.contract:899` and `:937`). Edit follows staleness, so it stays available to a tool that did not lose the build when someone else takes it in the same answer.

## Protocol and tests

`PROTOCOL` is `tamber/1.5` (`contract/cli/src/tamber_serve.rs:67`). The JS and Rust hello assertions expect `tamber/1.5`. `tamber/1.4` remains the history line at `docs/tamber-protocol.md:41` (and in the RFC). The design-tool stub still greets with `tamber/1.1`; the tool does not branch on the protocol string.

`scripts/tamber-builds.test.mjs:425` covers on, a silent repeat, takeover, a matching page exit, `clear`, `inspector` on `list()`, off everywhere including an away build, and a silent refresh. `scripts/tamber-inspect.test.mjs:113` and `:126` cover the 409 and a replaced command’s silent exit. `apps/design-tool/app.test.contract:933` covers the toggle across a takeover and a later own off that removes Edit. Register replace and eviction, a same-id re-register, a missed poll, a new epoch, and Outline have no test. The replace and re-register paths are right in the code above; the miss path is the first low.

## Low — A missed poll forgets an away build, and Inspect cannot restore it

`apps/design-tool/app.ts:1113`, `apps/design-tool/app.ts:1176`, `scripts/tamber-builds.mjs:127`, `scripts/tamber-builds.mjs:345`, `apps/design-tool/app.ts:1420`

Ada owns one build, and that build’s stream has dropped. The server keeps her as `inspector` (close announces `builds` and leaves the map). Her tool then receives `missed: true` because the retained log no longer reaches its cursor. The handler clears `inspectOn`. Hello lists only builds with an open stream, so the away id is not put back. The toggle says Inspect. Pressing it does nothing useful: the away build is already hers, `told` stays silent because nothing moved, and the reply is not applied to `inspectOn`. Stop is not offered, so that ownership stays until the build registers again. Registration replays `{on: true}` first, and the page comes back taking input; the following `builds` record is what turns the toggle back to Stop.

A connected build she still owns is restored from hello, and Stop then runs off-everywhere, which does clear away ids. The hole is the case where every build she still owns is away.

**Fix:** Have the missed answer name the ids this `client` still owns, connected and away, and refill `inspectOn` from that list. Keep applying hello’s `inspector` to connected rows. Add a test that drops the stream of an owned build, answers a miss plus a hello with no builds, and still offers a way to turn that ownership off.

## Low — The hello build schema omits `inspector`

`docs/tamber-protocol.md:137`

`shown()` always sends `inspector`, and the resync above depends on it. The event table mentions the field (`docs/tamber-protocol.md:560`). The hello field list, which is also what the `builds` event says it carries (`555`), is still `{id, host, capabilities, plan, design, status, pin, pinRevision, unsupported}`.

**Fix:** Add `inspector` (a client or null, *1.5*) to that list.

**Verdict:** Approve — round 1 holds and the event tracks ownership; two lows, away ownership after a missed poll and the hello schema.
