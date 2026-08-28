RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# Canonical consumer census

This is the W0-A draft of the `canonical-consumer-census`, **promoted 2026-08-26** (LLP 0506 D1(a)) to the generated authority `tests/protocol/exwf/consumer-census.json`. This file remains the hand-verified row source; the generated authority owns the coverage-discovery join, the anchor resolution, and the exact-once ledger join. Its machine authority is `consumer-census.json`: every row has a stable kebab-case ID, producer/consumer role, closed tier, source anchors, protocol families, current gap-check status, and at most one join to a generated recovery family. The generated `tests/protocol/exwf/recovery-classes.json` remains the sole authority for class assignment and disposition.

The JSON currently has **31 rows**. Reproduce the count and field closure with:

```sh
jq '.consumers | length' docs/kernel-refresh/w0a/consumer-census.json
jq -e 'all(.consumers[]; ((keys | sort) == (["anchors","gapCheck","id","name","notes","protocols","recoveryClassJoin","role","tier"] | sort)))' docs/kernel-refresh/w0a/consumer-census.json
```

## Census coverage

| Census IDs | Boundary covered |
| --- | --- |
| `ts-command-frame-encoder` | Shared TypeScript command-frame envelope producer. |
| `ts-pager-bindings-producer`, `ts-motion-snapshot-producer`, `ts-motion-command-producer`, `ts-list-model-producer` | TypeScript pager, Motion, and list-model producers. |
| `rust-native-menu-producer` | Contract Native menu-intent producer. |
| `ts-graphics-publication-producer`, `ts-graphics-teardown-producer` | TypeScript graphics publication/teardown producers. |
| `rust-command-frame-dispatch` | Unified Rust command decoder/apply path. |
| `rust-pager-sidecar-decoder`, `rust-motion-snapshot-sidecar-decoder`, `rust-motion-command-sidecar-decoder`, `rust-list-model-sidecar-decoder`, `rust-menu-sidecar-decoder`, `rust-graphics-publication-sidecar-decoder`, `rust-graphics-teardown-sidecar-decoder` | Concrete Rust sidecar decode/validation consumers. |
| `windows-in-process-host` | Windows host: `OpParser`/`execute_ops` directly; it is not a `HostInterpositionFrame` consumer. |
| `tui-host-interposition` | TUI host: the production `HostInterpositionFrame` consumer found by the census. |
| `apple-ffi-command-host` | Apple `exact_buffer_*`/`processBuffer` FFI staging and commit path. |
| `web-protocol-dom-host` | Web DOM host, including its exact-successor sequence check. |
| `android-kotlin-decoder` | Current Kotlin/Android decoder, retained as the known-red pre-JNI row. |
| `contract-native-frame-producer`, `exact-native-ui-app-abi-transport` | Rust Native frame producer plus App-ABI FIFO/event/receipt crossing. |
| `ibex-dispatch-abi` | Ibex dispatch callback boundary, explicitly **UNVERIFIED** per RFC 0491 Phase 2. |
| `apple-exff-baked-frame-reader` | EXFF baked-frame decode and ordinary-engine replay. |
| `contract-conformance-replay`, `rust-protocol-fuzz-replay` | Replay, integration, and fuzz consumers; neither claims the not-yet-built LLP-0487/differential harness. |
| `generated-app-archive` | CLI-generated product archive/App-ABI producer. |
| `agent-observation-projection` | Current agent receipt/event consumer and future EXNODE projection consumer. |

All detailed paths and line anchors live on the JSON rows. The high-risk host facts reproduce with:

```sh
sed -n '347,365p' packages/exact-host-windows/src/scene.rs
sed -n '358,410p' packages/exact-host-tui/src/projection.rs
sed -n '198,235p' ios/ExactApp/ExactApp/Engine/ExactEngineTreeDomain.swift
sed -n '420p;732,750p' packages/exact-native-web/src/protocol-dom-host.ts
sed -n '501,545p;2580,2605p' android/ExactApp/app/src/main/java/com/exact/androidhost/surface/ExactAndroidRenderer.kt
sed -n '10595,10618p' ios/ExactApp/ExactApp/ExactRuntimeShell.swift
```

