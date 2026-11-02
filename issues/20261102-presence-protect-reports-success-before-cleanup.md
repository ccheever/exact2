# Secret protection succeeds while its ungated copy remains

**Status:** Closed
**Severity:** P2
**Systems:** `runner/src/presence.rs`, `host/apple/src/presence_secrets.rs`
**Author:** Codex
**Date:** 2026-11-02
**Commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

`Backend::protect` adds the locked item, then logs a failed `Vault::put_plain(name, None)` but still sets occupancy to `Locked` and answers `Done::Ok` (`runner/src/presence.rs:771–792`). The completed job is removed, and a later protect exits successfully because occupancy is already locked (`runner/src/presence.rs:729–733`). No cleanup retry remains.

The app can therefore tell a person that protection succeeded while the previous copy still lives in the store without the new presence policy. This means an ungated copy, not necessarily unencrypted storage. Apple's real `KeychainVault::put_plain` propagates errors from `secrets.forget` (`host/apple/src/presence_secrets.rs:202–208`), so the error branch is part of the host path.

**Reproduction.** `/tmp/exact-runner-presence-delete-probe.rs` links this checkout's warm runner rlib. A public `Vault` wrapper delegates to `MemoryVault`, except that deleting the plain fixture returns a temporary error. Start an optional user-presence name with a plain value, enqueue `Protect`, and pump. Assertions confirm `Done::Ok`, `Locked` occupancy, and both plain and locked copies. Let deletion succeed, pump at 1 s, 10 s and 60 s, then protect again: every assertion still finds the plain copy. The log says both that the copy remains and that the secret is protected. No platform keychain, real secret, process runtime, or network was used; the probe validates backend behavior under the documented vault error, not the frequency of a platform failure.

**Recommendation.** Keep the cleanup queued until deletion succeeds, with an explicit pending/failure result while the ungated copy remains. A repeated protect should resume cleanup. Apply the same durable-deletion rule to reconciliation and migration paths that currently discard deletion errors.

**Deduplication.** Searched current `issues/` and both older checkout corpora, including closed issues. `20261102-reinstall-keychain-wipe-ignores-failure.md` concerns install-marker cleanup of old locked items; this failure is a live protect operation leaving its ungated copy and falsely completing.

## Resolution

Protection, migration and reconciliation stay pending and retry failed durable cleanup. They report a locked or absent value only after removing the plain copy; retries reuse the existing locked generation.

## Verification

The runner library suite passed 615 tests, including `protection_and_migration_wait_for_durable_plain_deletion` and the existing interrupted-operation tests. The regression injects deletion failure during protection, reconciliation, migration and migration on a device that cannot lock, then verifies recovery without an extra generation.
