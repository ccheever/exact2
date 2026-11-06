# Pull Requests navigation

## Rough

The PRs tab selected the archived threads route and tab changes pushed duplicate roots onto one stack. The live T3 Code surface has a Pull Requests list backed by `pullRequests.list`.

## Lean in

The mobile contract now declares independent tab roots and selects an existing tab stack for tab chrome events. The PR tab uses the existing mobile pull request projection and presents loading, error, empty, search, refresh and grouped row states.

## Needed

The native compact tab host must preserve its selected item while the route echoes back through Exact. iOS 17 through 25 receive the same four-item tab bar through the legacy UIKit path.

## Checkpoints

- Contract build passes with no errors.
- Focused mobile tests pass (40 tests, 134 assertions).
- iOS development build completes. Device installation was unavailable because the available simulator sessions are owned by other workspaces.
