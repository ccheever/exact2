surfaces: ios
agent: Codex
task: continue frontend parity

## Rough

The source error banner uses the Codex provider mark for ChatGPT usage limits, and the queue uses native swipe removal, drag reorder and 65%/95% sheet snaps. The mobile port had the limit actions but still used its generic alert icon, menu-only queue rows and medium/large queue detents.

## Lean in

The existing provider icon contract and queue command seam were enough to add the source presentation without changing shared client files. Percentage detents are already understood by Exact's iOS navigation host, so the queue route can carry the source fractions directly.

## Needed

- files: provided
- accessibility: provider mark remains decorative; queue removal and reorder controls have labels
- haptics: existing selection haptic retained for removal and queue actions

## Checkpoints

- focused Bun tests: passed
- generated source check: passed
- mobile app Contract build: passed with existing warnings
- native populated-queue visual and gesture drive: pending
