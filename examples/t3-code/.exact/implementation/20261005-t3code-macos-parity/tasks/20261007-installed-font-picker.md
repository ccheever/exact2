---
name: 20261007-installed-font-picker
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Appearance font pickers list and apply installed families

## Outcome

Settings > Appearance offers the installed font families that the reference desktop app offers.
A selection applies the actual family to its intended text and survives relaunch. Code and
Terminal reject a proportional family with the reference's error and keep the current value.

This is a discovery record from the 2026-10-07 desktop comparison. No fix is included.
`verification: unverified` describes the future implementation, not the observed discrepancy.
Complete application of a runtime-selected family is blocked by [X48](../issues/closed/20261007-x48-runtime-font-family.md).

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.
Both apps ran on the same Mac; source files were unchanged during the comparison.

1. Open Settings > Appearance and enable Advanced typography.
2. Open Code font, search for `Arial`, and select it in the reference.
3. Open the same picker and search in the clone.

| Observation | Reference Electron app | Exact app |
| --- | --- | --- |
| Code font catalog | Installed families include Arial and Menlo | Only the default SF Mono option |
| Search `Arial` | Arial is selectable | `No fonts found.` |
| Select proportional Arial for Code | Error title `"Arial" isn't monospace`; description `Code and terminal need a fixed-width font, so the current font was kept.`; Code remains Menlo | Cannot reach this choice through the picker |
| Interface font catalog | Installed-family picker | Default SF Pro plus SF Pro Rounded, New York and SF Mono |

Local evidence, relative to the checkout root:

- `target/desktop-audit/evidence/ref-settings-appearance-advanced-code-font-family.{png,txt}`
- `target/desktop-audit/evidence/ref-settings-appearance-code-proportional-rejected.{png,txt}`
- `target/desktop-audit/native/native-settings-code-font-menu.{png,json}`
- `target/desktop-audit/native/native-settings-code-font-arial.{png,json}`
- `target/desktop-audit/font-probe/results.json` and `catalog.txt`. A CoreText probe enumerated
  247 families, including Arial, Menlo, Monaco and Georgia. These local artifacts are not committed.

## Scope and guidance

Reference `apps/web/src/components/settings/FontFamilyPicker.tsx` shares enumeration state
between the rows, searches all discovered families, previews each item in its own face and
validates monospace eligibility on selection. Its `handlePick` preserves the existing setting
when rejecting a proportional face. `appearanceFonts.ts` supplies discovery and advance-based
monospace checks. Inspect the reference's unavailable-enumeration input fallback when implementing.

Clone `settings-appearance.ts:141-159` contains a fixed family list, an empty `MONO_FAMILIES`
and aliases that collapse named faces into generic stacks. `settings-appearance-look.ts:66`
uses those aliases for the app. `settings-a-fonts.contract` renders the picker, and
`settings-rows.contract` uses finite generic-family choices for the previews. Replace the
restriction when X48 permits a discovered family string to reach Contract text.

The existing Swift app module can enumerate CoreText families and measure their advances.
Enumeration alone is not the framework blocker. Current Exact already renders installed
families named as literals; the comment that only generic or bundled faces work is outdated.
Preserve the distinction between a missing family, a rejected proportional family and the
default stack. Do not relabel a generic fallback as the selected installed face.

Cover Interface and Code in simple mode, plus Prompt and Terminal in advanced mode. Keep the
reference's simple-mode sharing and advanced-mode independence. This task does not change
font-size ranges, root `rem` scaling, text smoothing or bundled font licensing.

## Dependencies and deduplication

- [X48](../issues/closed/20261007-x48-runtime-font-family.md) blocks complete runtime selection through Contract.
- `closed/20261005-interface-font-size.md` and `closed/20261005-interface-font-size-conversion.md`
  concern sizes and `rem`; they do not implement installed-family discovery or validation.
- `research.md` calls the Monospace font row already done. The presence of the row does not
  cover its catalog or actual face selection.
- X10 concerns ellipsis, wrapping, balance, placeholders and smoothing. It does not cover this gap.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Installed catalog | Open every font picker on a Mac with Arial, Menlo and Georgia installed; search by name | Same applicable catalog and search behavior as the reference; default entry remains available | Paired screenshots and native catalog result |
| Code and Terminal validation | Select Arial with Menlo currently selected | Exact reference error, setting remains Menlo, no preference write of Arial | UI capture and preference readback |
| Real face selection | Select a non-default installed proportional face for Interface/Prompt and Menlo for Code/Terminal | Preview and application text use the named face, including its own bold/italic variants where present | Native resolved face names plus visible before/after text |
| Persistence and mode switching | Relaunch, then switch simple/advanced typography | Family settings survive and sharing follows the reference | UI and preference readback |
| Unavailable family | Restore a preference naming a family absent on this Mac | Reference-compatible fallback and truthful trigger; no crash or mislabeled generic alias | Targeted test and live capture |
| Controls | Search, keyboard-select, Escape and restore default | Same selected value, focus return and dismissal behavior as the reference | Bounded live drive |

## Next action

X48 was filed as [#318](https://github.com/ccheever/exact2/issues/318) on 2026-10-08: this task waits for main fix of #318, then an adoption round; then
implement the catalog, validation and application path in the example.
Run the affected app tests and a rebuilt macOS comparison before changing verification status.
