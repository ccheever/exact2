# Thread anchor cache

## Rough

The mobile root kept only one thread scroll anchor. Selecting another thread in
the compact workspace replaced the row and offset for the previous thread, so
returning to it could land at the wrong place.

## Lean in

Thread history now stores a bounded cache of up to 100 anchors keyed by route,
environment and thread. A route change saves the current anchor and restores the
matching saved row and within-row offset when the thread returns. A reader who
reaches the live end, or taps the explicit scroll-to-end control, clears that
thread's saved position. Prepend revision guards and transient no-key handling
remain in force.

## Needed

A populated native thread drive still needs to confirm pixel-preserving restore
and accessibility readback. This source correction makes no performance claim.

## Checkpoints

- The focused history suite passes 35 tests and 236 assertions.
- The mobile Contract builds successfully with the repository's existing warning set.
