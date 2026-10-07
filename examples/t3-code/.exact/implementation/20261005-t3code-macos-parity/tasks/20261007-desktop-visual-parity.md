---
name: 20261007-desktop-visual-parity
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-desktop-visual-parity
pr_url: https://github.com/ccheever/exact2/pull/250
verified_commit: 54c43198e
---

# Match desktop typography previews, selectors and Markdown tables

## Outcome

Appearance previews show the same chip icons and highlighted diff as the Electron
reference. Keybinding and scheduled-task selector values and options align to
the left. Markdown table headers and body cells remain readable at their full
line height. Keep new appearance and layout findings from this audit together in
this task.

## Observed baseline

The 2026-10-07 audit opened the real Electron app and built and opened the Exact
macOS app. These findings come from their captured windows, then source inspection.

- Exact source: `fbce02624d2e33449ee2cde34497083d6fd47457` in the
  `t3code-desktop-audit` worktree; launch record
  `target/desktop-audit/native/launch-record.json`.
- Electron source: `1e2ecbd9758830669684b494d4398f626b0576e0`;
  `target/desktop-audit/reference-launch.json` records its source-equivalent build.
- The pairs show the T3 Code light palette and SF Pro interface font at 16 px.
  The code fonts were not normalized: Exact uses SF Mono at 13 px, and the
  reference uses Menlo at 13 px, confirmed by the saved reference tree. Icons,
  token colors and alignment within controls do not depend on matching these
  font metrics. Exact's logical viewport is 1280 by 840; the reference settings
  captures are 1226 by 840. The settings findings concern missing icon/token content and
  alignment within controls, not pixel spacing or line wrapping across viewports.
- The rich-thread pair for V03 uses matching 1280 by 840 logical viewports:
  Exact's PNG is 2560 by 1680 at 2x, and the reference PNG is 1280 by 840 at 1x.
  Both render the same two-column Markdown table, with a header and two body rows.
- Evidence paths below are relative to the worktree root and are local audit
  artifacts under `target/desktop-audit/`.

## Confirmed findings and reproduction

| ID | Steps | Electron result | Exact result | Evidence |
| --- | --- | --- | --- | --- |
| V01 | Open Settings > Appearance, leave Typography > Advanced off, scroll to Interface font and Monospace font. | Prompt chips use a cube for Frontend Design and file-type icons for the two files. The `src/formatUser.ts` sample shows TypeScript token colors, stronger backgrounds around changed text and a rounded-square file-change marker with a center dot. | Each prompt chip starts with a solid colored square. Every code token is the same text color; added/deleted rows have only whole-line backgrounds. The file-change marker is a hollow circle. | Exact: `native/native-settings-appearance.png`. Reference: `evidence/ref-settings-appearance-interface-font-family.png`, whose open font menu leaves the skill chip and left side of the highlighted diff visible; `evidence/ref-settings-appearance-advanced-restored.png` also records the typography section. |
| V02 | Open Settings > Keybindings, press +, then open Command. Also open an existing binding's When editor and inspect its condition selector. | Command placeholder, command options and condition values start at the left inset of their controls. | The Command placeholder, every command option and condition values are centered. | Exact: `native/native-settings-keybindings-add.png`, `native/native-settings-keybindings-command.png`, `native/native-settings-keybindings-condition.png`. Reference: `evidence/ref-settings-keybindings-new.png`, `evidence/ref-settings-keybindings-command-options.png`, `evidence/ref-settings-keybindings-condition-options.png`. |
| V02, additional controls | Open Settings > Scheduled Tasks > New task with the shared fixture project selected. | Runs on, Project, Workspace and Base branch values start at the left inset, next to any leading icon. | All four values are centered within their controls; Runs on and Base branch icons stay at the left edge. | Exact: `native/native-settings-scheduled-new-dialog.png`. Reference: `evidence/ref-settings-scheduled-new.png`. |
| V03 | Open the shared rich-thread fixture containing the Markdown table below; leave table cells collapsed. | The Surface / Result header and both body rows are fully readable, with space above and below the text. | The header text is clipped to a thin horizontal sliver, and the body rows are vertically compressed. | Exact: `native/native-thread-rich.png`, `native/native-thread-rich.json`. Reference: `evidence/ref-thread-rich.png`, `evidence/ref-thread-rich.txt`. |

For V02 the saved search and edited binding differ between the captures. The
Command dropdown lists the same options regardless of the list search. The
alignment difference is visible within those matching controls; it is not a
claim about search results or expression contents.

