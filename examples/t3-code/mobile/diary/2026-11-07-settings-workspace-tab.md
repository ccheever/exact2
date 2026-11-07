# Settings workspace tab parity

## Rough

The Settings root was still marked as a compact sheet and as a fullscreen route on split layouts. That hid the Duo workspace rail and exposed a Close or Back control that did not match the current Expo workspace navigation.

## Lean in

Settings now remains a persistent workspace tab on compact and split layouts. Its root has no native Close or Back control; tab selection dismisses it. Settings child routes keep the ordinary stack presentation and the Duo rail remains owned by the workspace.

The pull request detail scroll owner now has the same explicit native scroll id as its route declaration, so status-bar and tab reselect scrolling can reach both PR roots.

## Needed

The current device bridge was not required for the Contract verification, but a fresh iPhone and Duo capture is still needed before claiming pixel-level parity for the tab and rail geometry.

## Checkpoints

- Settings presentation and header tests pass.
- Expanded workspace and fullscreen route tests pass after rebuilding the Contract compiler output.
- Next checkpoint: native iOS no-install build and TestFlight submission from this commit.
