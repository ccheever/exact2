# History scroll events avoid redundant anchor writes

## Rough

The virtualized thread feed reports the same first-visible row on successive
scroll events. Each event rebuilt the route anchor array even when the key,
offset and history revision were unchanged.

## Lean in

The root now updates the retained anchor only when one of those facts changes.
The existing route, thread and revision guards still own prepend restoration;
this removes repeated filtering and allocation from a steady scroll without
changing the anchor that is restored.

## Needed

- native long-feed frame trace: still needed before claiming a measured frame
  rate change
- populated anchor drive: still needed for exact native offset acceptance

## Checkpoints

- Added a Contract regression for the repeated-event no-op.