`web-protocol-dom-host` is the only row proven here to enforce an exact successor: `expectedSequence` is initialized at line 420, compared at line 735, and advanced at line 748 only after apply. All other `gapCheck` values are deliberately `no` or `n/a`, not inferred from incidental sequence parsing.

## Census ↔ generated recovery-family join

This table answers every generated `concreteConsumerStatus: "consumer-census-owned"` placeholder with concrete census rows. It names only the generated family key and census joins; recovery coupling, rollback, resync, identity event, and presentation acknowledgement remain owned by `tests/protocol/exwf/recovery-classes.json`.

| Generated family | Producing census row | Consuming census row |
| --- | --- | --- |
| `pager` | `ts-pager-bindings-producer` | `rust-pager-sidecar-decoder` |
| `motion-snapshot` | `ts-motion-snapshot-producer` | `rust-motion-snapshot-sidecar-decoder` |
| `motion-command` | `ts-motion-command-producer` | `rust-motion-command-sidecar-decoder` |
| `list-model` | `ts-list-model-producer` | `rust-list-model-sidecar-decoder` |
| `menu` | `rust-native-menu-producer` | `rust-menu-sidecar-decoder` |
| `graphics-publication` | `ts-graphics-publication-producer` | `rust-graphics-publication-sidecar-decoder` |
| `graphics-teardown` | `ts-graphics-teardown-producer` | `rust-graphics-teardown-sidecar-decoder` |
| `virtual-topology-certificate` | `ts-virtual-topology-certificate-producer` | `rust-virtual-topology-certificate-sidecar-decoder` |
| `gpu-canvas-attachment` | *(reservation-only: no producer exists)* | *(reservation-only: the kernel mirror is an inventory exemption)* |

The join is checked directly against the generated family list:

```sh
jq -n --slurpfile census docs/kernel-refresh/w0a/consumer-census.json --slurpfile recovery tests/protocol/exwf/recovery-classes.json '$recovery[0].rows | map(.family) as $families | {missing:[$families[] | select(([ $census[0].consumers[].recoveryClassJoin ] | index(.)) | not)], joins:[$families[] as $family | {family:$family, producers:[$census[0].consumers[] | select(.recoveryClassJoin==$family and (.role=="producer" or .role=="both")) | .id], consumers:[$census[0].consumers[] | select(.recoveryClassJoin==$family and (.role=="consumer" or .role=="both")) | .id]}]}'
```

The generated authority now enforces both halves of this. Its coverage obligation is **discovered** from the generated sidecar-family roster rather than enumerated, so a family added to the protocol inventory reddens the break-package generator until this file carries a producer row and a consumer row for it — that is exactly how the missing `virtual-topology-certificate` rows were found. A family whose kernel opcode mirror is an inventory exemption is **reservation-only** and is excused by that rule, not by hand. And the exact-once rule declared by `ledger-row-schema.draft.json` is enforced by the generator against `tests/protocol/exwf/breaking-window-ledger.json`: each census ID resolves to one census row and occurs in exactly one ledger row's `censusJoin.consumerCensusIds`; every ledger join resolves back to exactly one census ID. This file still supplies the stable join keys and never restates generated break-package rows.

## Discrepancies carried into the ledger

- Windows consumes command frames but does not use `HostInterpositionFrame`; TUI does.
- TUI mutates host mirrors before `frame.apply`, the known ordering red.
- Android remains a handwritten Kotlin decoder even though RFC 0491 closes the target shape as JNI typed operations.
- EXNODE entries in current rows are Phase-3 target responsibilities, not claims that an EXNODE export already exists.
- The Ibex row proves only callback ABI/storage anchors and remains explicitly unverified until execution-entry parity is gated.