V03 uses this content; reproduce it in an assistant Markdown message:

```markdown
| Surface | Result |
| --- | --- |
| Transcript | Ready |
| Tool output | Ready |
```

## Source guidance

- `examples/t3-code/settings-rows.contract:402` defines `FontPromptPreview`.
  `FontChip` draws the generic colored box at line 426. `FontCodePreview` at
  line 428 draws its own circle and calls `CodeLine`, which renders each entire
  line as one text node at line 454. Reuse the existing composer chip and diff
  presentation where practical so the preview reflects the displayed content.
- Reference `apps/web/src/components/settings/SettingsFontPreviews.tsx` uses
  `ComposerPromptEditor` for its prompt at line 31 and the real highlighted diff
  renderer through `loadDiffPreviewHtml` at line 78 and `CodeFontPreview` at
  line 125. Its sample patch supplies the expected content.
- `examples/t3-code/settings-keybindings.contract:146` uses `SettingsSelect`
  for Command; line 302 uses it for a condition. Inspect `SettingsSelect` and
  `SkMenuItem` in `settings-kit.contract:93` and `:129`. Their text has no explicit
  left alignment, while it sits inside buttons. Fix the alignment at the shared
  component if its other callers expect the same result.
- Reference `apps/web/src/components/settings/KeybindingsSettings.tsx:1126`
  defines `NewKeybindingCommandSelect`; its condition selector is at line 323.
- `examples/t3-code/settings-scheduled.contract:306` defines the Runs on
  trigger; Project and Workspace follow it through `SettingsSelect`. The Base
  branch trigger is at line 326. Their stretched text nodes have the same
  alignment problem; include the custom triggers when correcting the shared
  selector.
- `examples/t3-code/markdown.contract:843` and `:846` give plain `TableCell`
  text a `0.75rem` font size and `0.1015625rem` line height. At the captured
  16 px root, that is 12 px text in a 1.625 px line box. The hidden sizing text
  at line 855 and the older table-row path at line 350 use the same line height.
  This is the likely cause of V03, inferred from source after observing the
  clipping; verify it while implementing. The adjacent `CellRuns` path instead
  reserves a `1.21875rem` minimum line height.
- Reference `apps/web/src/index.css:1983` sets table text to `0.75rem` and
  defines cell padding at line 1992; `ChatMarkdown.tsx:3515` supplies the
  surrounding `leading-relaxed` typography. Check plain and inline-code cells
  together when restoring the intended row height.

## Scope and existing tracking

This task contains only the newly observed cosmetic differences above. It does
not duplicate the residual rows of
[the previous minor UI task](closed/20261007-fix-minor-ui-issues.md), including
boolean model-trait groups, joined trait labels, wrapped menu height estimates,
the live compacting row size or focus-within receding. That task fixed left
alignment in the right-panel chooser; V02 records different affected controls.

[Settings scope and theme editor](closed/20261005-settings-scoped-controls-and-theme-editor.md)
owns scope writes, mixed states and editor lifetime. Its excluded Inspect control
remains under X30. [Shiki residuals](closed/20261005-shiki-residuals.md) owns
highlighting grammar, size and style behavior in chat, diff, Files and search;
V01 concerns the separate plain-text preview implementation.

[Timeline and Markdown](closed/20261005-upstream-timeline-and-markdown.md) owns
the earlier content and interaction parity work; it does not record plain table
text clipping. V03 concerns the observed row rendering. X32's sticky table-header
positioning is a separate known limitation.

Exclude native font smoothing, known shadow/blur differences under X11,
Dev/Nightly artwork, viewport-driven spacing differences, and the bottom of a
scrolling menu captured between rows. Functional model-picker, font-selection
and color-picker differences belong in their own tracking.

## Acceptance

1. At matching viewport sizes, capture both apps' Typography section with the
   menus closed. Verify the three prompt-chip icons, diff marker, TypeScript
   token colors and intraline change emphasis. Repeat in light and dark themes
   and with Advanced enabled. Changing the preview font size preserves these
   details.
2. Capture the empty Command trigger, open Command menu, a When condition
   selector and the four New task selectors. Their labels start at the same
   left inset as the reference. Verify selected values and long labels, plus
   unaffected shared selector callers.
3. Render the V03 table in light and dark themes, collapsed and expanded. Every
   header and body value is fully readable with reference-equivalent row height.
   Repeat after changing the interface font size, and include cells containing
   inline code plus a long wrapping value to exercise both cell-rendering paths.
