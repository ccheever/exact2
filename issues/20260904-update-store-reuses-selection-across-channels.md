# The update store reuses a selection across channels

**Status:** Open
**Systems:** Update store, Delivery, Apple host, Linux host
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11; LLP 1030 D3a

Channel is deliberately excluded from the compatibility id, so a beta and a
production binary can have the same id (`contract/cli/src/compat.rs`). The
durable store is scoped only to `<base>/exact/<app id>/update`
(`update/src/client.rs:74-93`), and its record persists compatibility id and
seq but no channel (`update/src/store.rs:170-221`).

At open, `Store::validate()` accepts a selected entry when its digest, app id,
and compatibility id match. It does not compare
`envelope.stream.channel` with the newly baked channel
(`update/src/store.rs:655-676`). The network check does compare channels, but
that happens after launch. Rebuilding the same app/cohort from `prod` to
`beta`, or back, can therefore boot the other channel's selected bundle before
the first check.

Done when channel is part of the store selection namespace or admission key,
and a channel change resets/refuses old selection state before boot. This is
pre-1.0, so replace the record shape rather than add a compatibility shim.
Test one app and compatibility id with a selected prod entry, then reopen it
with a beta `Embedded` and prove entry zero (or a beta selection) boots.
