# A same-launch fallback to entry zero still reports the refused bundle's seq

**Status:** Open
**Systems:** Delivery, Apple host, Linux host
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11, LLP 1030 D7, `update/src/client.rs`

Both hosts, on a selected plan that fails decode/boot, call `entry_refused` and then boot the embedded bytes in the same launch (`host/apple/src/abi.rs` `boot_selected`; `host/linux/src/app.rs` `boot_presenter`).

`Client::entry_refused` only clears `booted_selection` so first pixel will not bless the entry — correct for the crash counter. It does not set the store's running entry to none/zero. `Store::status` still reports `running_seq` of the refused bundle. `status_into` therefore fills `delivery.seq` / `stream` as if that bundle were on screen. The runner's own comment is "the entry running now." After a same-launch fallback the banner and `state.delivery` can disagree with the tree.

The failure *count* is left standing, which is what D11 asked; the running facts are not.

On `entry_refused`, set the client's running identity to entry zero (seq of the embedded bundle, stream `"embedded"`) without clearing `record.selected` or `failures`. Refresh the snapshot after that so the first `set_delivery` is honest.
