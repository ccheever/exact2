---
name: 20261008-x61-field-focus-ring-opt-out
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/302
reproduced_on: febb2c5fb and e200397ec (main; one-file apps) and 74114cde1 (feat(example)/t3-code-adopt-main-fixes-r6, main e200397ec merged)
---

# X61: an app cannot remove the focus ring Exact draws on a bare text field or textarea (no `outline`)

Moved to main `issues/20261009-field-outline-none.md` (2026-10-09); tracked there.

## Summary

On macOS, Exact draws a 2 pt ring in `keyboardFocusIndicatorColor` around every focused bare `input` and
`textarea` (LLP 1104 D6), `appearance="none"` included since main `5b2b77339`. Contract has no `outline`
property, so an app cannot take the ring away as CSS `outline: none` does on the web. T3 Code removes the
ring wherever it draws its own focus look (`outline-none`), for example on the composer and the Appearance
prompt preview.

## Why it arose

### The T3 Code behavior
- `apps/web/src/components/settings/SettingsFontPreviews.tsx` `PromptFontPreview` (1e2ecbd975): the preview
  is a `ComposerPromptEditor` inside a bordered box, `focus:outline-none`; focusing it shows the caret only.
- The composer (`ComposerPromptEditor`, Tiptap) has `outline-none` as well: the composer card shows no ring.
- `components/ui/input.tsx`: the field itself is `outline-none`; focus is the wrapper's `border-ring` and a
  3 px `ring-ring/24` shadow, which the clone draws itself.

### Where the clone hits it
Before `5b2b77339` (main `1f19b2400`) a literal `appearance="none"` left the ring out, and the clone used it
as the opt-out; main adoption round 6 brought in LLP 1104 r9 D6, which keeps Exact's ring on every bare field
and textarea.
- `settings-prompt-preview.contract` (`prompt-font-preview`, `appearance="none"` for exactly this reason,
  [editable-font-prompt-preview](../../tasks/closed/20261007-editable-font-prompt-preview.md)): no ring on the base
  `07dcef1ab`, a ring on the merged branch (image 02 of [adopt-main-fixes-r6](../../tasks/closed/20261008-adopt-main-fixes-r6.md)).
- `composer.contract` (`composer`): the ring is on both builds (r4's sheet ring on the base, the ring layer
  now; image 03), where the reference shows none.
- Every other bare field of the clone shows the same ring when focused (the palette input, branch and model
  searches, the inline settings fields). Where the reference's field draws its own focus look the clone
  duplicates it.

## Clone workaround

None: the ring shows, a visible difference on the main screen (the composer is focused most of the time).
On adoption, add `outline="none"` to the composer, the prompt preview, the file editor and the inline fields
whose reference is `outline-none`; keep the ring where the reference keeps the browser's.

## Evidence and history

- Local draft (2026-10-08, adopt-main-fixes-r6), reproduced on main `e200397ec` with a one-file app (a bare
  `textarea` and an `appearance="none"` one in bordered cards, both with a 2 pt blue ring, image 05 of
  adopt-main-fixes-r6) and in the clone on `74114cde1`.
- Filed as [#302](https://github.com/ccheever/exact2/issues/302) ([Feature] `outline: none` on `input` and
  `textarea`, to remove the focus ring Exact draws on a bare field) on 2026-10-08: a feature request, since
  LLP 1104 r9 D6 keeps the ring on purpose. Reproduced on main `febb2c5fb` with a one-file app: a bare
  `textarea`, a `textarea appearance="none"` and a bare `input` each show a 2 pt blue ring on macOS, and
  Chrome's focus outline on the web; `outline="none"` is refused (`[lower-unknown-attr]`). Evidence:
  [x61-bare.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-bare.png),
  [x61-none.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-none.png),
  [x61-input.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-input.png),
  transcript [x61-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-ops.txt).
- Accepted upstream on 2026-10-08 ("Add the requested outline:none suppression").
