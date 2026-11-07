# Scene activation refit

## Rough

The latest TestFlight capture showed the Duo system rail over an otherwise empty
Exact canvas. The iOS host refit its native navigation owner on
`willEnterForeground`, but a prewarmed scene can become active without that
notification.

## Lean in

`ExactView` now listens for `UIScene.didActivateNotification` as well as the
foreground transition. It lays out the parent window and Exact viewport, then
refits the native navigation owner. A deferred pass handles the final bounds
after a Duo display handoff. The scene delegate and the view's mount callback
also request the same refit, covering a prewarmed scene that activated before
the view joined its window.

## Needed

The hosted iOS test runner is occasionally unable to launch on the Duo
simulator. The app still needs a fresh physical TestFlight check after the next
build to confirm the reported display handoff.

## Checkpoints

- Added a regression for activation-only viewport and owner refitting.
- Added an explicit activation-refit regression for the prewarmed mount path.
- Focused activation test passed on the `t3-native-tabs duo` simulator.
- The broader mobile TypeScript lane still has its existing Home-header test
  mismatch; it is unrelated to this lifecycle change.
