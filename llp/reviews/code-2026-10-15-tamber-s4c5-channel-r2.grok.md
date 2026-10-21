# Code review: stage 4 commit 5: the build channel, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the change commit and the first-round commit.
- **Method:** one brief (sha256 `7ab1f729495ada05cf721d239a5ba5a0f81fee3170afdd91ce7a212fe1557230`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking findings; two P2 (Linux polling behind plan downloads; JS status after a live batch) and a P3 (the doc's structure field).
- **Disposition:** applied in the second-round commit.

---

Nothing blocking. No P0 or P1. Two defects should be fixed, and one spec comment is wrong.

## Findings

[P2] Poll the build channel within 5 seconds even while a plan download is in flight — `host/linux/src/fetch.rs:185`

The commands poll runs only after `fetch_changed` returns, and that request sets no timeout. The server drops a polled stream that has not polled for 5 seconds (`scripts/tamber-builds.mjs:34`, `sweep` at `scripts/tamber-builds.mjs:61`). A generation or module download, or a stalled envelope GET, longer than that removes the Linux build from `list()`. A pin to every build in that window is not stored for it (`scripts/tamber-builds.mjs:160`), and the re-register replays the previous pin, or none. The 500 ms delay is applied only after the worker finishes (`host/linux/src/fetch.rs:165`), so it does not keep the poll inside the 5 second window.

[P2] Report the JS build's design again after a live batch or snapshot — `host/web-js/dev.mjs:231`

`live` applies the batch and, when stale, the snapshot, then stops. It never calls `status()`. The web page does (`host/web/dev.js:276`), and Apple and Linux report a design change too. The server updates a build's `design` only from `register` and `status` (`scripts/tamber-builds.mjs:132`). After a value-only edit, the JS build stays listed on the previous digest until a pin or a color-scheme or contrast change happens to send status.

[P3] Say that a batch's `structure` is the design shape — `docs/tamber-protocol.md:436`

Hosts apply a batch or snapshot when that field equals `design::shape` of the running plan (`runner/src/runner/live.rs:53`). The protocol still says a snapshot applies when its structure is the plan's, and the live-table section defines that word as the plan without its design values (`docs/tamber-protocol.md:420`). Those two digests differ across the JS target and a native build of the same sources, which is why the wire field was changed. The same stale description remains at `runner/src/design.rs:7` and `runner/src/design.rs:36`, next to the struct comment that already says the field is the shape.

## Assessment

The round-1 fixes and the rulings hold in the code. The nonce is 128 bits from `crypto.randomBytes`. `/__dev/report`, `/__dev/hello`, and `/__dev/commands` sit behind the development gate on both loops. A browser POST whose `Origin` host does not match the request host is refused. Polled streams are capped at 64 and expire after 5 seconds of not polling. Builds are capped at 256. A report for an expired nonce is 403, and a report before `register` is 409. Production builds do not open a channel: Apple keys off `compat.json`'s `inputs.trust`, Linux off the same check, and neither looks at `EXACT_DEV_PLAN`.

Pins are cleared when a client's last subscription closes or it has not been present for 35 seconds. A build that registers again is told the pin still stored, or null. A second stream that claims the same build id takes it. The relay stops when the device request closes, and a nonce the main loop no longer holds ends the device stream. The resident `--serve-as` loop mounts no design server and broadcasts no batches of its own.

On each host, the first pin records what it replaced, a later pin replaces the whole set, a refused mode restores those earlier modes, and a session or generation that boots under a pin gets it again. Scheme, contrast, and root font size follow `prefer`. Status on the web, Apple, and Linux is the effective facts. Reports wait until registration, and a new hello drops a stale nonce. Command kinds return before any generation handling on the web page, the JS page, Apple, and Linux. Changed files are within the 1,500-line cap (`Session.swift` is the closest, at 1,499).

The expiry test still advances the clock by 31 seconds and its comment still says 30 seconds (`scripts/tamber-builds.test.mjs:82`), so it passes if the unpolled limit regresses to 30 seconds. The same 30 second figure is left in `scripts/tamber-builds.mjs:83` and `host/linux/src/build_channel.rs:7`. Report bodies are capped at 64 KiB, but the read has no deadline and an oversize body is ended without destroying the socket (`host/web/dev.mjs:1153`, `host/web-js/dev.mjs:258`).
