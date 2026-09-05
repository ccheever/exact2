# The deploy classifier does not use the artifact graph

**Status:** Closed
**Resolution:** Completed target build receipts and one shared artifact dependency classifier; duplicate reports closed together after the real build and publication matrix.
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


## Verification (2026-09-05)

Closed together with `20260904-dev-and-deploy-classifiers-diverge-from.md`: the two reports describe the same missing
producer-to-classifier boundary. The binary-producing bake now emits its
compatibility document and per-artifact requirements; the completed Cargo build
adds actual compiler-loaded inputs, selected compilation units, build-script
inputs, and host packaging inputs. Dev and deploy share the same comparison.
Native platforms without a completed receipt remain unbuilt; pending source
changes never acquire a guessed target, grant set, or compatibility identity.

Actual builds proved a host-only implementation edit leaves the id unchanged
and produces an independent binary row. A real signed Linux stream accepted
a changed plan at sequence 2 despite an unrelated data-crate binary change
moving the candidate id. Adding a new source demanded by the plan refused
that older stream by `app.plan: sources.cameraLocation`, leaving sequence 2
unchanged. A shader-body-only edit kept its interface and binary fingerprint.
The browser dev drive visibly accepted the changed plan and restored source.

The external path-dependent Weird Castle fixture used non-empty grants and
completed builds for wasm32, macOS arm64, iOS arm64 device and simulator,
Linux's host-native headless executable on this Mac, and x86_64 Linux. Every
completed receipt id matched the actual bake and embedded artifact. Web, macOS,
simulator, and host-native Linux receipt readers executed; device and x86_64
Linux checks inspected the built archive/ELF and defined receipt export without
executing those targets. The live external app was not changed.

Production analysis artifacts explicitly refuse launch before data construction,
grants, plan evaluation, update-store access, network, or first pixel. Actual
linked native refusal probes and Apple exported-ABI counter tests cover that
boundary; strict production release receipt requirements remain unchanged.
Signed stream receipts authenticate the frozen cohort requirements, and
publication rechecks them under the origin lock before writing blobs. Source
and canvas demands absent from the frozen plan's roster are conservatively
refused; this does not claim to infer unused native exports.

Evidence is retained in ignored `target/llp-ship/20260905-classifier-drives/`
and `20260905-classifier-deploy/` in the classifier worktree, and
`20260905-collector-matrix/` plus `20260905-analysis-refusal/` in the reused
contract worktree. The selected-unit fingerprint regression and the origin
protocol cases run in the existing `scripts/caps.test.mjs`; full workspace
and runtime delivery gates apply to the final integrated commit.
