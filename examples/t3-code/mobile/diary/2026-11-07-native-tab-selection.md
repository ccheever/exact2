# Native tab selection handoff

## Rough

The native Home coordinator initialized both the iPhone tab controller and the Duo rail to Chats on every mount. Compact Home also installed an extra Open settings button even though Settings is a root tab.

## Lean in

The Contract now passes the selected root tab in the Home chrome configuration. UIKit applies that value after creating or refreshing the native tab bar, and compact Home leaves Settings to the tab bar while the sidebar keeps its gear action.

## Needed

The device bridge was unavailable during this pass, so the selected icon and compact header still need a fresh iPhone and Duo drive.

## Checkpoints

- Focused Home, root, and native configuration tests pass.
- Full mobile suite passes 4,325 tests, with 2 intentional skips.
- Contract build and the normal iOS no-install build pass; the build emits the existing Contract warnings.
