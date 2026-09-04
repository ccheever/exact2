# The deploy classifier does not use the artifact graph

**Status:** Open
**Systems:** Delivery, exact deploy, Build
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030 D3; LLP 1030.000 D3/D5

`classify()` does not consume an artifact graph, build receipt, or source
delta. For each compatibility id produced from the current checkout it asks
only whether `app.plan` or an asset digest differs from that stream's head
(`scripts/deploy.mjs:313-334`). Every other compatibility id is
unconditionally a `binary` row (`scripts/deploy.mjs:337-366`).

That is not LLP 1030 D3's classifier. In particular:

- A host, painter, runner, or exact2 fix that leaves the compatibility inputs
  stable and does not change plan/assets produces `current`, with no binary
  row. Baked policy such as `deploy.activate` can change the binary's behavior
  without moving the id too.
- An older cohort always produces `binary`, even when the candidate plan and
  assets are dependency-closed over what that cohort already provides. This
  withholds an independently publishable bundle merely because an unrelated
  binary layer changed in the same snapshot — the exact co-occurrence rule D3
  rejected.

The compatibility id answers whether two clients have the same interface. It
does not answer whether this source delta requires a binary, nor whether a
candidate is safe for a different compatible interface.

Done when bake emits the artifact/dependency graph and receipts D3 assigns to
it, and both dev and deploy classify from that one output. Tests must include:
a host-only implementation edit yielding a binary row without an id change;
a candidate that remains publishable to an older cohort despite an unrelated
binary change; and a candidate that actually requires a new capability being
refused for the old cohort by the named dependency, not by id inequality.
