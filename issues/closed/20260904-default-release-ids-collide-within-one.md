# Default release ids collide within one second

**Status:** Closed
**Resolution:** generated ids carry a random nonce, every bake has a private stage, and reused immutable receipts are refused before stream writes
**Systems:** exact deploy, Delivery
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030 D3a; LLP 1030.000 D3; scripts/deploy.mjs

The default release id contains the UTC timestamp only to whole seconds plus
seven hex digits of the commit (`scripts/deploy.mjs:176-177`). Two deploys of
the same commit begun in one second receive the same id. That id is also used
as the supposedly run-specific local directory
`target/deploy/<release>` (`scripts/deploy.mjs:477-481`).

Before every bake, `bake` recursively removes that directory
(`scripts/deploy.mjs:181-185`). Concurrent invocations can therefore delete
or replace each other's staging tree while the other process is building or
reading it. The per-stream origin lock is acquired only later, so it cannot
protect the bake. The deploy smoke avoids the defect by assigning its racing
publishers `r-race-a` and `r-race-b` explicitly.

The same namespace names `releases/<release>.json`. As recorded in
`stream-files-overwritten-before-head`, those receipts are ordinary
overwriting puts even though LLP 1030.000 D3 calls them immutable. A collision
can consequently conflate two runs locally and overwrite their audit record
remotely.

Done when a run has a collision-resistant identity and private staging
directory independent of the human-facing release correlation id. Generated
release ids must also be collision resistant, and an explicit reused id must
either be proven byte-for-byte idempotent or refused before any publish.
Drive two default invocations through a barrier so they choose their ids in
the same clock tick; both must retain separate bakes and immutable, truthful
receipts.
