---
name: 20261005-interface-font-size-conversion
plan: 20261005-t3code-macos-parity
implementation: complete
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: 'feat(example)/t3-code'
branch: 'feat(example)/t3-code-interface-font-size'
pr_url: https://github.com/ccheever/exact2/pull/206
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

Parent specification: [spec](../../spec.md). Source behavior: reference
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
| resolved framework issue | [X3](../../issues/closed/20261005-x03-root-font-size.md) | none yet | Fixed on `main` and merged into the integration branch, or waived by the user | resolved: #102 closed by main #185; the branch merged main `cff90b364` |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged | pending |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |

Scheduling preference: after the last ticket that adds UI in the area being converted.

## Issue assessment at preparation

Checked sources and time: {{at prepare}}.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X3](../../issues/closed/20261005-x03-root-font-size.md) | App-settable root font size | `EXACT2-GAPS.md` X3 | blocking if reproduced | resolved through `20261005-interface-font-size` |

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

2026-10-07: built in the same PR as the foundation task (the user asked for both in one PR; not split per area).

- **Contract lengths.** Every literal length in the 145 Contract files follows the reference's split: font-size,
  line-height, letter-spacing, sizes and min/max, gap, padding, margin, positions, radius and flex-basis are N/16 `rem`
  (monospace UI text too: `font-mono text-xs` is rem); px stay: borders, shadows, blur, stroke and svg geometry,
  popover offsets, 1–2 px hairlines and dots, the 52 px top bars, table minimum widths, and text bound to its own px
  setting. A proposal script did the edit (not committed); `font-size-map.json` lists every file. At 16 every value is
  the same number of px.
- **px kept by the reference** (map `pxKept`): the Prompt font size (the composer text now follows the setting:
  `look.promptSize`, `text-(length:--font-size-prompt)`, `leading-relaxed`; before this task it was a fixed 14), the code
  size (code blocks, diffs, file previews), the terminal, the @pierre/trees rows (Diff tree, Files explorer: 12px in
  `pierre-tree-theme.ts`), ChangedFilesTree and process-tree indents (inline px), and real layout values.
- **Lengths computed from constants or TypeScript sizes.** The root provides `rem` (`provide rem = data.look.fontSize`;
  60-odd components `inject` it), and each such length multiplies it in, since Contract `calc()` takes `<percent> ± <px>`
  only: chat lane and composer stack, top-bar insets, toasts (max-w-90, `--toast-inset`, `--toast-gap`), tooltips and
  fields sized from text estimates, sidebar overlays and edge tips, the timeline minimap card (w-88) and steps, row
  gaps, list indents and block gaps, the live sweep, dialog caps (`calc(100vh - Nrem)`), pickers and editors (accent
  picker, theme editor, collections), menus (composer via `atRootFontSize`, usage, model picker rail, palette), select,
  field and slider widths, and the floating device player's resize handles (-1/h-2 edges, -2/size-4 corners).
- **Native module text** (`T3RootFont.swift`): the root size reaches the module with `devicePresentation`
  (`client.ts` `rootFontSize`), and the key recorder (13), SSH password field (text-sm), media preview failure label
  (text-xs), device tools tags (text-3xs, leading-3.5, px-1) and the ⌘Q pill (text-2xl, px-8 py-4) scale and redraw on a
  change. AppKit test `testPillFollowsTheInterfaceFontSize`.
- **Prompt sample in Settings › Appearance.** It is the reference's PromptFontPreview, a composer at the Prompt font
  size: the sample text and its chips now both follow the prompt size (`previewSize`), so at 12 the chips no longer
  shrink under px text.
