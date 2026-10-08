---
name: 20261008-x61-field-focus-ring-opt-out
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/302
reproduced_on: febb2c5fb and e200397ec (main; one-file apps) and 74114cde1 (feat(example)/t3-code-adopt-main-fixes-r6, main e200397ec merged)
---

# X61: an app cannot remove the focus ring Exact draws on a bare text field or textarea (no `outline`)

**Status (reclassified 2026-10-08):** Bucket 4, approved, no fix in progress: #302 (`outline: none` on fields); no PR.

## Summary

On macOS, Exact draws a 2 pt ring in `keyboardFocusIndicatorColor` around every focused bare `input` and
`textarea` (LLP 1104 D6), `appearance="none"` included since main `5b2b77339`. Contract has no `outline`
property, so an app cannot take the ring away as CSS `outline: none` does on the web. T3 Code removes the
ring wherever it draws its own focus look (`outline-none`), for example on the composer and the Appearance
prompt preview. The requested support is CSS's `outline-style`/`outline-width` (at least `outline: none`)
on fields, honoured by every host's Exact-drawn ring, or another documented opt-out.

## Why this issue arose

### The T3 Code behavior
- `apps/web/src/components/settings/SettingsFontPreviews.tsx` `PromptFontPreview` (1e2ecbd975): the preview
  is a `ComposerPromptEditor` inside a bordered box, `focus:outline-none`; focusing it shows the caret only.
- The composer (`ComposerPromptEditor`, Tiptap) has `outline-none` as well: the composer card shows no ring.
- `components/ui/input.tsx`: the field itself is `outline-none`; focus is the wrapper's `border-ring` and a
  3 px `ring-ring/24` shadow, which the clone draws itself.

### What exact2 does today
- `bun scripts/exact.mjs contract vocab outline`: "`outline` is not a tag or an attribute" (main `e200397ec`).
- `host/apple/Sources/ExactKit/Mac/FieldEditingMac.swift` `showFieldFocus`: the ring shows for any enabled
  field that is not a native single-line field (`!isNativeTextControl || textArea != nil`); for a textarea it
  is a layer above the scroll view (`exact.fieldFocus`). Nothing reads an author row.
- Before `5b2b77339` (main `1f19b2400`) the ring followed r4's `fieldStyle` mark, which a literal
  `appearance="none"` left out, so `appearance="none"` was the opt-out. LLP 1104 r9 D6 keeps Exact's ring on
  every bare field and textarea, so that opt-out is gone.

### Where the clone hits it
- `settings-prompt-preview.contract` (`prompt-font-preview`, `appearance="none"` for exactly this reason,
  [editable-font-prompt-preview](../tasks/closed/20261007-editable-font-prompt-preview.md)): no ring on the base
  `07dcef1ab`, a ring on the merged branch (image 02 of [adopt-main-fixes-r6](../tasks/closed/20261008-adopt-main-fixes-r6.md)).
- `composer.contract` (`composer`): the ring is on both builds (r4's sheet ring on the base, the ring layer
  now; image 03), where the reference shows none.
- Every other bare field of the clone shows the same ring when focused (the palette input, branch and model
  searches, the inline settings fields). Where the reference's field draws its own focus look the clone
  duplicates it.

## Why it must be resolved

The composer is focused most of the time the app is used, and a blue frame around it is the most visible
difference from T3 Code on the main screen. The web standard lets an author remove the UA ring with
`outline: none`. LLP 1104 D6 says the web host restores the ring for bare fields through `:focus-visible`, so
the same gap should be there too (not run here: the clone is macOS-only).

## Requested support

`outline: none` (or `outline-style: none` / `outline-width: 0`) on an `input` or `textarea`, honoured by the
ring Exact draws on macOS, iOS, Linux and the web. A whole `outline` vocabulary is not needed for this.

## How to reproduce

One-file app (`bun scripts/exact.mjs new <dir>`), view:

```contract
component RingProbe
  state text = "A bare textarea"
  state other = "appearance none"
  action write(value: string)
    text = value
  action note(value: string)
    other = value
  view
    column padding=24 gap=16 background-color="#ffffff"
      column padding=12 border-radius=16 border-width=1 border-style="solid" border-color="#d4d4d8"
        textarea value=text input=write testId="bare-textarea" height=48 padding=0 border-width=0 background-color="#00000000"
      column padding=12 border-radius=16 border-width=1 border-style="solid" border-color="#d4d4d8"
        textarea value=other input=note testId="none-textarea" appearance="none" height=48 padding=0 border-width=0 background-color="#00000000"
```

`bun exact.mjs mac`, then `bun exact.mjs agent macos --size 420x420 "tap bare-textarea" "type bare-textarea key
ArrowRight" "screenshot a.png" "tap none-textarea" "type none-textarea key ArrowRight" "screenshot b.png"`:
both show a 2 pt blue ring inside the card (image 05 of adopt-main-fixes-r6, which also shows a bare `input`).
Expected: a way to draw no ring, as Chrome draws none with `outline: none`.

## Acceptance for the fix

- `contract build` accepts `outline="none"` on `input` and `textarea`.
- The probe above with `outline="none"` on both textareas shows no ring on macOS (screenshot), and the
  web JS target shows none in Chrome.

## App adoption after resolution

Add `outline="none"` to the composer, the prompt preview, the file editor and the inline fields whose
reference is `outline-none`; keep the ring where the reference keeps the browser's.

## Status and next action

Local draft (2026-10-08, adopt-main-fixes-r6). Reproduced on main `e200397ec` with a one-file app and in the
clone. Not searched upstream, not published: publication is the coordinator's (the brief forbids filing from
task agents).

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/302 (#302, [Feature] `outline: none` on `input` and `textarea`, to
remove the focus ring Exact draws on a bare field). Filed as a feature request, not a bug: LLP 1104 r9 D6 keeps the
ring on purpose, and what is missing is the author's `outline: none`. Reproduced on main `febb2c5fb` with a one-file
app: a bare `textarea`, a `textarea appearance="none"` and a bare `input` each show a 2 pt blue ring inside the app's
card on macOS, and Chrome's focus outline on the web; `outline="none"` is refused (`[lower-unknown-attr]`). Evidence:
[x61-bare.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-bare.png), [x61-none.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-none.png), [x61-input.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-input.png), transcript
[x61-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-ops.txt).

Next: `issue-close` once #302 lands: add `outline="none"` where the reference is `outline-none` (App adoption above).

## Decided upstream (2026-10-08): waits for main fix of #302

[Charlie on #302](https://github.com/ccheever/exact2/issues/302#issuecomment-6055582892): "Add the requested outline:none suppression."
- Waits for main fix of [#302](https://github.com/ccheever/exact2/issues/302), then an adoption round. On adoption, `outline="none"` goes on the composer and the Appearance prompt preview.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): approved feature, medium (input and textarea only).
