# Code review: LLP 1122, the design tool's reads follow the files, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `a1c3ab7ee`.
- **Method:** one brief (sha256 `d573ba017a051fa8a9185461be17e56ec16c2948a2ca311103c8f8c3d355769a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the run compacted its context once).
- **Verdict:** nothing blocks the read; one medium (the proposal's chip), one low (a comment's cause).
- **Disposition:** both taken, in `c06d9ee5b`:
  - the proposal is keyed on the builds running (id, host, plan digest) and `hello.live`, with a contract test;
  - the low was right: a driver's `request` does not make the app read again; the one describe the first test let through was the first `advance` publishing the files' key. Both tests now step past that first `advance` before the events, the stub's `pass` defaults to none, and the comments say so.
- **Not taken:** a 15 s propose or axes describe behind a long batch: with four resolves in flight (`b9e3b1c3c`) and rt1's params cache, that queue is short.

The design read now follows file changes, and that part holds. The proposal was keyed the same way, so its Live/Rebuild chip can stay wrong after a rebuild that does not touch a design file.

[P2] Re-propose when class or the live table changes without a design-file digest — `apps/design-tool/app.contract:562`

`proposeEdit` now runs only when the draft, `filesAt`, or `epoch` changes. `class` is computed on the next `propose`, not stored in the file digests. The server answers `rebuild` when the app’s build stopped before the design (`contract/cli/src/tamber_serve.rs:1172`), and the JS loop downgrades a live edit that moves a token written inline once the design reader is built (`scripts/tamber-server.mjs:552`, `host/web-js/dev.mjs:617`). Neither of those changes the `.tamber` digest. A contract edit or a reader that finishes linking reloads pages and the poll delivers `builds` / `build-status`; `noted` ignores those, so `filesAt` stays put.

Open a draft of `text.body` (a font-weight number) while the chip says Live and Scrub live is enabled (`app.contract:1304`). Edit the contract so that token is inline, or break the build before the design. The chip stays Live (`app.ts:807`). Scrub live is still offered; the server answers 409 and the tool shows `No scrub: …` (`app.ts:302`). Apply still writes the bytes already reviewed. The other way, a build that starts succeeding again leaves the chip on Rebuilds and Scrub disabled until the draft or a design file changes.

The same key misses the live table. `liveTable` is captured only inside that propose (`app.ts:667`, `app.ts:807`). `advance` already refreshes `hello` when `seq` moves, so `hello.live` updates and the chip does not. On the wasm loop `live()` flips across a successful rebuild (`host/web/dev.mjs:390`, `host/web/dev.mjs:1213`). After that, a live edit keeps a Rebuilds chip and Scrub stays disabled.

[P3] The allowed describe is the first clock step, not the driver request — `apps/design-tool/stub/tamber-stub.ts:298`

`pass: 1` matches this test as written, and the comment names the wrong cause. `request` only fetches the stub (`scripts/agent-test.mjs:605`); it does not read the app. This test has not run `advance` yet, so `filesAt` and `epoch` are still empty. `clock +1` runs that first `advance`, publishes them, and re-keys `loadDesign` once. The injected `builds`, `build-status`, and `scrub` events are not `changed`, the session is already live, and the later `clock +3000` does not read again. A second describe is refused, which is what fails the old `since` key. The restart is required: `refuseDescribe` is process-wide and the next test needs describe to succeed.

The design read, the axes read, and the snapshot dependents still follow the cases that change bytes or the server run. A write from this tool, another client, a hand edit, or a git checkout is a `changed` event, and `noted` bumps `fileChanges` (`app.ts:598`). A restart is still an `epoch` key, and a missed-events gap takes `anew`, which bumps the count and replaces `lastFiles` from the snapshot (`app.ts:1011`). Choosing another file is still the `file` argument. Accept, undo, and redo publish `changed` when bytes move. Revert only holds a proposal until accept. A commit is a `committed` event and does not change digests; the changeset stays keyed on `since`, so the committed mark still appears. `design.answer` moves when that read finishes, so contrast, branch, highlights, the typed fields, `draftStale`, and `partsStale` still refresh after a file change and no longer rerun on a build status. Branch was already keyed only on `design.answer`. The selection and the changeset still rerun every poll (`changes.answer`, `since`).

`READ_WAIT` of 120s is enough for the clone that prompted this (73 tokens, about 15s of resolves) and is the right budget for the queued resolves. It equals the server’s own reply wait (`ANSWER_MS` in `scripts/tamber-server.mjs:92`). Writes use 130s so the client outlasts that wait; a resolve the server answers at 120s is reported as no server (`app.ts:84`) and `advance` then refreshes the design. The describe and the trailing hello inside the same `readDesign` do not need 120s: describe is sent before the resolves, and hello is sent after those replies are in. A propose or an axes describe issued while a long resolve batch is already queued still uses the 15s default and can fail, then stick until `filesAt`, `epoch`, or the draft changes.

Nothing here blocks the read-skip. The chip is the defect to fix: key the proposal on a build generation or `hello.live`, and keep the design read on `filesAt`.
