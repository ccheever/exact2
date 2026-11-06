# Launch snapshot lifetime

## Rough

The root snapshot carried the live clock as an identifying argument. That made
the resource's kept answer ineligible at launch, so a cold render could show
Home's empty projection before the first current snapshot arrived.

## Lean in

The snapshot serial remains the identity that invalidates data. The clock now
arrives through the resource's `with` context, keeping elapsed labels current
without discarding a suitable launch answer.

## Needed

A cold release launch with a populated saved catalog should compare the first
visible frame against the reference. The simulator service was unavailable for
that drive in this checkpoint.

## Checkpoints

- Contract source coverage asserts the stable snapshot identity and clock context.
- Focused scroll and app tests remain to be run after this change.
