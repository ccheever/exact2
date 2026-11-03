# Queue gestures

## Rough

The mobile queue exposed menu actions for removal and movement, while the source queue uses native swipe removal and an adjustable reorder handle.

## Lean in

Queue rows now reveal a Remove action during a horizontal swipe, snap back with the Exact spring transition, and expose a native reorder handle through the list's reorder group. Existing queue tickets and commands remain the mutation boundary, so stale rows still go through the same admission checks.

## Needed

Physical-device visual acceptance still needs a populated queue and a native drag and swipe run.

## Checkpoints

- Contract build: `examples/t3-code/mobile/app.contract` succeeded.
