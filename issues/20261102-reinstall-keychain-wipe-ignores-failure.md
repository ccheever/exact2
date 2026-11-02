# A failed reinstall wipe is recorded as done

**Status:** Closed
**Systems:** Apple locked secrets (`host/apple/src/presence_secrets.rs`, `host/apple/Sources/ExactKit/Presence.swift`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** On iOS, a reinstall is supposed to delete the previous install's locked items. The data-protection keychain survives app deletion and the Application Support marker does not. If that delete fails, or if the record listing fails, the marker is still written. Later launches see the marker and leave the previous install's items in place.

**Cause.** `first_launch` runs only when `Library/Application Support/exact/installed` is absent. It deletes the two services when both `kv.keys` calls succeed and return no keys. `call(4)` is `SecItemDelete` of a service (`Presence.swift`). Not-found is success; any other status, including a locked keychain, is a failure. The `Result` is discarded, and `std::fs::write` of the marker runs either way. A `keys` error makes `empty` false, so the delete is skipped and the marker is still written.

**Reproduction.** Install, store a locked item, delete the app, reinstall, and launch once while `SecItemDelete` fails. The marker exists and the item is still in the keychain. Launch again after the keychain is available. The item remains.

**Fix.** Write the marker only after both deletes return success or not-found. Leave it absent when a delete fails or when `kv.keys` fails, so the next launch tries again.

**Acceptance.** A failed delete leaves no marker, and the next successful launch removes the items and then writes the marker. An update that already has records does not delete items and does write the marker.

**Resolution.** Write the first-launch marker only after both record lookups and any required keychain deletes succeed. Failed cleanup leaves no marker so the next launch retries; an update with existing records retains its secrets.

**Verification after the fix.** The Apple Rust library tests passed. The existing keychain test now checks failed deletion, successful retry, denied record lookup, and retained secrets during an update using the deterministic keychain hook.
