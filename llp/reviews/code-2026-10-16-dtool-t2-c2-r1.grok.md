# Code review: LLP 1122 T2 step 2 (builds panel and pins), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `39d166329`.
- **Method:** one brief (sha256 `444f48e239cd240a54d03fb905137a0cbbeccc495d9175adaabe81d4bd4e8242`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 3 medium, 2 low).
- **Disposition:** taken: builds come only from the server: hello's list, read inside the poll's answer whenever the poll starts again (the first answer, a gap, a new run), so the panel shows it at once, and a new run drops the old list; each listed build now carries `pinRevision` (tamber/1.3, scripts/tamber-builds.mjs: the revision of the last pin frame it was sent, a pin, a hand-back or its pin again on registering, whoever sent it), and a row is caught up only when its status reports that revision (never before its status comes), so another client's pin, a reload, a new run and a pin announced before its reply all read right with no local bookkeeping; a hand-back shows "handing back" until caught up; a build that has not reported says so; an unsupported answer stays while the pin it answered is the build's pin; Pin needs something to pin (only Hand back sends null); the preview is "Preview only", and the docs say it replaces the pin; the font field's minimum is 4; the stub replaces on preview, announces `builds` with `pinRevision`, and refuses a client that has not polled; the contract tests cover pinning then pinned, handing back, a preview replacing a pin and its refusal, another client's pin, a build not reported yet, and a new run's builds; the real drive keeps the tool open past one lease with the pin held, and checks the hand-back lands 10-50 s after the tool closes. Not tested: the stub's refusal of a client with no poll through the tool (the tool always polls; the server's own test has it).

---

The builds panel can show the wrong list and call a pin caught up while the build's status is still old. Those two are the blockers.

## High

**1. A missed poll or a new server epoch leaves the panel on a stale list for up to 25s, and a restart keeps the old pin bookkeeping.** `apps/design-tool/app.ts:455` (`takeBuilds`), `apps/design-tool/app.ts:669` (`loadHello`), `apps/design-tool/app.ts:907` and `apps/design-tool/app.ts:920` (`waitChanges`), `apps/design-tool/app.contract:442` (`advance`).

The panel renders `changes.builds`, which is only the snapshot `waitChanges` returns. `hello.builds` is written into `buildsNow` as a side effect and is not what the panel reads. The events reply has no build list (`scripts/tamber-server.mjs:374`). On `missed` or an epoch change the server sends **no** events (`scripts/tamber-server.mjs:372`), including when the kept log no longer reaches `after: 0` (`events[0].seq > 1`, the usual case after 256 design edits).

`advance` then refreshes hello and starts the next poll at the new tip. That poll matches the epoch and has nothing new, so it sits for `POLL_MS` (25s, `scripts/tamber-server.mjs:82`). Hello can fill `buildsNow` during that wait, but the panel does not repaint until the poll returns.

- Open the tool against a dev loop that has already dropped early events, with a phone registered. The first poll is `missed` and returns before hello. The panel says "No build is connected" for about 25 seconds, then lists the phone.
- Restart the dev loop while the tool is open. The failed poll still shows the old builds; the follow-up poll with the old epoch returns immediately as `missed` and still paints them. The corrected list appears only on the next idle poll. `takeBuilds` copies `sent` and `unsupported` by id (`app.ts:457`), so a build that comes back as `web-1` still shows the previous run's "Not applied: …", and `caughtUp` is compared with the previous run's revision counter (it starts again at 0).

Fix: on a new epoch, drop `buildsNow` (including `sent` and `unsupported`). On `missed` or a new epoch, apply a fresh `hello.builds` inside that same `waitChanges` return so the panel paints the server's list without waiting for the next long poll.

**2. "Pinned" is shown for the whole lag the indicator exists to cover.** `apps/design-tool/app.ts:452` (`caughtUp: !b.sent || …`), `apps/design-tool/app.ts:1174` (`sendPin`).

`sent` stays 0 until the pin reply is applied, and `!b.sent` means caught up. The server announces `builds` inside `channel.pin` **before** it writes the pin response (`scripts/tamber-builds.mjs:256`, `scripts/tamber-server.mjs:471`). A long poll that is already waiting is woken there, so the tool usually applies the new pin while `sent` is still 0 and paints "pinned". `sendPin` then stores the revision, but nothing repaints until the next poll. That next poll is the build's `build-status`, or another 25s idle timeout if the build is slow. Either way the orange "pinning" state never appears during the lag. A tool reload has the same hole: `sent` is gone, so any pin already on the server shows as caught up even when `status` is still null after a re-register.

The contract test hides this. The stub logs `build-status` and returns, and the test only clocks the event after `pin-result` (`apps/design-tool/app.test.contract:465`), so `sent` is already 1. The real server does the opposite.

Fix: before the `await` in `sendPin`, mark the targeted builds pending (every build when the target is "all") so `caughtUp` is false for the event that lands during the request. On success, set `sent` from `revision` and clear pending; on failure, clear pending only. Treat a null `status` as not caught up whenever a pin is present.

## Medium

**3. Pin with the form still blank hands every build's pin back.** `apps/design-tool/app.ts:1167` and `apps/design-tool/app.contract:566`, with the default target at `apps/design-tool/app.contract:361` and `apps/design-tool/app.contract:713`.

An empty fact set is sent as `pin: null` with no `build`. The server deletes every build's pin, whoever set it (`scripts/tamber-builds.mjs:251`). The form opens on "Every build" and empty scheme, contrast, mode, and font, and Pin stays enabled. One click clears the phone and the canvas frame; the note even says "Pins handed back". Hand back is the other button.

Fix: have Pin refuse to send when no fact is set, and send `pin: null` only from Hand back.

**4. Hand back never shows a lag.** `apps/design-tool/app.contract:700`.

The "pinning / pinned" line is rendered only when the pin text is non-empty. A null pin is announced in the same `builds` event, before the build applies it. The row drops the pin immediately while status still shows the pinned scheme, so a slow device looks as if it has already let go.

Fix: while `sent` is ahead of `status.revision` and the server pin is null, keep the previous facts and show them as handing back.

**5. The stub does not match the server, and the tests miss the real order.** `apps/design-tool/stub/tamber-stub.ts:55`.

The real `preview` replaces the whole pin with `{mode: {axis: value}}` (`scripts/tamber-server.mjs:467`, `scripts/tamber-builds.mjs:251`). The stub merges into the existing pin, so a scheme pin survives a preview in the contract test and would not on the server. A pin is announced as `build-status`, not `builds`, and any non-empty `client` is accepted. There is no 409 when that client has no long poll. `app.test.contract` therefore never exercises the event-before-reply order, a replacing preview, a missed log, or a new epoch. The drive in `scripts/design-tool.test.mjs:751` only checks that the polled build eventually sees `pin: null` within 60s of the page closing. An explicit null pin on close would pass too. `docs/testing.md` asks for a lower bound when the claim is that something did not happen immediately.

Fix: make the stub replace on preview, emit `builds` (status unchanged, so the row stays "pinning"), and refuse a pin whose `client` is not the long poll's. Point the contract test at a `builds` event that arrives before the reply is applied, and assert the hand-back frame is still the old pin for a short beat after the page closes and null only after that.

## Low

**6. A refusal stays on the row after the build has moved on.** `apps/design-tool/app.ts:930` and `apps/design-tool/app.ts:457`.

`unsupported` is cleared only when this client pins again. A later `build-status`, a reload that keeps the id (the server sets `status: null` and announces `builds`), or a new epoch all leave "Not applied: pin.contrast: …" up. After a reload the status line is also blank (`apps/design-tool/app.contract:702` joins empty strings) until the next report, with no sign that status has not arrived.

Fix: clear `unsupported` when a `build-status` revision catches `sent`, and whenever `takeBuilds` runs for a new epoch. When `status` is null, say that this build has not reported yet.

**7. The per-row Preview control reads as a tweak and is a full replace.** `apps/design-tool/app.contract:706` and `apps/design-tool/app.ts:1168`.

Preview sends one axis, and the server then drops scheme, contrast, root font size, and any other axis. Pin Dark and 20px, then choose Dim on that row: the device keeps only Dim. The root font field also allows 0–3 (`app.contract:731`) while the server accepts 4–64 (`scripts/tamber-builds.mjs:60`); those values only fail after the round trip.

Fix: label Preview as replacing the pin, or send a `pin` that copies the current facts and changes that one axis. Set the font field's minimum to 4.

The long poll and the pin use the same `design-tool:${person}` (`app.contract:374`), and nothing sends an unload. That part matches the brief.

Verdict: Do not land this step until the panel republishes `hello.builds` on a miss or a new epoch, and a pin stays "pinning" until `status.revision` catches the revision the tool just sent.
