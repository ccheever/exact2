# Tab menu verification — 2026-10-06

Implementation: `8498fdc8a0a4b7b5b310f4be1a9f3a38fcfee0e2`. No implementation edits made. Exact `verify` and `prove` workflow read. The runner hashes all top-level application TypeScript/Contract/JSON, all native module/app Rust/test sources, native facade/data-key generator, and dependency manifests. Its result applies to the declared functional checks, not the full task acceptance verdict.

## Executed evidence

`recipe.json` records exact argv and self-contained native compilation. `checks/report.json` reports passed and source_unchanged=true. `checks/001.stderr.log`: 20 Bun tests, 0 failures. `checks/002.stdout.log`: 11 native XCTest cases, 0 failures. The native executable rebuilt against every current module Swift file and generated keys in this checkout. `environment.txt` records OS, Xcode and pinned Bun.

| Acceptance | Current observation | Remaining application proof |
| --- | --- | --- |
| Menu eligibility/order | Bun and actual NSMenu template assert Rename for device, Copy path for nonattachment files, disabled close flags; native menu command routing passes | Visible pointer menu at both viewports and Escape dismissal |
| Close actions | Active/inactive/neighbor/final/others/right/all and no-op policy tests pass; diffOpen synchronization passes | Integrated five-tab UI drive |
| Guard/cleanup seam | Single guard cancellation; all bulk cleanup; async cleanup retaining new selection/tab pass | None at policy boundary; live button-to-policy connection remains |
| Rename | Domain Enter/blur commit, Escape followed by blur cancellation, whitespace trim and empty fallback pass; native double click routes rename | Real selected focus, native Escape, Enter and blur through mounted Contract editor |
| Copy path | Native bridge request and success/error toasts asserted with stub | Real clipboard bytes and visible toast |
| Middle click/keyboard | AppKit native event constructed with button 2 closes exactly one, Shift-F10 targets focused tab; removed element ignores input | Normal launched app routing/visible menu dismissal |
| Host/device identity/persistence | Distinct identities, same device ID across hosts, per-device rename and activation, serialize/reload restoration pass | Three-device live fixture and real process relaunch |
| Pairs/fidelity | Not repeated per explicit user instruction to stop pixel-perfect test/fix loops | Functional screenshots are root capture responsibility |

## Practical fixture notes

The task names `examples/macos/t3-code/tools/` and `target/t3-ui-parity/{lane-backend.sh,electron-oracle.mjs}`; none exists in this checkout. Device mock HTTP/WebSocket hub exists in `apple/tests/r11-device/hub.swift`. Saved preference `rightPanels[draftKey]` can seed matching connected thread surfaces, restored once the client is ready and preferences loaded. Device entries preserve device:{hostId,deviceId,name,platform}, title, and escaped host/device identity. File entries require id `file:${path}`.

Visible tab test IDs: `panel-tab-${id}` (contextmenu row), `tab-${id}` (activation button), `close-tab-${id}`, `tab-name-${id}` (inline editor). Native input hooks dispatch existing Contract actions and do not bypass close guards.

No whole-task pass/closure is asserted here. The root agent owns normal app launch and capture, shared gates, common dependency reconciliation and task records. Outstanding evidence is not an observed implementation failure.
