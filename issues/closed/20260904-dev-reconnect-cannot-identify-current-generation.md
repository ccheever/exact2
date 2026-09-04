# The dev reconnect cannot identify the current generation

**Status:** Closed
**Resolution:** Fixed by epoch/program identity and canonical complete manifests with immutable payload URLs; browser and modular native runtime drives prove initial/reconnect repair, reversed-fetch refusal, restart ordering, unchanged reconnect, all-session acceptance and explicit-apply invalidation.
**Systems:** Dev loop, Web host, Apple host
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1023 D3; LLP 1030 D10; LLP 1031 D11

The dev stream's revision is process-local and describes only the plan.
`dev.mjs` starts `seq` at zero and its hello contains `{seq, digest, baked}`,
where `digest` is only `app.plan` (`host/web/dev.mjs:54-62`). Asset edits
increment seq but do not change that digest, and the hello carries no asset
manifest even though `/exact.json` has one.

Permanent staleness and rollback paths follow:

- Apple's initial envelope path parses the plan and events URL, but never the
  envelope's `assets` array (`PlanURL.swift:325-369`). A host that connects
  after an asset edit receives no historical asset event and keeps its
  bundled or cached bytes even on a clean first connection.
- If an SSE disconnect spans an asset edit, Apple compares only the unchanged
  plan digest and does nothing (`PlanURL.swift:482-485`); the web client has no
  current asset list to reconcile either.
- If `dev.mjs` restarts while a browser page survives, the page retains its
  `queued` seq and discards every new-process revision until the reset counter
  exceeds it (`host/web/dev.js:11-13,83-97`). There is no server epoch.
- Apple starts every asset fetch from one event independently, and every event
  on a connection uses the same `resolution` generation
  (`PlanURL.swift:200-222,477-481`). Two quick edits to one asset can complete
  in reverse order; the older response then becomes the live override. A
  multi-asset edit also becomes visible one file at a time rather than as the
  generation the server announced.

The hello therefore does not fulfill LLP 1023 D3's promise that reconnect
heals a missed event. Deletions are separately tracked in
`dev-loop-asset-deletion-is-a`.

Done when a revision identifies a complete generation — plan plus the asset
manifest — and includes a server/session epoch or another ordering identity
that cannot move backward on restart. On initial connect and reconnect both
clients stage and commit that generation transactionally, including removals;
an older completion cannot overwrite a newer one. Drive: connect after an
asset edit; miss an asset event then reconnect; deliver two edits' fetches in
reverse order; restart the server after N edits without reloading the page;
and reconnect when already current. Every client must converge once on the
server's full current generation.
