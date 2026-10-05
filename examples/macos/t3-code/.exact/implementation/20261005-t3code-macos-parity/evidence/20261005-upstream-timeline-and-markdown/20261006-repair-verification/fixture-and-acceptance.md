# Additional timeline acceptance fixture

This is fixture preparation and focused logic evidence, not a task passing report.
No production source was edited by this worker.

The existing isolated reference runtime `1e2ecbd975` at port 16843 is owned by the coordinator. Before its restart, the stopped isolated database was backed up to `target/t3-ref/verify-runtime/pre-acceptance-fixture.sqlite`. Seven synthetic persisted turn items were added; original items remain. The complete setup is recorded in `target/t3-ref/verify-runtime/acceptance-fixture.json`. The runtime and server source were not modified.

- Thread `verify-timeline`: existing command, failing command, read and skill; then `fixture-website`, `fixture-themed`, `fixture-native`, `fixture-broken`, `fixture-failed-icon`, `fixture-empty`, and `fixture-dynamic`.
- Website and themed icons use locally encoded blue/light and yellow/dark PNGs. Broken icon uses invalid PNG data; failed icon has status `failed` with valid PNG. Native icon requests Finder through actual `assets.createUrl`.
- `fixture-dynamic` has 31 lines of distinct output for real output-scroll interaction.
- Final Markdown contains known `$verify`, price `$5`, code `$verify`, linked `$verify`, and the three file-label classifications.
- Project `.agents/skills/verify/SKILL.md` is present for actual Codex workspace discovery. Its presence alone does not prove provider discovery or chip rendering.

Focused pinned Bun command:

```sh
cd examples/macos/t3-code
/Users/daehyeonmun/.bun-1.4.2/bin/bun test timeline-item-detail.test.ts timeline-tool-icons.test.ts r4-timeline-chips.test.ts r3-composer-controls-resume.test.ts timeline.test.ts client.test.ts
```

Result: 158 tests, 646 assertions, zero failures. `timeline-functional.log` records deferred reversed-order A/B detail replies, keyed source identity, missing/fetched-empty/RPC-error states, revision refresh, manual-only compaction, favicon/theme selection and skill exclusions. These deterministic protocol tests do not substitute for integrated view acceptance.

## Coordinator GUI actions still required

1. Connect the rebuilt app to the isolated runtime; allow real cwd-scoped provider refresh. Confirm the known skill becomes a chip while code, prices and linked tokens stay unchanged. Copy the message and compare source text.
2. Expand command, failed command, read, skill and dynamic rows; verify their distinct output, command body, nonzero exit, and empty-row disabled disclosure. Open two rows in quick succession and confirm independent content.
3. Verify real native overlays for website/themed/Finder, invalid PNG fallback, and failed valid image plus trailing x. Switch theme once to observe alternate PNG; no pixel-perfect matrix.
4. Tab from disclosure into repaired output scroll, use Space/Return on disclosure, and use ArrowDown/Space within output. Verify focus stays usable.
5. Hover a timestamp in the long transcript. Deliver a real wheel event that changes transcript bounds, keep pointer still, and confirm dismissal stays latched. At boundary, wheel without movement must not dismiss. Repeat with a focused trigger; focus must preserve the tooltip.

Forced delayed/error detail replies remain deterministic unit evidence unless the coordinator adds an isolated transport fault. Existing stubbed `ExactElement` hook harness proves only native hook logic and must not be described as full app integration.

## Native component rerun

The pre-existing `20261006-functional-verification/run-hooks.py` was recompiled and rerun successfully: five actual NSImage lifecycle, four NSClipView scroll/latch, and two focused-descendant checks. Log: `native-component.log`. Its `ExactElement` is a notification stub; this establishes hook-local logic only. The actual Contract TimelineTip wrapper currently has no focus handler, so integrated focused-trigger reachability requires explicit verification and may require repair.

## Actual workspace discovery

The restarted real server returned no Codex workspace catalog because the isolated Codex version is unsupported and unauthenticated. Added the same synthetic skill under the isolated project's `.claude/skills/verify/SKILL.md`, then called `server.refreshProviders` for `claudeAgent` and the fixture cwd. The actual response contains enabled project skill `verify`; minimal metadata evidence is `known-skill-discovery.json`. No provider turn was invoked. The actual GUI must select Claude before this positive skill catalog can be asserted in Markdown.
