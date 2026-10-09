---
name: 20261009-settings-appearance-and-skill-chip
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-settings-appearance-and-skill-chip
pr_url: https://github.com/ccheever/exact2/pull/364
verified_commit: null
---

# Appearance: theme editor opens Advanced for a copy or an edit, Open VSX publisher names, and the skill chip popover

## Outcome

- Duplicate of a non-managed theme, and Edit of a saved theme, open the editor with Advanced on and every role filled.
- Open VSX theme results show the extension's namespace as the publisher.
- A skill chip opens its details popover on a press: in the Appearance prompt preview, and in the composer, where the
  closed skill-chip task found the same gap.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| S1-5 | Duplicate T3 Chat opens "Create theme" with name "T3 Chat copy" and Advanced on (all 20 roles: Foundation, Brand & content, Context, Status, with the source colours). Edit of a saved custom theme opens "Edit theme" with Advanced on. `ThemeEditorPanel.tsx:395-399` opens Advanced for any source theme that is not managed, so guided regeneration cannot discard hand-tuned colours. | Duplicate opens with Advanced off and only Background (#fdf7fd) and Accent (#db2777). Edit of a theme saved with Advanced on also opens in simple mode ("Two colors, rest derived"). `settings-appearance-editor.ts:128` always sets `advanced: false`. | Appearance › Duplicate T3 Chat; save; press Edit on "T3 Chat copy". | `target/t3-audit/evidence/settings-1/S1-5-ref.png`, `S1-5-clone.png`, `S1-5-edit-ref.png`, `S1-5-edit-clone.png` |
| S1-6 | Dracula results: "dracula-theme · 431.5K downloads", "Dracula-2 · 104K downloads", "bceskavich · 18K downloads", "MateuszDrewniak · 16.9K downloads" (`openVsxThemes.ts:195`: publisher = namespace). | "open-vsx · 431.5K downloads", "TimDeen · 104K downloads", "open-vsx · 18K downloads", "Verseth · 16.9K downloads" (`settings-appearance-import.ts:160` uses `publishedBy.loginName`). | Appearance › Add theme › Popular › Dracula. | `target/t3-audit/evidence/settings-1/S1-6-ref.png`, `S1-6-clone.png`, `S1-6-ref.txt`, `S1-6-clone.txt` |
| S1-12 | Clicking the "Frontend Design" chip in the Typography sample opens a popover: "Frontend Design" / "No description is available for this skill.". | A press on the chip shows no popover; the tree has no chip control. The closed [skill-chip-provider-name](closed/20261008-skill-chip-provider-name.md) (lines 82-84) found the same for the composer's chips: the reference's chip opens a popover (label, description or "No description is available for this skill.", "View instructions" for a skill with a path) and is named "Skill <label>". | Appearance › Typography › click the "Frontend Design" chip. In the composer, insert a skill chip and press it. | `target/t3-audit/evidence/settings-1/S1-12-ref.png`, `S1-12-clone.png` |

## Scope and exclusions

Included: the three findings above.

Excluded:
- The theme editor's Inspect (S1-15): waits for main fix of X68 ([#321](https://github.com/ccheever/exact2/issues/321)), plan decision U18.
- The usage highlight and "N uses" (S1-7): [blocked-theme-usage-highlight](20261009-blocked-theme-usage-highlight.md).
- The colour picker's placement (S1-10): X17, waits for main fix of #112.
- The installed font picker (S1-11): [installed-font-picker](20261007-installed-font-picker.md).
- Atomic chips in plain Contract fields: #276 is closed as not planned; the chips stay the native editor's. This task adds
  a press and a popover to the existing native chips; it does not move them.
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`):
- S1-5: `components/settings/ThemeEditorPanel.tsx:395-399`.
- S1-6: `openVsxThemes.ts:195`.
- S1-12: `components/ComposerPromptEditorTiptap.tsx` (the skill chip and its popover).

Clone (`examples/t3-code`):
- S1-5: `settings-appearance-editor.ts:128`.
- S1-6: `settings-appearance-import.ts:160`.
- S1-12: the native editor draws the chips (`modules/apple/T3ComposerEditor.swift`, `T3ComposerChipTips.swift`; the prompt
  preview's editor in `modules/apple/T3Module.swift`; tests in `macos/tests/composer/promptpreview.swift`). Report a chip
  press from the native editor to the app, and draw the popover in Contract anchored at the chip's frame.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| S1-5 | Duplicate T3 Chat opens Advanced with all 20 roles and the source colours. Edit of a saved Advanced theme opens Advanced. A managed theme still opens simple. Unit test. | `s1-5-duplicate.png`, `s1-5-edit.png` | agent |
| S1-6 | Dracula results show the namespaces as in the reference. Unit test on the mapping (needs Open VSX reachable for the live row). | `s1-6-open-vsx.png` | agent |
| S1-12 | A press on the preview's chip opens the popover with the label and "No description is available for this skill."; the composer's chip does the same and shows "View instructions" for a skill with a path; the chip is named "Skill <label>". XCTest for the press report. | `s1-12-preview-chip.png`, `s1-12-composer-chip.png` | agent, then needs_real_input (a real click on the native chip) |

## What was built

**S1-5 (Advanced on open).** `settings-appearance-editor.ts` `opensAdvanced`: the editor opens with Advanced on for any
source theme the guided editor did not make (ThemeEditorPanel.tsx:395-399: `sourceTheme !== null && sourceTheme.managed
!== true`). The source is the edited theme, else the seed (a duplicate's theme, or the active theme for Create theme).
T3 Code's stock look is no theme definition in the reference (`getThemeDefinition`), so a copy of it opens simple.
- `CustomTheme.managed` (`settings-themes.ts`): a save from simple mode carries it, a save from Advanced does not; adding a
  palette to a guided theme keeps it only from simple mode (handleSubmit). It is kept by the saved preferences
  (`decodeCustomThemes`), the theme file (`serializeTheme`, `parseThemeFile`) and so imports. An edit also keeps the
  theme's collection, as the reference's does.
- The clone's built-in palette table has 19 roles, so Advanced showed T3 Code's values for the rest (Raised surface
  `#fcfcfc`, Subtle surface `#fafafa`, …). `settings-theme-library.ts` holds the five built-ins' full definitions (57 roles,
  light and the dark variant) from `packages/shared/src/themePalettes.ts`, converted with the clone's own `toHex`; the
  editor seeds a built-in source from them. All 20 rows of a T3 Chat copy now equal the reference's hex fields. The app's
  painting is unchanged.

