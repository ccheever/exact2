# Working status motion

## Rough

The Contract already rendered every status in one persistent row, so a status
change replaced its contents abruptly and the capsule width jumped. The pinned
mobile control crossfades status rows and lets the glass capsule reflow. Exact's
conditional arms and layout-transition property provide those primitives without
adding a native timer or view.

## Lean in

Keyframe entry and exit animations compose with the existing status branches,
while the reduced-motion flag disables entering and layout motion. The layout
transition keeps the measured preview, agent and queue segments stable as the
status label changes.

## Needed

- motion: provided by Contract keyframes and layout transitions
- native glass: provided by the existing composer material module

## Checkpoints

- editing app.contract: smooth
- focused mobile tests: smooth
- Contract build: smooth
