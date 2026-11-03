# A retirement notice on a Current head never reaches the app

**Status:** Closed
**Systems:** Update client and store (`update/src/client.rs`, `update/src/store.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A signed head that matches the embedded plan and asset roster can carry a sunset card, and the running app never reports it. `state.delivery` and `exactDelivery()` omit the notice during the check and after restart. The app keeps running the embedded generation with no retirement message.

**Cause.** `finish_check` returns `Check::Current { sunset: envelope.sunset }` and does not store the card. `Client::finish_check` maps `Check::Current` to `Outcome::Current` and drops the field (`update/src/client.rs`). `Store::status` reads sunset only from `self.view` (`update/src/store.rs`). `view` is set when a different payload is staged, and when a selected entry is loaded at open. A Current head writes no entry and no view, so the card is discarded. Tests that stage a head whose payload differs from entry zero do not cover this path.

**Reproduction.** A production binary whose embedded payload matches the stream head. The head is signed and includes `sunset.message`. After a successful check, `Check::Current` holds the card and `store.status().sunset` is `None`. Relaunch: still `None`.

**Fix.** Persist the accepted sunset with the floor witness (see the Current-head witness ticket) and return it from `status` while that stream is the one the process is on, including when the running generation is the embedded one.

**Acceptance.** The reproduction reports the message from `status` during the check and after a relaunch. Clearing the card on a later head clears `status`. A staged generation that is not yet running still reports the accepted stream's notice, as the comment on `status` describes.

**Independent verification (Codex).** At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, `/tmp/exact-review-trust-rust-repro.rs` checked a signed sequence-6 head matching embedded sequence 3's payload and carrying `sunset.message = "Install a new binary"`. The actual store returned `Check::Current` with the card, while `store.status().sunset` was `None`. This exercises the store/status path, not a native UI. Relevant locations: `update/src/store.rs:818-828`, `1071-1074`, and `update/src/client.rs:317-321`. No matching client issue was found in either older issue corpus.

**Resolution.** Status keeps the latest accepted stream advisory independently of the selected or running payload. A Current head can add, replace or remove it, and recovery restores it from the signed witness.

**Verification after the fix.** The 54 update integration tests passed, including Current status, record-loss recovery and clearing the advisory on a newer sequence.
