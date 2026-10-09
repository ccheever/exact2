---
name: 20261009-settings-appearance-and-skill-chip
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
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

## Next action

Prepare a branch from `feat(example)/t3-code`. Build, unit-test and run the composer XCTests. Then do one batched live
drive at the end for every row's before/after pair. Close every row in this PR, or record the blocker of a row that
cannot pass.