**S1-6 (Open VSX publisher).** `settings-appearance-import.ts`: the publisher is the extension's namespace
(`openVsxThemes.ts:195`), not `publishedBy.loginName`.

**S1-12 (the skill chip's details).** The chips are drawn by the native editor (#276: they stay the native editor's).
- `modules/apple/T3ComposerChipPress.swift` (new): each editor's styler sees the presses the application sends (a local
  `leftMouseDown` monitor: a hand's click and the agent's tap both go through `NSApplication.sendEvent`). A press that the
  window gives to the text view and that lands on a skill chip opens its details (a second press on it closes them, as
  PopoverTrigger toggles); a press elsewhere in the text, Escape, an edit, the chip going away and the editor leaving
  close them. The press is reported on `t3.chip` with the chip's pill frame in window space, and the frame follows
  scrolling and layout while open. The composer and the Settings prompt sample each have one; the newest open press wins.
- Each skill chip is an accessibility button "Skill <label>. Show details" (the reference's trigger name), a child of the
  text view, whose press opens the details.
- `T3Module+Composer.swift`: `editorChip` (the newest press) and `editorChipClose` (the press the app saw).
- `composer-chip-popover.ts` (new): the `chip` resource. The label is the chip's; the composer's description and path
  come from the selected provider's skills for the workspace (`skill?.description ?? "No description is available for
  this skill."`, View instructions when the skill has a path, ChatComposer's `openMention` = `openFileSurface`); the
  Settings sample passes no skills (SettingsFontPreviews `EMPTY_SKILLS`). `editorlocal:chip-close` and
  `editorlocal:chip-instructions` close it (and open the file).
- `composer-chip-popover.contract` (new): PopoverPopup side top, align center, sideOffset 4, w-96 within the window,
  compact padding, flipped below when there is no room above and kept 5 pt inside the window; a dialog named "Skill
  <label>". `app-window.contract` draws it above every page (Base UI portals it) and closes it on a primary press outside
  it; `app.contract` declares the resource (`macos/src/markdown.rs` lists `composerChip` as TypeScript-owned).
- Review round: the composer stays mounted under Settings and the pages, where T3 Code's route change unmounts it, so
  ⌘, alone left the native press open and the popover (drawn last) over Settings. `app.contract` `chipCovered` (the
  composer's chip while Settings or a page covers the chat): T3Window hides it at once and the `chipCover` task closes the
  native press, so it does not come back on return. The palette and dialogs do not close it: the reference keeps the
  popover open above the palette, in the composer and in the Settings sample, and still open after the palette's Escape
  ([shots and aria](https://raw.githubusercontent.com/ccheever/exact2/02039b16526dd23fb43a427f7b87f3ae1e722938/settings-appearance-and-skill-chip/cover-record.txt)), as the clone already did.
- The details read the selected provider through `composer-editor.ts` `selectedProvider`, the one lookup the `$` menu
  and the chip labels read.

## Acceptance results

Lane `settings-appearance-and-skill-chip` (before: `…-before`), embedded server 16922, window 1280×840. Before: the
evidence-base build (feature-branch tip code). After: this branch's bundle, the same steps
([drive-ops](https://raw.githubusercontent.com/ccheever/exact2/ebba8b1eb0349bd33121ee0c824700d794e6af73/settings-appearance-and-skill-chip/drive-ops.txt), [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/ad98f2169bbc2179bd5000d9a54c1678d5b59d41/settings-appearance-and-skill-chip/drive.sh.txt)). Reference: the Electron reference on 16920/16921.

| Id | Result | Proof |
| --- | --- | --- |
| S1-5, Duplicate T3 Chat | Pass (agent): "Create theme", "T3 Chat copy", Advanced on, Foundation, Brand & content, Context and Status with all 20 rows equal to the reference's hex fields (before: simple, Background and Accent only) | [s1-5-duplicate](https://raw.githubusercontent.com/ccheever/exact2/221fba4a7d94330edb857295c815283ed47657d5/settings-appearance-and-skill-chip/s1-5-duplicate.png), [live record](https://raw.githubusercontent.com/ccheever/exact2/a384eb014b6c1413618e66392c851fe81699c2e0/settings-appearance-and-skill-chip/live-record.txt); `settings-appearance-advanced.test.ts` |
| S1-5, Edit of the saved copy | Pass (agent): "Edit theme" opens with Advanced on (before: simple, "Two colors, rest derived") | [s1-5-edit](https://raw.githubusercontent.com/ccheever/exact2/fcfa176e6a6ac82cfb39a4bb1210a78d7c27aa0a/settings-appearance-and-skill-chip/s1-5-edit.png), [live record](https://raw.githubusercontent.com/ccheever/exact2/a384eb014b6c1413618e66392c851fe81699c2e0/settings-appearance-and-skill-chip/live-record.txt) |
| S1-5, a managed theme opens simple | Pass (unit): a theme saved from simple mode is `managed` and its Edit opens simple; a copy of T3 Code's stock look and Create theme on it open simple | `settings-appearance-advanced.test.ts`; [logic before/after](https://raw.githubusercontent.com/ccheever/exact2/0d75badd2c1bfbe43854e16b91cb122b0b043940/settings-appearance-and-skill-chip/logic-before-after.txt) |
| S1-6 | Pass (agent, Open VSX live): dracula-theme, Dracula-2, GulajavaMinistudio, PROxZIMA, bceskavich, MateuszDrewniak, nszihan, lefd, as the reference (before: open-vsx, TimDeen, …, open-vsx, Verseth, WhiteVermouth, LEFD) | [s1-6-open-vsx](https://raw.githubusercontent.com/ccheever/exact2/c22793096d8dc306c4e23c0649db0731048b6275/settings-appearance-and-skill-chip/s1-6-open-vsx.png), [live record](https://raw.githubusercontent.com/ccheever/exact2/a384eb014b6c1413618e66392c851fe81699c2e0/settings-appearance-and-skill-chip/live-record.txt); unit test on the mapping |
| S1-12, the Settings sample's chip | Pass (agent, the agent's real mouse events at the chip): the popover above the chip, "Frontend Design" / "No description is available for this skill.", a dialog "Skill Frontend Design" (before: nothing) | [s1-12-preview-chip](https://raw.githubusercontent.com/ccheever/exact2/acc4a4d0dddfd3f8dff342ff1ab88e6aaef74df5/settings-appearance-and-skill-chip/s1-12-preview-chip.png), [live record](https://raw.githubusercontent.com/ccheever/exact2/a384eb014b6c1413618e66392c851fe81699c2e0/settings-appearance-and-skill-chip/live-record.txt) |
| S1-12, the composer's chip | Pass (agent): `$frontend-design now`, a press on the chip opens the same popover above it (before: nothing) | [s1-12-composer-chip](https://raw.githubusercontent.com/ccheever/exact2/1cabdd596ec9733a3e8e4e5f674584218d7e0d46/settings-appearance-and-skill-chip/s1-12-composer-chip.png), [live record](https://raw.githubusercontent.com/ccheever/exact2/a384eb014b6c1413618e66392c851fe81699c2e0/settings-appearance-and-skill-chip/live-record.txt) |
| S1-12, View instructions for a skill with a path | Pass (unit): the description and path come from the selected provider's skill of that name; the button closes the popover and opens the file. Not verified live: no provider in either lane lists a skill (the reference lane neither) | `composer-chip-popover.test.ts`; real-input batch step 4 |
| S1-12, the chip is named "Skill <label>" | Pass (XCTest): the text view lists a button "Skill Frontend Design. Show details" whose press opens the details. Not verified live: the agent's scoped `tree --ax` drops an accessibility element whose parent is not an Exact view (`AgentAccessibility.swift` `axOwner`), so it lists only the text area | [xctest](https://raw.githubusercontent.com/ccheever/exact2/171c770b72b615d558d63071b31d8637a57030f4/settings-appearance-and-skill-chip/xctest-chip-press.txt); real-input batch step 3 |
| S1-12, XCTest for the press report | Pass: four tests in `macos/tests/composer/chippress.swift` (real mouse events through `NSApplication.sendEvent`) | [xctest](https://raw.githubusercontent.com/ccheever/exact2/171c770b72b615d558d63071b31d8637a57030f4/settings-appearance-and-skill-chip/xctest-chip-press.txt) |
| S1-12, ⌘, with a composer chip's details open (review) | Pass (agent): Settings shows no popover and the press is closed (`chip.open` false), as the reference's route change; back in the chat it stays closed (before: the popover over Settings) | [cover-settings](https://raw.githubusercontent.com/ccheever/exact2/b016ed567e62ebf65495a1c6f009927d01229d1e/settings-appearance-and-skill-chip/cover-settings.png), [record](https://raw.githubusercontent.com/ccheever/exact2/02039b16526dd23fb43a427f7b87f3ae1e722938/settings-appearance-and-skill-chip/cover-record.txt), [steps](https://raw.githubusercontent.com/ccheever/exact2/b2dbdd170b22a4a38ba1361cc394c96c12f10e60/settings-appearance-and-skill-chip/cover-drive.sh.txt) |
| S1-12, ⌘K with the details open (review) | Pass (agent), unchanged: the popover stays above the palette and open after its Escape, in the composer and the Settings sample, as the reference | [cover-palette](https://raw.githubusercontent.com/ccheever/exact2/8e28f998458cbd24d0e3e91980fb7c6a59c13551/settings-appearance-and-skill-chip/cover-palette.png), [record](https://raw.githubusercontent.com/ccheever/exact2/02039b16526dd23fb43a427f7b87f3ae1e722938/settings-appearance-and-skill-chip/cover-record.txt) |
| S1-12, a real click on the native chip | Open (needs real input) | Real-input batch steps 1-2, 5 |

## Tests

- `settings-appearance-advanced.test.ts` (new, 6 tests): Duplicate T3 Chat opens Advanced with the reference's 20 values;
  every built-in opens Advanced, T3 Code's look simple; managed saves and Edit; a palette added to a guided theme; the flag
  through preferences, file and import; Open VSX publishers are namespaces.
- `composer-chip-popover.test.ts` (new, 9 tests): the popover's content for the composer and the sample, a closed or
  non-skill press, the resource against the provider's workspace skills and another thread's press, the details following
  the `$` menu's provider (`selectedProvider`), the close and View instructions ops, the Contract's placement, name and
  outside-press close, and the close under Settings or a page (not under the palette).
- `macos/tests/composer/chippress.swift` (new, 4 XCTests, registered in `main.swift`): a real click opens, toggles and
  closes; the frame follows a moved ancestor; Escape, an edit, the app's close and the editor leaving close; the newest
  press wins across the composer and the sample, with the provider's label; the accessibility button and its press.

Checks: see the PR ("Checks").

## Real-input batch steps

Launch this branch's bundle normally (not agent mode) from the worktree
`/Users/daehyeonmun/orca/workspaces/exact2/t3-code-settings-appearance-and-skill-chip`:

```sh
A=/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit L=$A/lanes/settings-appearance-and-skill-chip
T3_LOCAL_HOME=$L/clone-t3-home T3_LOCAL_PORT=16922 T3_LOCAL_RUNTIME_DIR=$A/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 \
  CODEX_HOME=$L/codex CLAUDE_CONFIG_DIR=$L/claude T3CODE_TELEMETRY_ENABLED=false \
  EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle --run
```

1. **Composer chip.** In the "New thread" composer type `$frontend-design now`. Click the "Frontend Design" chip: a popover
   above it reads "Frontend Design" and "No description is available for this skill.". Click the chip again: it closes.
   Click it, then click the heading "What should we build in work?": it closes. Click it, press Escape: it closes and the
   prompt is unchanged. Click it, type a letter: it closes.
2. **Settings sample chip.** Settings › Appearance › Typography: click the "Frontend Design" chip in the prompt sample: the
   same popover above it. Scroll the page: it moves with the chip. Press Escape: it closes and Settings stays open.
3. **The chip's name.** With Accessibility Inspector (or VoiceOver), inspect the composer's Message text area with a
   `$frontend-design` chip: it has a child button "Skill Frontend Design. Show details"; its press (VO-Space) opens the
   popover.
4. **View instructions.** Only with a provider that lists skills (the `$` menu shows rows): insert one from the `$` menu,
   click its chip: the popover shows its description and "View instructions"; click it: the skill's file opens in the
   right panel and the popover closes. The lane's providers list none, so this step needs a ready provider.
5. **Covered by Settings or the palette.** Click the composer's "Frontend Design" chip, press ⌘,: Settings shows no
   popover. Press Back: the chat shows no popover; click the chip once: it opens (it was closed, not hidden). Press ⌘K: the
   popover stays above the palette (as T3 Code's); press Escape: the palette closes and the popover is still open.

## Found, not in this task

- The theme editor's save button reads "Save theme" with the paintbrush when editing; the reference reads "Save changes"
  without an icon (`ThemeEditorPanel.tsx:1253-1258`; and `Merge into “…”` / `Add <appearance> palette` for a name another
  theme has). Visible in [s1-5-edit](https://raw.githubusercontent.com/ccheever/exact2/fcfa176e6a6ac82cfb39a4bb1210a78d7c27aa0a/settings-appearance-and-skill-chip/s1-5-edit.png).
- `decodeCustomThemes` drops a theme's `collection` when the preferences load, so an Open VSX collection card splits
  into single cards after a relaunch.
- The palette's Escape over Settings leaves Settings too in the clone (agent drive, [record](https://raw.githubusercontent.com/ccheever/exact2/02039b16526dd23fb43a427f7b87f3ae1e722938/settings-appearance-and-skill-chip/cover-record.txt) steps 46-49, before and
  after this change); the reference closes only the palette and stays in Settings. Not checked with real keys.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | Electron `1e2ecbd975` on 16920/16921 | Composer chip (after a reload, which builds the draft's chips), Duplicate, Edit, the sample's chip, Dracula | the reference column of each image |
| Before drive 1-2 | evidence-base | Stopped at an unquoted label, then at a tap outside the viewport (`mouse at` does not scroll; the contact form does) | — |
| Before drive 3 | evidence-base | Complete | [live record](https://raw.githubusercontent.com/ccheever/exact2/a384eb014b6c1413618e66392c851fe81699c2e0/settings-appearance-and-skill-chip/live-record.txt) |
| After drive 1 (the live drive) | this branch | Complete; every row passed, but the copy's Raised and Subtle surfaces read T3 Code's values (the clone's 19-role palette) | — |
| After drive 2 (the one retry) | this branch, the full built-in role sets and an opaque popover | Complete, every agent row passes | [live record](https://raw.githubusercontent.com/ccheever/exact2/a384eb014b6c1413618e66392c851fe81699c2e0/settings-appearance-and-skill-chip/live-record.txt) |
| Review: before drive | `5ddda47d4` (rebuilt: a Swift comment had changed after the first bundle) | The popover over Settings after ⌘, (the review's finding, confirmed) and above the palette | [record](https://raw.githubusercontent.com/ccheever/exact2/02039b16526dd23fb43a427f7b87f3ae1e722938/settings-appearance-and-skill-chip/cover-record.txt) |
| Review: reference | Electron `1e2ecbd975` over CDP | Settings (a route change) unmounts it; the palette leaves it open above and after Escape, so the palette part of the review's suggestion is not applied | [record](https://raw.githubusercontent.com/ccheever/exact2/02039b16526dd23fb43a427f7b87f3ae1e722938/settings-appearance-and-skill-chip/cover-record.txt) |
| Review: after drive | this head, a first build with the palette in the rule was stopped once the reference showed otherwise | Complete, both rows pass | [record](https://raw.githubusercontent.com/ccheever/exact2/02039b16526dd23fb43a427f7b87f3ae1e722938/settings-appearance-and-skill-chip/cover-record.txt) |

## Progress

2026-10-10: implemented S1-5, S1-6 and S1-12, unit- and AppKit-tested, built the bundle, drove the base and the branch
with the same steps, shot the reference, and opened draft PR [#364](https://github.com/ccheever/exact2/pull/364).

2026-10-10, review round: the composer chip's details close under Settings or a page (live before/after/reference), the
palette keeps them as the reference does, and the details read the `$` menu's provider through one helper.

## Next action

The coordinator runs the real-input batch steps, reviews the draft PR and merges it. The two items under "Found, not in
this task" are for the plan to schedule.
