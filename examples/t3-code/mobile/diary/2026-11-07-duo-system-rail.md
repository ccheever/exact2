# Duo system rail ownership

## Rough

Split Home chrome always installed the app-owned footer tab bar. On iOS 27's unfolded Duo, that left the four workspace destinations below the sidebar instead of using the vertical rail that owns the device's right-edge occlusion.

## Lean in

When UIKit reports a vertical system bar, the sidebar Home chrome now installs the existing window-level system tab controller. Split devices without that trait keep the footer bar. Both paths use the same Contract tab selection and native icon fallback.

## Needed

A current Duo drive must confirm the rail's four hit targets, selected tint, and delayed content-geometry reservation after folding and rotation.

## Checkpoints

- Focused mobile tests
- Contract build
- iOS no-install build for the Duo simulator
