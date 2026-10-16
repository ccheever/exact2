# Code review: LLP 1122 T2 step 2 (builds panel and pins), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `39d166329`.
- **Method:** one brief (sha256 `444f48e239cd240a54d03fb905137a0cbbeccc495d9175adaabe81d4bd4e8242`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (6 medium, 1 low).
- **Disposition:** taken: builds come only from the server: hello's list, read inside the poll's answer whenever the poll starts again (the first answer, a gap, a new run), so the panel shows it at once, and a new run drops the old list; each listed build now carries `pinRevision` (tamber/1.3, scripts/tamber-builds.mjs: the revision of the last pin frame it was sent, a pin, a hand-back or its pin again on registering, whoever sent it), and a row is caught up only when its status reports that revision (never before its status comes), so another client's pin, a reload, a new run and a pin announced before its reply all read right with no local bookkeeping; a hand-back shows "handing back" until caught up; a build that has not reported says so; an unsupported answer stays while the pin it answered is the build's pin; Pin needs something to pin (only Hand back sends null); the preview is "Preview only", and the docs say it replaces the pin; the font field's minimum is 4; the stub replaces on preview, announces `builds` with `pinRevision`, and refuses a client that has not polled; the contract tests cover pinning then pinned, handing back, a preview replacing a pin and its refusal, another client's pin, a build not reported yet, and a new run's builds; the real drive keeps the tool open past one lease with the pin held, and checks the hand-back lands 10-50 s after the tool closes. Not tested: the stub's refusal of a client with no poll through the tool (the tool always polls; the server's own test has it).

---

Read-only review completed; no files changed or tests run.

1. **Medium — Catch-up tracks the last local command, not the current pin.** [apps/design-tool/app.ts:452](/tmp/rv-dt-t2b/apps/design-tool/app.ts:452)  
   If another client replaces revision 5 with revision 6, the row retains `sent=5`; status at revision 5 therefore labels the new pin “pinned.” Disconnecting also discards `sent`, so a reconnected build with `status:null` is immediately considered caught up. Conversely, a new server epoch can retain an old revision for the same build ID. **Fix:** track the current pin’s expected revision per epoch and build, retain it across reconnects, and show unknown revisions as awaiting status.

2. **Medium — An old `hello` response can overwrite newer events.** [apps/design-tool/app.ts:675](/tmp/rv-dt-t2b/apps/design-tool/app.ts:675)  
   `loadHello` replaces the shared map without checking its request generation, epoch, or event cursor. A delayed snapshot containing a connected build can arrive after the disconnect event and a newer snapshot, restoring a ghost row. Discarding the resource’s stale response does not undo this module-side mutation. **Fix:** serialize snapshot reconciliation with event processing, or reject snapshots superseded by a newer snapshot or cursor.

3. **Medium — Snapshot and pin acknowledgments do not update the displayed rows.** [apps/design-tool/app.ts:1174](/tmp/rv-dt-t2b/apps/design-tool/app.ts:1174)  
   The panel renders `changes.builds`, which is materialized only when `waitChanges` returns. Updating `buildsNow` from `hello` or a pin acknowledgment does not publish new rows. After missed events, a refreshed snapshot can remain invisible until the next 25-second poll timeout. Similarly, if the pin event precedes its acknowledgment, “pinned” can remain displayed after `sent` changes and proves the status is behind. **Fix:** publish build rows whenever snapshots, events, or command acknowledgments change them.

4. **Medium — A successful pin response can erase its own unsupported report.** [apps/design-tool/app.ts:1174](/tmp/rv-dt-t2b/apps/design-tool/app.ts:1174)  
   The acknowledgment unconditionally clears `unsupported`. If a web build reports `pin.contrast` through the poll before the pin HTTP response reaches the tool, that response deletes the newly received diagnostic. Subsequent rows then omit the required explanation. **Fix:** associate diagnostics with event or command ordering and clear only diagnostics predating the command. Cover the report-before-acknowledgment ordering.

5. **Medium — Hand-back hides pending application.** [apps/design-tool/app.contract:700](/tmp/rv-dt-t2b/apps/design-tool/app.contract:700)  
   The catch-up indicator exists only when `b.pin != ""`. After handing back a dark pin, the server announces `pin:null` before the build necessarily applies it. The row immediately loses its pending indicator while still reporting dark, making an unapplied hand-back indistinguishable from the app choosing dark. **Fix:** display “Handing back” whenever the clear revision remains unapplied, independently of whether a pin exists.

6. **Medium — The stub implements different pin semantics.** [apps/design-tool/stub/tamber-stub.ts:61](/tmp/rv-dt-t2b/apps/design-tool/stub/tamber-stub.ts:61)  
   The stub merges a preview into existing pins and emits `build-status`. The real server replaces the entire pin with `{mode:{axis:value}}` and announces a full `builds` snapshot. Thus previewing an axis after pinning scheme/font size behaves differently, and the contract tests bypass the real pin-update reconciliation path. The stub also accepts any nonempty client without establishing presence. **Fix:** match the existing server behavior and test a preview replacing a nonempty pin.

7. **Low — The lifecycle test does not establish the promised lease behavior.** [scripts/design-tool.test.mjs:753](/tmp/rv-dt-t2b/scripts/design-tool.test.mjs:753)  
   The assertion permits cleanup after 60 seconds despite claiming 35; cleanup at 50 seconds passes. It also never holds the tool open beyond the lease to prove continued polls preserve pins. **Fix:** measure cleanup from page closure against the stated deadline, and separately keep the page open beyond one lease while asserting the pin remains.

Verdict: Request changes for build-state consistency, acknowledgment ordering, and protocol-faithful tests.
