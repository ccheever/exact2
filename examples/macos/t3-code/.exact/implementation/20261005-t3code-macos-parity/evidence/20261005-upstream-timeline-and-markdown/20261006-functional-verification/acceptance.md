# Timeline functional verification, 2026-10-06

Implementation: `8498fdc8a` on `daehyeon/t3code-parallel-features`. Application and dependency bytes are fingerprinted by the skill runner reports. Environment: macOS 26.6.2 (25G83), Xcode 27.0 (27A266a), Bun 1.4.2, private checkout Hermes provisioning. No production source was edited by this verification worker. No screenshot comparison or pixel-perfect repair loop was attempted.

## Observed checks

- `checks/report.json`: 158 focused Bun tests / 646 assertions pass; actual AppKit image/tooltip harness has 11 passing assertions. The harness uses actual NSView, NSScrollView and NSWindow first responders; **ExactElement notification is a stub**, so this does not certify complete Contract/native integration.
- `rust-checks/report.json`: all 10 app Rust tests pass, including the 16-case file-link label table, prose label preservation, known skill token context exclusion, and copy-source preservation.
- `keyboard-checks/report.json`: separate actual ExactKit AppKit key-loop test. **FAILED:** actual Tab from disclosure node 1 reaches the following button node 3, skipping output node 2. `acceptsFirstResponder`, `canBecomeKeyView` and `Presenter.tabbable` are all false. The runner confirms unchanged declared source. This builds the unchanged host sources and links the current app archive; the app output node semantics are reproduced as a scroll node with no event handlers. It does not pretend to be the full application GUI.

## Acceptance matrix

| Criterion | Evidence established here | Full app acceptance still needed |
| --- | --- | --- |
| A1 | Sending past synthetic context banner dispatches only typed message; explicit compact retained (Bun) | Banner plus actual send trace in connected app |
| A2/A3/CN1 | Lazy keyed reads, deduplication, concurrent late replies, revision refresh, error/empty states, command body and nonzero exit behavior (Bun) | Expand actual command/read/skill/dynamic rows using new runtime; pointer and keyboard; protocol trace |
| A4 | 16 label classifications, prose + chip, source preserved (Rust) | Render three link cases and copy in actual app |
| Tool icons | Source selection and favicon tests (Bun); success/cache/failure/unmount notifications (AppKit) | Render all kinds and broken URL in both themes; provider MCP icon is explicitly optional in ticket |
| G12b | Known names only, prices excluded, code/link context excluded, copy token preserved (Bun/Rust) | Render/copy in actual app with workspace skills |
| A17 | Real clip bounds movement dismisses, unchanged bounds does not, physical-motion callback releases latch, focused descendant prevents dismissal (AppKit) | Integrated hover/scroll under stationary pointer; actual wheel attended check |
| Disclosure keyboard | Actual ExactKit host key loop is tested separately | Full app Space/Return toggling and focus retention |

## GUI recipe for coordinator

Use isolated backend/runtime at reference `1e2ecbd975` or later, and isolated app data. Run both 1280×840 and 840×620 in light/dark. Open a fixture with completed successful/failed command, dynamic tool, read, skill, and empty successful row. The disclosure selector is `work-detail-${entry.id}`; get the actual ID from the tree because compound source-thread IDs are JSON strings. Capture initial tree, disclosure tap, loading tree, settled output, and recorded `orchestration.getTurnItem` payload. Open two rows before replies and assert output ownership. The output scroll has accessibility label `Tool output`, but no testId.

For keyboard, focus the disclosure through actual Tab sequence, Space opens and Return closes. On reopen, press Tab: the output must be a reachable stop. Do not count direct focus calls as sequential reachability. Current `timeline-work.contract:306` has no focus/blur/key/press handler; `NodeViewMac.swift:184` and `PresenterMac.swift:1142` exclude such plain scroll nodes from the key loop. Preserve the actual failure instead of adding unsupported tabindex.

For tooltips, hover an expanded row timestamp, scroll its real enclosing transcript, leave pointer still and verify tooltip stays closed; give the trigger actual keyboard focus and repeat. A wheel at the boundary without a bounds-origin change must leave the tooltip unchanged. The native focus harness tests the dismissal guard, not whether the current Contract timestamp offers a useful keyboard focus target.

## Scope of report verdicts

Passing unit/hook and Rust runner reports apply only to their declared checks. They are not full task pass reports. Task closure requires integrated acceptance and independent review; this worker has not changed task status or moved it into closed.
