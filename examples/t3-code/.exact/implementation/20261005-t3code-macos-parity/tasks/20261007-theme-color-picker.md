---
name: 20261007-theme-color-picker
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

# Choose custom theme colors with hue, saturation, brightness and RGB

## Outcome

The theme editor's color swatch opens the reference color picker: a saturation/brightness
plane, hue control, HEX field and RGB field. Users can choose arbitrary colors visually or
enter their RGB values, with every representation and the draft preview staying in sync.

This is a discovery record from the 2026-10-07 desktop comparison. No fix is included.
`verification: unverified` describes the future implementation, not the observed discrepancy.

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.

1. Open Settings > Appearance > Create theme in each running app.
2. Open the Background color swatch.
3. Inspect the controls offered by the popup.

The reference shows the saturation/brightness plane, hue slider, `Background picker hex value`
and `Background picker RGB value`. The Exact popup offers 24 fixed color swatches and closes
when one is chosen. Its surrounding editor row permits manual HEX entry, but the popup has
no arbitrary visual color selection or RGB entry. This changes how a user can choose a color;
it is not only a layout discrepancy.

Local evidence, relative to the checkout root:

- `target/desktop-audit/evidence/ref-settings-appearance-create-color.{png,txt}`
- `target/desktop-audit/native/native-settings-theme-color.{png,json}`
- `target/desktop-audit/native/native-settings-theme-color-selected.{png,json}`

These captures are local artifacts and are not committed. Only the controls and preset-selection
path were compared here; the acceptance rows below require new verification after implementation.

## Scope and guidance

Reference `apps/web/src/components/settings/ThemeColorPicker.tsx` defines `ThemeColorPickerPanel`,
RGB parsing, HEX synchronization and preservation of an existing alpha suffix. It uses
`ui/color-picker.tsx` for `ColorSaturationValuePlane` and `ColorHueSlider`. Port the reference's
input commit/invalid-draft behavior rather than normalizing incomplete edits on every keystroke.

Clone `settings-appearance-editor.contract` `EditorPicker` renders `presets`; the fixed catalog
is `PRESETS` in `settings-appearance-editor.ts`. `EditorColorRow` separately renders the HEX
field. Keep the editor's current draft/save/cancel path and replace the popup's limited selection
controls. The provider accent control in `settings-b-accent.contract` and `settings-b-accent.ts`
already implements a hue control, a saturation/brightness plane and HSV conversion; inspect
and reuse suitable behavior while preserving theme-specific alpha and input rules.

Apply the picker to every existing theme color row in Create, Edit and Duplicate, simple and
advanced modes. Keep the row field, popup fields, swatch and draft preview synchronized. The
reference's accessible plane exposes separately adjustable saturation and brightness values;
retain keyboard controls, visible focus, Escape dismissal and trigger focus return.

## Dependencies and deduplication

- [Settings scopes and theme editor](closed/20261005-settings-scoped-controls-and-theme-editor.md)
  owns the floating editor's app-wide lifetime, drag, resize, minimize and save notices. Its D16
  scope does not include the color picker's missing controls.
- [X30](../issues/20261005-x30-ts-announce-readback-picker.md) tracks Inspect/pixel readback.
  Choosing a color in the editor does not require screen inspection.
- No framework blocker was demonstrated; the example already has hue/plane controls for
  provider accents. Pure spacing and color differences remain in the audit's shared visual task.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Arbitrary visual selection | Drag/click the plane and hue control to a non-preset color | Selection marker, swatch, HEX, RGB and app preview update together as in the reference | Paired live drive and captures |
| HEX and RGB input | Enter valid HEX and RGB values; leave partial/invalid values and blur | Reference parsing, commit and invalid-draft behavior; values never diverge | Focused input tests and UI drive |
| Alpha preservation | Edit a theme role whose current value has an alpha suffix | Hue, plane and RGB changes preserve the alpha behavior of `ThemeColorPickerPanel` | Conversion test and saved theme readback |
| Reopen and switch colors | Close/reopen the popup and move between color rows | Each control starts from the current row's value without leaking another row's draft | Live drive |
| Editor modes | Exercise Create, Edit and Duplicate in simple/advanced modes; Save or Cancel | Saved theme equals the chosen colors; Cancel restores the prior theme | Theme readback and relaunch |
| Keyboard | Adjust saturation/brightness/hue with arrows, Shift, Home/End; Tab and Escape | Same accessible control behavior and focus return as the reference | Bounded keyboard drive |

## Next action

Implement the theme picker using the existing color-control capabilities, then run affected
app tests and a rebuilt macOS comparison before changing verification status.
