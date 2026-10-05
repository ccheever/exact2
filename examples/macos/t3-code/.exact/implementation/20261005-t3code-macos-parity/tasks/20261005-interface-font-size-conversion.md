---
name: 20261005-interface-font-size-conversion
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# The whole interface scales with the interface font size

## Outcome

Settings › Appearance › Interface font size (12–20 px, default 16) scales every size that
the reference sizes in `rem`, across every surface, while the sizes the reference keeps in
`px` (prompt, code, diff, terminal canvas, borders) stay fixed. At 16 px the app is
pixel-identical to the matrix before the conversion.

**Planned split.** This row tracks the scope now. At its `prepare`, split it per area from
`font-size-map.json` (made by `20261005-interface-font-size`), for example shell and sidebar,
chat and composer, panels/diff/files, pull-request pages, settings and providers, terminal
and device. Each area is a separate PR from the updated integration branch (no stacks).
Keep this name for the first area and use `20261005-interface-font-size-conversion-<area>`
for the rest.

## Scope and exclusions

Included: converting each area's Contract sizes to `rem` exactly where the reference uses
`rem` (per the map); the lane limits that depend on `rem` (TN5, with
`20261005-floating-device-player`); the sidebar minimum width; JS layout constants that
mirror `rem` sizes; and the fixed metrics in the app's Swift modules (native composer and its
chips, native menus, timeline rows and other native views), scaled by the same root size.

Excluded: the root-size mechanism, the feasibility check and the shared style classes
(`20261005-interface-font-size`); sizes the reference keeps in `px`.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior: reference
`apps/web/src/appearanceFonts.ts:95-123`, `apps/web/src/index.css` (mixed `rem`/`px`), and
the oracle captures at 12/16/20 px recorded in `font-size-map.json`.
Library revision: `20261005-platforms-v3`. Selected topics: design (preserve the app's type
scale; test long labels and truncation at each size), layout-and-interaction (content-box
defaults; `min-width=0` on flexible rows; check every pane at 840×620), testing-and-debugging.
Root font size is unknown in the library (knowledge gap 10).
Line numbers are from the mc-orch tree on 2026-10-05; find code by symbol.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-interface-font-size](20261005-interface-font-size.md) | pending | Merged (root size works; the map exists) | pending |
| resolved framework issue | [X3](../issues/20261005-x03-root-font-size.md) | none yet | Fixed on `main` and merged into the integration branch, or waived by the user | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |

Scheduling preference: after the last ticket that adds UI in the area being converted.

## Issue assessment at preparation

Checked sources and time: {{at prepare}}.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X3](../issues/20261005-x03-root-font-size.md) | App-settable root font size | `EXACT2-GAPS.md` X3 | blocking if reproduced | resolved through `20261005-interface-font-size` |

## Implementation notes

Convert by the map, not by search-and-replace of every `font-size=`: only values the
reference sizes in `rem` change. One commit per surface.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| 16 px unchanged | Lane fixture backend; round-11 thread fixture | Matrix at 1280×840 and 840×620, light and dark | Every cell within 0.02 of the pre-conversion run | macOS | matrix |
| Scales at 12 and 20 px | Same fixture; setting 12 and 20 | Shots of the area beside the oracle at the same setting | Pairs "match" or carry a declared deviation with an issue link; no clipped or overlapping text | macOS both sizes | shot pairs |
| Native views scale | Setting 12 and 20 | `layout <id> native 2` on the native composer, a chip, a native menu row and a timeline row | Sizes scale by the root factor where the reference uses `rem`; match the oracle pair | macOS | layout JSON, shot pairs |
| px sizes stay | Setting 20 | `layout` on prompt, code, diff, terminal nodes | Same sizes as at 16 | macOS | layout JSON |
| Long labels | Setting 20, 840×620 | Drive the area's longest labels | Truncation as the oracle; controls reachable | macOS | shots |
| Real slider (attended session) | Lane build with `T3_LOCAL_HOME` / `T3_LOCAL_PORT` | Drag the slider 12→20 | Live resize with no layout jump that the oracle does not show | macOS | notes + recording |
| Repository gates | `git add -A` | caps; the five checks; clone checks | Pass | macOS | logs |

Task-owned source paths: the area's `*.contract` files; `font-size-map.json` status column.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

After `20261005-interface-font-size` merges: `prepare` splits this ticket per area (plan
revision), then `implement` the first area.
