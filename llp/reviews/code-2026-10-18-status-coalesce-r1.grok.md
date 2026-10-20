# Code review: a design-only build-status told once per 150 ms, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the coalescing commit (1665bbe4a).
- **Method:** one brief (sha256 `977712ad246ee894e976321410696fe6ae521a1eab1e1ff18740a0d4ffe58ac9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the same selection latch; the timer and registration; the doc table; the test).
- **Disposition:** fixed in round 1's commit (bce9dcd95; see its message).

---

There is one blocking issue: a selection made while a design-only status is waiting is latched against the previous design, then marked stale for good when the trailing `build-status` arrives.

1. **Blocking** — A current selection is latched to the design the client has not been told yet, then stuck stale. `scripts/tamber-builds.mjs:171`, `scripts/tamber-server.mjs:219`, `apps/design-tool/app.ts:1131`.

   Native scrub updates `b.design` on every frame and `tellStatus` holds the `build-status` for up to 150 ms. The server still accepts a `selected` report against that kept design (`current` in `tamber-inspect.mjs`) and publishes `selection` immediately. `inspect {refresh: true, status: true}` is the same hole: the follow-up status is design-only relative to the last told payload, so it is coalesced, and the selection event still goes out first. The tool sets `selectionNow.at` from `buildsNow` (`buildKey` includes `design`). Up to 150 ms later the trailing status moves `buildsNow.design`, `latch` sees a mismatch, and `stale` stays set until another selection. During a scrub the timer is armed almost continuously, so a click in that window — including the refresh that was supposed to deliver status first — comes back already stale.

   Fix: if a tell is waiting for that build, flush it with `tellStatus(b, true)` synchronously inside the `selected` report before `selection` is sent (same turn, so one poll applies the status first). Clear that timer there. Add a test where a design-only status is still waiting, a matching `selected` is reported, and the next events are `build-status` with that design, then `selection`.

2. **Should-fix** — The 150 ms timer is not tied to the build's stream, so it tells a build that has already gone, and it judges the next registration against the previous one. `scripts/tamber-builds.mjs:172`, `scripts/tamber-builds.mjs:156`, `scripts/tamber-builds.mjs:349`.

   `close` announces `builds` without that id, but the timeout still runs. The build stays in `builds` until the 256 cap drops it, and the callback only checks `builds.get(id)`, then `tellStatus(now, true)`. The tool upserts `build-status` (`app.ts:1119`), so a build the `builds` list just removed is shown as connected again, with its old status, until some later `builds` event. Re-register and reconnect hit the same timer: `toldFacts` is still the previous generation, registration sets `status: null` and does not clear it, and the first new status whose facts match that old key and whose design differs is coalesced for up to 150 ms. A selection in that window latches `status: null`. Forget-over-cap drops the map entry, so the callback no-ops, but `toldLater` is not cleared; the timeout is also not `unref`'d, so after the channel's clients are gone it still calls `send`.

   Fix: on close, on register (including the id a stream replaces), and in the forget loop, `clearTimeout` and delete `toldFacts` / `toldDesign` / `toldLater` for that id. In the callback, send only when `connected(now)`. `unref` the timeout.

3. **Nit** — The event catalogue still says every status report produces a `build-status`. `docs/tamber-protocol.md:752` (new sentence at `docs/tamber-protocol.md:896`).

   The new sentence is right that a design-only tell is at most one per 150 ms with the latest design, and that any other change is immediate. The table row still says `build-status` is one build after its `status` report. It also never says `hello` and `builds` keep the uncoalesced design (they do: `list` / `shown` read `b.design` on every report).

   Fix: point the table row at the 150 ms rule, and say `hello` and `builds` are not held.

4. **Nit** — The new test does not catch either bug above. `scripts/tamber-builds.test.mjs:640`, `scripts/tamber-builds.test.mjs:648`.

   `Bun.sleep(200)` only checks a quiet 150 ms timer. The "status asked for again" case resends `design: 'd7'` after that design was already told, which skips the coalesce because the design string matches, not because a refresh carried an untold design. Closing, re-registering, or selecting while the timer is armed is untested, so both failures stay green.

   Fix: drive the timer explicitly (or inject `now`), and cover disconnect, re-register, and a `selected` report while a design tell is waiting.

The facts key is the right one: it is `shown(b)` minus `design`, which is the whole `build-status` payload. A native scrub status changes the design digest only (`LiveDesign` / the presenter's `report_design`); scheme, contrast, mode, root font size, plan, plan digest, and pin revision stay put, and a real change in any of those is told immediately with the current design. Nothing that must be immediate is left out. `toldFacts` / `toldDesign` are capped (sweep past `BUILDS * 2`). Holds and `replayHold` still run before the tell and only when `planDigest` changes, which is in the key, so they are not delayed. Scrub end and verify follow `scrub` events, not `build-status`. The server's own selection check reads the kept design, so it is not 150 ms behind — the client latch is.
