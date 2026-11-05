# History anchor gap

## Rough

The prepend anchor was retained when a scroll event named a different row, but a native event with no first-visible key cleared it before restoration. That transient event can occur while the virtualized feed is reflowing older rows.

## Lean in

The root now preserves the captured anchor for a matching route and thread while the history revision is newer than the saved revision. Once restoration finishes, a normal no-key event can clear the stale anchor again.

## Needed

The Contract regression proves the state transition. A populated native thread drive is still needed for exact pixel-offset acceptance.

## Checkpoints

- Added a no-first-visible-key regression to the existing history anchor test.
- Focused and full mobile tests, strict TypeScript, Contract compilation, and the normal iOS build are the acceptance checks for this slice.
