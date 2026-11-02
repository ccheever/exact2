# Transcript projection index

## Rough

The mobile thread adapter built a map for projected rows and then copied it into a second map of row payloads on every snapshot. A long transcript paid for both traversals before the virtualized list could draw its visible rows.

## Lean in

The adapter now keeps each projected row and its validated payload in one named index. Fork, activity, and copy metadata reads use that shared entry, so the output stays the same while one full-map allocation disappears from the scroll-facing snapshot path.

## Needed

The list still derives the complete transcript before the host virtualizes it. A device trace on a populated thread is still needed before claiming a frame-rate improvement.

## Checkpoints

- Focused thread, history, and scroll tests pass.
- `git diff --check` passes.
