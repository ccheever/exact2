# 2026-11-07 — compact workspace tab labels

## Rough

The Expo compact workspace dock shows icons while keeping tab names for
accessibility. Exact's authored tabs also contained visible text, so the iOS
projection could fall back to a labelled tab bar instead of the icon-first
chrome used by the reference.

## Lean in

The four root tabs now declare `aria-label` values and omit visible text
children. Exact's iOS tab projection can therefore keep the labels for
VoiceOver while rendering the deployed icon tokens as the visible face.

## Needed

A current iPhone and Duo install must confirm the glyphs, selected tint and
hit targets after the native tab host settles. Device-panel startup was
unavailable during earlier checkpoints, so source and build checks remain the
local evidence until a fresh drive is possible.

## Checkpoints

- Full mobile suite: 4,325 passed, 2 skipped.
- Contract build with no errors.
- Signed iOS simulator build from this checkout.
