# Repair verification, 2026-10-06

The user authorized repairing the previous verification failures. Base commit: `7444d5999`; branch: `daehyeon/t3code-parallel-features`. This is app-only work on the recorded framework base, not the pending migration to main.

## Repairs and diagnosis

- Tool output now accepts keyboard focus, shows a focus indicator, and uses the observed scroll offset for Arrow Up/Down and Space. The previous plain scroll was skipped by the actual host Tab loop.
- Timeline timestamps now have a keyboard focus trigger. Focus reveals the timestamp and keeps its tooltip visible while scrolling.
- Native menu production code is unchanged. A bounded reproduction isolates the old delay to a nested agent `clock settle` during native menu tracking. Real selection and Escape complete promptly without this interruption. Full-app capture uses platform timing and lets native tracking return before inspection.

The first repair recipe records a failed Contract compile caused by unsupported `tabIndex`; those attributes were removed. The native host has an internal property by that name, but the Contract compiler does not expose it. The failed report remains intact. `checks-supported/report.json` compiles supported attributes and passes all seven commands with unchanged source.

## Regression checks

Pinned Bun 1.4.2: 1,197 app tests, strict TypeScript, full Contract compilation, root Cargo build/clippy/format and boot checks pass through the Exact verify runner. Root Cargo tests (default members, all lib/bin/test targets, no fail-fast) also exit 0. App Rust tests pass 10/10. The complete macOS bundle builds. Staged source caps pass. Logs are adjacent to this record.

Actual ExactKit key-loop regression passes output and timestamp reachability and focus/key delivery. It also reports an existing zero-size tooltip dismissal hook as an extra Tab stop; it does not claim that stop was removed or that standalone native traversal proves the entire mounted app. Live scroll boundaries and mounted acceptance are recorded separately.

## Acceptance status

These regression passes alone do not close any task. Mounted tab, timeline and workspace-retry acceptance and independent review are being completed in separate task evidence. The existing framework issue records are unchanged; app workarounds do not prove upstream fixes.
