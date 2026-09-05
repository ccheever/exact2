# Deploy and dev compute compatibility ids that disagree with the bake

**Status:** Closed
**Resolution:** Completed target build receipts and one shared artifact dependency classifier; duplicate reports closed together after the real build and publication matrix.
**Systems:** Delivery, Dev loop
**Severity:** P1
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** host/web/dev.mjs:204,212,229; scripts/deploy.mjs:210 compatOf; contract/cli/src/main.rs:44-47; contract/cli/src/compat.rs:309,311; llp/1030.000-dev-server-as-deployer.rfc.md

Two divergences, both verified. (1) `dev.mjs` carries its own conservative classifier plus `classifyRebuild` instead of importing the deploy classifier (RFC 1030.000 owed “dev.mjs importing this classifier”); two classifiers can drift, and `dev.mjs` never publishes so divergence is silent. (2) Both script classifiers call `contract compat` with no `--target` and grants `None` (`grantCeiling: null`), while bake uses the real `TARGET` plus the data crate's grants. A rebuilt Linux binary's `OUT_DIR/compat.json` was `030646b901a2bd78a9fcdf21d86ffd8b` (`aarch64`, `grantCeiling: ""`); the CLI used by deploy returned `2f2aee5a5b0a397f5a799e9b3cc1fcd` (`aarch64`, `grantCeiling: null`). For web the second computation also says the publisher's architecture instead of `wasm32`.

Effect: deploy publishes streams under ids the built binary's baked `head_url`
never checks, and a grants-only change moves the baked cohort while both script
classifiers say it did not. `host/apple/build.mjs --embed` has the same second
computation: it supplies a target but no grants, so the `compat.json` shipped
beside the archive can disagree with the string compiled into that archive
(`host/apple/build.mjs:282-294`). Its receipt does not carry either id.

Do not make another caller reconstruct grants and targets. The binary-producing
bake must emit the compatibility document and build receipt that deploy,
`--embed`, and the dev classifier consume as their sole authority; assert that
the emitted id equals what the binary exposes. Then move both scripts onto one
classifier. Cover wasm32, macOS, iOS device and simulator, Linux, a non-empty
grant set, and an external path-dependent app.


## Verification (2026-09-05)

Closed together with `20260904-deploy-classifier-does-not-use-artifact-graph.md`: the two reports describe the same missing
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
