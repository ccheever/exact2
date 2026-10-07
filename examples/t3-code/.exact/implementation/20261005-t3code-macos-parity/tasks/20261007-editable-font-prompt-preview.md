---
name: 20261007-editable-font-prompt-preview
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

# Type into the Appearance prompt font preview

## Outcome

Settings > Appearance lets the user edit the prompt sample to try its font family and size.
The preview supports text input, cursor movement, selection and undo while retaining the
sample's skill/file chips. Its draft belongs to the preview; typing never sends a message or
changes the active thread's composer draft.

This is a discovery record from the 2026-10-07 desktop comparison. No fix is included.
`verification: unverified` describes the future implementation, not the observed discrepancy.

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.
Both apps were running on the same Mac with source files unchanged.

1. Open Settings > Appearance, with Advanced typography off.
2. Click the prompt sample under Interface font and append ` AUDIT_FONT_PROBE`.
3. In the reference, press Command+Z to restore the sample.

| App | Observed result |
| --- | --- |
| Reference Electron | The sample accepts the suffix. The screenshot and `Prompt font preview` textbox show the changed text. Command+Z removes the suffix. No typography setting changes. |
| Exact | Clicking `prompt-font-preview` leaves a `View` containing only text and chip rows; no editor mounts. The native driver's attempt to type the suffix into that target returns `view 45943 is not an input`. The sample remains unchanged. |

The native result establishes the clicked surface and its lack of an input target. OS-key
simulation was unavailable in this accessory-mode session, so the driver refusal is not
presented as an OS typing trace. The source below independently confirms that the preview
has no editable control, draft state or input handler.

Local evidence, relative to the checkout root:

- `target/desktop-audit/evidence/ref-settings-prompt-preview-edited.{png,txt}`
- `target/desktop-audit/evidence/ref-settings-prompt-preview-restored.{png,txt}`
- `target/desktop-audit/native/native-final-prompt-preview-click.{png,json}`

These captures are local artifacts and are not committed. The input result above was recorded
by the native audit driver. The acceptance rows below require new verification after implementation.

## Scope and guidance

Reference `apps/web/src/components/settings/SettingsFontPreviews.tsx:30-56` defines
`PromptFontPreview` as an enabled `ComposerPromptEditor` with local prompt and cursor state.
Its `onChange` updates that state; it is intentionally interactive. `SettingsPanels.tsx:1588`
uses it for Prompt font in advanced mode, and line 1751 uses it under Interface font in simple mode.

Clone `settings-rows.contract:234` mounts `FontPromptPreview`; its definition at line 402
renders fixed text and `FontChip` rows. It has no input, focus or editing state. Replace this
static preview with an editable preview using the existing composer editing/chip behavior where
practical. Inspect `composer-editor.contract` and `modules/apple/T3ComposerEditor.swift` when
choosing the integration; keep its document, selection and focus separate from the active thread.

Use the reference's initial sample and local lifetime. Font family/size changes must update
the preview text and chips while following the reference's draft behavior. Match its mount/remount
reset behavior rather than persisting the sample as a setting. This task does not add message
submission, provider calls or a second conversation composer.

## Dependencies and deduplication

- [Desktop visual parity](20261007-desktop-visual-parity.md), V01, owns the prompt-chip icons
  and highlighted code sample. This task owns editing behavior. Coordinate the shared preview
  component without duplicating the cosmetic acceptance rows.
- [Interface font size conversion](closed/20261005-interface-font-size-conversion.md) records
  sizing of the sample text and chips. It does not track input, cursor state or undo.
- [Composer fidelity](closed/20261005-composer-fidelity.md) owns the conversation composer;
  it does not cover editing the Settings preview.
- [Installed font picker](20261007-installed-font-picker.md) and X48 own installed-family
  enumeration/application. Editing with currently supported families does not depend on X48.
- No framework blocker was demonstrated by this comparison. Reuse of the existing editor
  needs implementation verification; do not silently substitute a plain field that loses chips.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Type and undo | Click the sample, append ` AUDIT_FONT_PROBE`, then Command+Z | Suffix appears and undo restores the sample, matching the reference | Paired screenshots and input/selection state |
| Cursor and selection | Move the caret, select and replace text before and after the chips; delete and undo | Same text/chip editing and caret behavior as the reference | Bounded live input drive |
| Font changes | Edit the sample, then change a supported family and font size | Text and chips reflect the chosen typography; draft behavior matches the reference | Before/after captures |
| Simple and advanced | Exercise Interface font's sample, then advanced Prompt font's sample | Both are editable; switching/remounting follows reference lifetime | Live captures |
| Draft isolation | Keep an unrelated draft in an active thread, edit the preview, then return | Thread draft is unchanged; no message, provider operation or preview preference write occurs | UI readback and command log |
| Focus | Enter and leave the preview by keyboard; type editing keys while it is focused | Accessible editable surface; editing keys affect the preview, and focus returns to Settings controls normally | Keyboard drive and accessibility tree |

## Next action

Implement the isolated editable preview, then run affected composer/app checks and rebuild
the macOS app for a paired live comparison before changing verification status.
