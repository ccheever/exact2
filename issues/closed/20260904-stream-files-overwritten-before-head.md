# Stream publish overwrites live plan files before swapping the head

**Status:** Closed
**Resolution:** fixed by immutable blob URLs and a release-record-before-head commit boundary, with injected failure coverage
**Systems:** Delivery, exact deploy
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030.000 D3 item 5, LLP 1023 D3, LLP 1030.001, `scripts/deploy.mjs`

LLP 1030.000 D3 item 5: blobs first, then a conditional put of the signed head, so a client mid-publish sees the old head whose files are all present, or the new one — "nothing was replaced."

`publishStream` does write `.exact/blobs/<sha256>` and read them back (`scripts/deploy.mjs`). The head it signs then names `url: "./app.plan"` and `url: "./assets/" + name`, not the blobs. The store fetches those URLs (`update/src/store.rs` `obtain`, `envelope.rs` `resolve_url`). Immediately before `putHead` it overwrites exactly those paths.

A client holding seq N's head that re-fetches `./app.plan` in that window gets seq N+1's bytes and fails the digest (fail-closed locally; the check changes nothing). A crash between the stream puts and `putHead` leaves the previous head standing with its payload replaced: the last good bundle is no longer retrievable from the URLs the head still names. The blobs still hold the old bytes; nothing points at them.

That is the opposite of 1023 D3 / 1030.000 D3 item 5 whole-or-absent on the origin. The origin row has the same shape: `publishRoot` overwrites `/app.plan` then `/exact.json` in place (alphabetical, `index.html` last).

There is a second stream commit boundary after the head. The release record is
written only after `putHead` (`scripts/deploy.mjs:437-445`) and through an
ordinary overwriting put. If that write fails, deploy reports the stream
failed and says “the previous head stands,” although the new head is already
live. Reusing a release id can also replace what D3 calls an immutable receipt.

Point the head's `url`s at the blobs (or at seq-unique names), and stop mutating `./app.plan` / `./assets/*` under a live head. Named copies, if they stay for humans, go on after the head is in place or not at all. Define the release record as immutable/idempotent and order it so every reported failure is truthful about whether the new head is visible. Inject a failure before and after every write.
