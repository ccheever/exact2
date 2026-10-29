# Code review: Excord warm address-following, round 5, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `eecb89e29b`.
- **Method:** a delta brief (not kept), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-check the warm-navigation delta at HEAD against `394e2889ef`, read-only, and verify the two claimed fixes with file:line evidence before looking for regressions.The delta is two files. I'll read the routing contract and the navigation tests around `stayOpen`, `openVisits`, and the `/nope` paths.Both claimed fixes are in the code, and the three named follow-ups still hold. One interaction of the two new branches pushes a second visit.

The list-then-`/nope` fix holds. Narrow Back pops the visit (`routing.contract:112-115`) and `leaving` leaves `open` set (`app.contract:545-560`), so `openVisits` is 0 (`routing.contract:40`). `/nope` only calls `supersede` (`routing.contract:87-90`). With no router change, the host restores `/` (`host/web/navigation.js:314-318`). The new test asserts that (`fixture/routing.e2e.test.ts:252-267`).

The encoded-address fix holds for that step alone. `go` matches canonical URL text (`route/src/router.rs:228`); `%31` keeps its spelling (`route/src/location.rs:7-8`) while `params.channel` is decoded (`route/src/table.rs:186`). After the visit was popped, the encoded push is the only match, `openVisits` is 1, and `takeDecoded` supersedes and rejoins in that visit (`routing.contract:143-147`). It does not push `path("channel", open)`.

These also hold:

- In-flight `/nope` over the open visit: the other channel does not count, `openVisits` stays 1, and `stayOpen` pops to the canonical entry still on the stack (`routing.contract:87-88`, `71-74`). The test at `fixture/routing.e2e.test.ts:237-249` still matches.
- Wide `/`: still the untouched branch at `routing.contract:91-93`, for a canonical visit.
- Back after that encoded visit: the entry beneath is the list, so one Back leaves the channel (`routing.contract:108-115`).

**MAJOR** — `routing.contract:87` (push at `:74`; also `:95` then `:99`, and `app.contract:982`). `openVisits > 0` is treated as "the canonical visit is on the stack, so `go` will pop." It counts decoded params. The visit `takeDecoded` now keeps (`:145-147`) is the encoded spelling, so its URL is not `path("channel", open)`. From that screen:

- `/nope` takes the new `openVisits > 0` arm and `stayOpen` pushes the canonical address under the encoded one.
- Typing the plain `/c/<id>` hits `nav = go(nav, url)` before the `id == open` rejoin, and pushes the same way.
- A link to the open channel calls `stayOpen` and pushes too.
- The same `> 1` arm (`:143-144`) pushes a third visit when both entries are encoded spellings and neither URL is canonical.

Back then lands on the encoded visit, still that channel. One Back does not leave. Wide `/` (`:93`) does this same push from that state. Ordinary wide `/`, with the visit already canonical, does not.

**MINOR** — `routing.contract:90`. Narrow list (visit already popped, `open` still set), then `/c/<other>` and `/nope` before it opens: `openVisits` is 0, so this only supersedes. The other channel never opens, and the route `askRoute` already pushed stays. The URL is that cancelled channel, the screen is the channel left (`reading` follows the route, the transcript follows `open`), and the list is not showing. One Back returns to the list.

**NIT** — `routing.contract:83-84`. That comment still says the open channel's address is put back over an in-flight request. The branch below does that only when `openVisits > 0`.

REQUEST CHANGES
