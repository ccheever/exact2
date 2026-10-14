---
name: 20261007-installed-font-picker
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-appearance-theme-tools
pr_url: https://github.com/ccheever/exact2/pull/548
verified_commit: 3edaae26884e28b820506e0b54dc5cee030baf7e
---

# Appearance font pickers list and apply installed families

## Outcome

Settings > Appearance offers the installed font families that the reference desktop app offers.
A selection applies the actual family to its intended text and survives relaunch. Code and
Terminal reject a proportional family with the reference's error and keep the current value.

The original 2026-10-07 discrepancy is recorded below. X48 is now adopted. App-only implementation
landed locally in `6accb80fbc021fad73772a302e2af4dbf46326ed`, followed by
`3edaae26884e28b820506e0b54dc5cee030baf7e`. The verification receipt below distinguishes
completed checks from acceptance cases not exercised; the full task remains unverified.

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

## Original scope and guidance

Reference `apps/web/src/components/settings/FontFamilyPicker.tsx` shares enumeration state
between the rows, searches all discovered families, previews each item in its own face and
validates monospace eligibility on selection. Its `handlePick` preserves the existing setting
when rejecting a proportional face. `appearanceFonts.ts` supplies discovery and advance-based
monospace checks. Inspect the reference's unavailable-enumeration input fallback when implementing.

The audited clone’s `settings-appearance.ts:141-159` contained a fixed family list, an empty `MONO_FAMILIES`
and aliases that collapse named faces into generic stacks. `settings-appearance-look.ts:66`
used those aliases for the app. `settings-a-fonts.contract` renders the picker, and
`settings-rows.contract` used finite generic-family choices for the previews. The implementation removes those restrictions using the adopted runtime font-family support.

The existing Swift app module can enumerate CoreText families and measure their advances.
Enumeration alone is not the framework blocker. Current Exact already renders installed
families named as literals; the comment that only generic or bundled faces work is outdated.
Preserve the distinction between a missing family, a rejected proportional family and the
default stack. Do not relabel a generic fallback as the selected installed face.

Cover Interface and Code in simple mode, plus Prompt and Terminal in advanced mode. Keep the
reference's simple-mode sharing and advanced-mode independence. This task does not change
font-size ranges, root `rem` scaling, text smoothing or bundled font licensing.

## Dependencies and deduplication

- [X48](../issues/closed/20261007-x48-runtime-font-family.md) is adopted and permits runtime selection through Contract.
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

## Implementation and verification receipt — 2026-10-14

`T3InstalledFonts.swift` reads `NSFontManager.availableFontFamilies` and fixed-pitch traits.
`appearance-fonts.ts` caches a validated catalog per client. All four pickers search the
installed catalog without case sensitivity, and clear their query each time they open.
Code and Terminal reject a proportional selection before writing the preference and show
the reference error. Named families stay named in the runtime CSS font stack.

The family reaches the interface, Prompt composer/placeholder, Code Markdown/diff/gutter/Files
text, Terminal, and the picker/setting previews. The app uses the adopted runtime font-family
support; no framework code changed.

| Check | Result | Local evidence |
| --- | --- | --- |
| Full app Bun suite | 4,861 passed; one existing skip; no failures | `target/appearance/full-tests.log` |
| Contract, strict app TypeScript, staged caps | Passed on `3edaae268` | `contract-followup.log`, `typecheck.log`, `caps.log` under `target/appearance/` |
| Native installed catalog and named faces | AppKit suite: 47 passed, including installed Arial/Menlo and fixed-pitch classification | `target/appearance/menus.log` |
| Interface Arial; Code and Terminal Menlo; reject Arial | Rebuilt native UI: mixed-case search finds the family; rejection toast appears and Menlo remains selected | `target/appearance/font-drive.json`, `font-checks.json` |

The native drive uses an isolated runtime home and an empty local thread. It types a local
draft only; it sends no provider request. Earlier rehearsal receipts that stopped before
runtime readiness or tried an offscreen Prompt trigger are not acceptance evidence.

The final rebuilt native drive passed all 28 checks on `3edaae268`. It opens each popup before
querying it, scrolls the Prompt trigger into view, selects Georgia, closes Settings, and types
`Font appearance check` into the real composer without sending it. The snapshot reports
`promptFont: "\"Georgia\", system-ui"`; the composer layout reports a dynamic font family
and the screenshot shows the serif text. It opens the Diff panel, expands the local
`appearance.ts` file, and renders the +1/-1 code and gutters with a dynamic family while
`look.codeFont` is `"Menlo", ui-monospace`.

Evidence under `target/appearance/`: `font-drive.json`, `font-checks.json` (28/28),
`composer-layout.json`, `diff-text-layouts.json`, `after-composer-window.png`, and
`after-diff-window.png`. Both screenshots were visually reviewed. Native build:
`followup-build.log` (Cargo 228.2 s; Swift 1.2 s; exit zero).

The first diff check found a collapsed file; `collapsed-diff-*` preserves that failed recipe
precondition. The final receipt expands the visible file before requiring line rendering.
Native layout exposes a dynamic family ID, not a resolved `NSFont` face name; resolved names,
bold/italic variants, Files/PR live typography, full persistence/mode-switching, absent-family
restoration, and keyboard selection/focus-return coverage remain unverified. These limits
keep the full task’s verification status open; the bounded catalog, validation, and runtime
family wiring work is implemented and checked as above.

## Coordinator publication, 2026-10-14

Draft PR [#548](https://github.com/ccheever/exact2/pull/548) was published and read back at
`31d46d0a21e1de85626ce04a0d81068fcd8cc6cd`, based on `feat(example)/t3-code`.
The [durable native packet](https://github.com/ccheever/exact2/blob/ffe98932fe5943814d09069cbb210d2f91c5d01d/appearance-theme-tools/20261014-resume/README.md)
contains accepted final screenshots, source/binary identities, and compact baseline/after
observations. Its transparent baseline PNG is rejected, and differing fixtures are declared.
Independent review approved the bounded implementation; the outstanding full-task
verification rows above remain open. This publication update is coordinator bookkeeping.

### Remote delivery, 2026-10-14

GitHub reports this PR merged at 03:04 UTC into `feat(example)/t3-code`, published as
first-wave checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a`. This records delivery
only; the task's remaining acceptance and real-input qualifications above still apply.