4. Build and drive the changed macOS app; record before/after screenshots and
   affected checks. Discovery screenshots establish the defects; they do not
   count as verification of a future fix.

## Progress

- 2026-10-07: Independently inspected the paired live screenshots and the source
  paths above. V01–V03 have high confidence. No product source was changed.
- 2026-10-08: Implemented V01–V03 (`54c43198e`, draft PR #250).
  - V01: `settings-font-previews.contract` and `settings-font-previews.ts` draw the
    composer's chips and the diff panel's presentation of the reference patch. That is
    Pierre's default header and unified rows with word-alt emphasis: jsdiff
    `diffWordsWithSpace` and Pierre `pushOrJoinSpan`, ported; their licences are in the file.
  - V02: `text-align="left"` on the shared `SettingsSelect` and `SkMenuItem` text and on
    the Runs on and Base branch triggers.
  - V03: the cause was confirmed. The rem conversion (bf35a2d49, 6d87fd91a) had turned 8
    unitless line heights into pixel-sized rem. They are restored, and a test guards the class.
  - One live session ran (after: the one retry; before: the base build), plus a
    reference pass in the release's own web client. Independent review found no blocking
    findings.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| 1 | `54c43198e` on `d82fb6a47` | `bun test examples/t3-code` 2518 pass / 1 skip / 0 fail (base 2511 + 7 new); strict tsc clean; `contract build` 2629 slots; `cargo test -p t3-code-macos --lib` 11 pass; caps pass; five checks pass (cargo test 3383 passed, 0 failed, 33 ignored; clippy, fmt, boot exit 0). Runner recipe `passed`, `source_unchanged: true`, digest `0c82c168eae0…`, committed tree matches. Port parity against the reference's `diff@9.0.0` and `@pierre/diffs`: 5,000 + 5,000 random pairs identical (reviewer: 20,000). | `t3-code-evidence/desktop-visual-parity/` at `09039d524be3`: pairs 01–15, `ref-*.png`, `live-session.md`, `ops-after.txt`, `ops-before.txt` | see below |
| live, after | bundle of `54c43198e`'s tree (before review edits) | Attempt 1: the standalone dev binary has no bundled runtime (`runtime-missing`), so no flow ran. Attempt 2 (the one retry): the bundle binary with `T3_LOCAL_HOME`, port 16511. Typography light/dark/Advanced/18+16 px, keybinding Command trigger, menu, long value and When condition, New task and its Workspace menu, the fixture tables expanded/collapsed/dark/18 px. Two refused ops were retried by view id. | `ops-after.txt` | — |
| live, before | base `da4f4512f` (evidence worktree, rebuilt under its lock) | The same flow on the same lane home | `ops-before.txt` | — |

The fixture thread came from a lane-only Grok ACP stand-in, modelled on the reference's
`grok-text-mock-agent.mjs`; it is uncommitted, under `target/lane/`, and made no real provider call.
Post-drive edits are non-visual at the captured size: licence comments, chip test ids and labels,
and stripe height `1.25rem` → `20px`.

Not verified, with blockers:
- The other shared selector callers (Storage, Source Control, Projects, host and keybinding row
  menus) were not captured live. This needs one more session (one-session rule, 2026-10-06);
  reviewed in code: all 9 `SettingsSelect` and 8 `SkMenuItem` sites align left, as the reference does.
- There are no reference captures in dark, with Advanced on, or of collapsed tables: same session budget.
- Oracle and trace rows were not run: user decision 2026-10-06.

Found while capturing. This is existing behaviour, outside this task's acceptance, and needs a
follow-up task:
- The reference draws inline code that names a workspace path (`apps/web/src/index.css`) as a file
  chip; Exact keeps a code span.
- Expanded tables: the reference's long value wraps across the full column (Chrome ignores
  `max-width` on `td`), while Exact wraps at 22.5rem. In one collapsed capture Exact cut at a word
  with no ellipsis.
- Intraline emphasis is square-cornered. Pierre has a 3 px radius, but Contract inline spans paint
  background only.
- In t3-code agent drives, plain `screenshot <png>` returns a uniform white image;
  `screenshot <png> window` works.

## Next action

Review of draft PR #250. #257 (editable prompt preview) replaces the static prompt sample and is
expected to conflict in `settings-rows.contract`. One more live session would close the
shared-caller and reference-dark rows.