- **Workspace card at 20, 1280 wide.** Checked against the reference code: `threadDetailsCardLayout.ts`
  `resolveThreadDetailsCardLayout` returns null when `x - DETAILS_CARD_CLEARANCE - lane.padding < lane.minChatWidth`
  (x = container − 280 − 12; minChatWidth is the measured `min-w-[40rem]`). With the sidebar open the container is 1024:
  at 16, 732 − 32 − 20 = 680 ≥ 640 (shown); at 20, 732 − 32 − 25 = 675 < 800 (hidden). The clone runs the same function
  with the same rem metrics, so hiding it matches T3 Code.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | as [the foundation's attempt 1](20261005-interface-font-size.md#attempts-and-evidence) | at 16 every probed box equal; 12 and 20 scale | five images | remaining areas (then pending) |
| 2 (2026-10-07) | as the foundation's attempt 2 | all checks green (listed there); drives at 1280×840 and 840×620 with `layout … native` on the composer and the key recorder | the evidence below | none in code |

Rows not run, with the exact reason:
- **Persistence across a real relaunch (launch, set 20, quit, relaunch).** An agent session cannot show it: under the
  agent the transport keeps no preferences (`T3Module.swift`: `T3Transport(persistent: !context.agent, …)`), so a second
  launch on the same named store came back at 16 (`round2/persist-record.txt`). A person-style launch needs real input, and
  `orca computer` could not read or click any app on this Mac today (every call, Finder included: "no accessibility
  window … macOS Accessibility may need Orca Computer Use toggled off and on again in System Settings"; permissions report
  granted), which only a person can repair. A seeded normal launch (`t3-code.json` with `fontSizeInterface: 20` under a lane
  HOME) did not read the saved preferences in the unpaired state either (a seeded `sidebarWidth` did not apply), so it proves
  nothing about this change. Covered instead by `client.test.ts` "a saved Interface font size is the root font size after the
  preferences load" (saved 20 → `look.fontSize` 20 → `devicePresentation.rootFontSize` 20) and the `rootFont` task, which runs
  on every key, the first included.
- **Attended size change with real input.** The same accessibility block. (The reference's control is a Select, not a
  slider: `SettingsPanels.tsx` FontFamilySettingsRow `<Select>` 12–20; the agent drove it at 12, 16 and 20.)
- **Oracle pixel pairs and the round-11 matrix.** No desktop oracle (user decision 2026-10-06). The workspace card at 20 was
  checked against the reference code instead (below); no screenshot of T3 Code (Nightly) at 20 was taken, because that would
  change its setting.
- Palette and Usage changes made after the drive (palette list caps 26.25/28/34rem, the usage grid gap) are compile- and
  test-checked only.

### Evidence (before/after)

Before is the untouched feature-branch tip `4f523ef5c` (`t3-code-evidence-base`), after is this branch at the final
drive (round 2); the same drive (`evidence/20261007-interface-font-size/round2/drive2.mjs.txt`), light, an isolated lane
server (port 16521, lane homes) with project Alpha and two threads. One image per scenario:

- Settings › Appearance at 16 (default), 1280×840, unchanged: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/01-settings-appearance-16-before-after.png
- Settings › Appearance at 12: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/02-settings-appearance-12-before-after.png
- Settings › Appearance at 20: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/03-settings-appearance-20-before-after.png
- A thread with the sidebar and composer at 20: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/04-thread-20-before-after.png
- A thread with the sidebar and composer at 12: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/05-thread-12-before-after.png
- Settings › Keybindings with the native key recorder at 20: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/06-keybindings-recorder-20-before-after.png
- A thread at 20 in the 840×620 window: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/07-thread-20-840-before-after.png
- Keybindings and the key recorder at 20 in the 840×620 window: https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/interface-font-size/08-keybindings-recorder-20-840-before-after.png

`layout` per size (`evidence/20261007-interface-font-size/round2/{before,after}-{1280,840}-record.txt`; boxes w×h):

| | before (12 / 16 / 20 alike) | after at 12 | after at 16 | after at 20 |
|---|---|---|---|---|
| `state` root font size | 16 | 12 | 16 | 20 |
| sidebar thread row | 239×78 | 243×58.5 | 239×78 | 235×97.5 |
| sidebar toggle / settings button | 28×28 / 32×32 | 21×21 / 24×24 | 28×28 / 32×32 | 35×35 / 40×40 |
| composer toolbar | 678×48 | 550×36 | 678×48 | 918×60 |
| composer text (`layout composer native`) | 14 px, line 22.75 px | 14 px, 22.75 px | 14 px, 22.75 px | 14 px, 22.75 px |
| native key recorder (`layout keybinding-recorder native`) | 156×24 | 116.5×18 | — | 195.5×30 (840×620: 195.5×30) |
| chat / settings header (52 px, px) | 52 | 52 | 52 | 52 |

At 16 every probed box equals the before build's. No `setRootFontSize … refused` line in any log.

## Next action

PR #206 review and merge.
