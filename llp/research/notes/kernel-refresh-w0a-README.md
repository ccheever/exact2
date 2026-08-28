RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# RFC 0491 W0-A census pack

This directory is the non-mutating hand-verified census and draft-schema slice of Accepted RFC 0491 W0-A. The generated break package is at `tests/protocol/exwf/`, owned by LLP 0507 and generated as LLP 0514 M4 WP0; `tests/protocol/exwf/break-package.manifest.json` declares exactly two deferred companions, `canonical-consumer-census` and `breaking-window-ledger-rows`, which this pack drafts. Boundary rule: a generated artifact is referenced, joined, and checked here but never copied or restated; recovery classes, identity events, identity carriers, refusal identities, schema-package content, and published tuples remain generated-authority facts.

`prop-model-census.md` serves W0-A's WS-A/WS-B current-model inventory bullet: it explains the current prop-ID map, dispatch reachability, string-key probes, and pinned `Node` layout, with the commands that reproduce each count.

`prop-model-census.json` serves the same WS-A/WS-B bullet as the machine-readable prop-ID, `PropValue`, string-probe, and `Node` shape census consumed by later table/codegen work.

`ffi-surface-census.md` serves W0-A's WS-E FFI-arity/field-order preparation bullet: it summarizes exported functions, process-global callback generations, hand-written opaque handles, and exact literal status-collapse evidence.

`ffi-surface-census.json` serves that WS-E bullet as the machine-readable complete `#[no_mangle]`/public C-export census with per-file totals and callback/global anchors.

`wire-table-census.md` serves W0-A's WS-B/WS-D declaration-authority inventory bullet: it records style-mask occupancy, hand-ordered TypeScript vocabularies, four SetStyle implementations, the trailing-byte discard, and the live sidecar-ID join.

`wire-table-census.json` serves that WS-B/WS-D bullet as the machine-readable style/vocabulary/implementation census, with every sidecar checked against both the opcode inventory and the generated recovery authority.

`ws-f-dead-code-candidates.md` serves W0-A's read-only WS-F population-zero preparation bullet: it distinguishes verified private dead code, unproved public/dependency surfaces, and live migration candidates without deleting anything.

`ws-f-dead-code-candidates.json` serves that WS-F bullet as the machine-readable candidate/spans/evidence table; `verified` means the stated deadness was established, not merely suspected.

`consumer-census.md` serves W0-A's canonical-consumer-census and exact-once census↔ledger preparation bullet, including the concrete consumer join for every generated recovery-class family.

`consumer-census.json` is the manifest-deferred `canonical-consumer-census` draft: stable IDs cover current producers, hosts, transports, tests, agents, App-ABI/archive paths, and the explicitly unverified Ibex boundary.

`semanticframe-schema.draft.md` serves W0-A's WS-H SemanticFrame-schema bullet: it proposes the owned IR shape, semantic equality, captured nondeterministic ingress, generated-authority identity references, recovery/watermark evidence, bounds, retention, and privacy projection.

`ledger-row-schema.draft.json` is the manifest-deferred `breaking-window-ledger-rows` companion schema: it serves W0-A's machine-readable ledger and exact-once bidirectional census↔ledger join bullet without minting ledger rows.

`DISCREPANCIES.md` serves W0-A's honesty requirement by collecting every RFC-0491-versus-tree discrepancy found during this census, with reproduction commands and without changing the governing RFC.

`performance-baseline.capture.json` serves W0-A's fingerprinted pinned pre-change performance-baseline bullet (landed 2026-08-25 as W0-A debt): the machine-generated capture from `kernel/benches/w0a_baseline.rs` on the pinned box (Bones, per Charlie's 2026-08-25 ruling via exact-9e), covering the layout, mutation, export, allocation, and bytes-per-node axes over the pinned 1K fixture with the session A/A noise floor, host/toolchain/tree-object fingerprint, and the legacy-oracle freeze-pin join.

`performance-precommitment.json` is the machine-readable precommitment companion: per-axis baseline values, the derived "no worse than baseline beyond the recorded noise floor" regression limits on every axis, the positive targets on layout and allocation only, and the trend-only wake→presentation device disposition. Its `status` field is the signature state; while `draft-awaiting-charlie-signature`, the numbers are drafted, not ratified. It reads **`signed`** as of 2026-08-26: Charlie Cheever ratified the drafted numbers as drafted (exact-9e packet ruling item 2, recorded @11992d827), so the per-axis regression limits, the layout+allocation positive targets, and the trend-only wake->presentation disposition now bind the RFC 0491 Phase 1-5 gates.

`performance-precommitment.md` is the human-readable precommitment document Charlie signed on 2026-08-26 (its signature box is checked), derived one-to-one from the capture and the JSON companion; `scripts/check-kernel-w0a-performance-baseline.mjs` (registered as `kernel-w0a-performance-baseline`) validates the join between all three files and the freeze pin.

The LLP 0487 corpus harness over the current kernel has now landed as the registered `choice-corpus-kernel-harness` check plus the manual `choice-corpus-kernel-differential` evidence check. The fingerprinted performance baseline landed 2026-08-25 (capture plus precommitment above) and its numbers were **signed as drafted on 2026-08-26** (@11992d827), discharging that debt item completely. The 88-bit style-order fixtures landed 2026-08-26 as the registered `kernel-style-order-fixtures` gate (`tests/protocol/style-order-table.generated.json` plus the crate-internal `kernel/src/protocol/style_order_gate.generated.rs`), discharging WS-B's fourth and last named interim non-mutating gate. **The three pinned tape apps are now the only open W0-A debt owed before Phase 1 entry.** Three items the W0-A non-mutating rule deferred landed with RFC 0491 W0-B: the differential fuzz harness (the pre-existing `kernel/fuzz/` no-panic crate extended with `parser_differential` and `style_decoder` targets, plus the `kernel-w0b-differential-fuzz-smoke` registered check), the legacy-oracle content-addressed freeze (`legacy-oracle-freeze.json`, checked by `kernel-legacy-oracle-freeze`), and registration of this pack in `exact-verify.json` (`kernel-refresh-w0a-pack`).

Pack membership is checked with:

```sh
find docs/kernel-refresh/w0a -maxdepth 1 -type f -print | sort
jq -r '.breakPackageScope.deferred[]' tests/protocol/exwf/break-package.manifest.json
```
