# Entry zero mistakes an asset removal for current

**Status:** Closed
**Resolution:** Fixed by exact embedded roster equality and complete generation resolvers; store tests cover same-plan removal and readdition, Apple runtime and Linux selected-asset tests prove omitted names hide embedded bytes.
**Systems:** Update store, Delivery
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11; LLP 1030 D1/D3a; update/src/store.rs

The store decides that a signed head is already represented by entry zero when
its plan digest matches the embedded plan and every asset *named by the head*
matches an embedded asset (`update/src/store.rs:501-513`). It never proves the
reverse: that every asset in the embedded bundle is still named. An empty or
strict-subset asset list therefore passes `Iterator::all` vacuously.

This is reachable through the publisher, which treats an asset present in the
old head but absent from the new bake as a `removed` bundle change
(`scripts/deploy.mjs:254-264`). Keep `app.plan` byte-identical, delete one
asset, and publish the resulting signed head to a fresh binary that embeds the
old plan and asset. The classifier says `bundle`; `Store::check` returns
`Current`, downloads no files, advances `record.seq`, and leaves entry zero's
asset available.

Staging a non-current plan does not fully solve the semantic problem either.
Entries contain only the new head's assets, while the Apple and Linux
resolvers overlay those files on the embedded asset root. With no tombstone or
complete-generation boundary, a removed embedded name continues to resolve
from the binary. Production asset deletion is therefore not representable,
even though the classifier promises it as a bundle change. The dev loop's
parallel deletion defect is tracked separately in
`dev-loop-asset-deletion-is-a`.

Done when entry-zero equality compares complete asset rosters, not just the
new head's subset, and an activated/selected generation can explicitly hide
an embedded asset it removed. Tests must cover an identical plan with one
embedded asset removed (it is not `Current`), an updated plan whose removed
name no longer resolves on Apple and Linux, removing an asset from a stored
entry, and later re-adding the same name with new bytes.
