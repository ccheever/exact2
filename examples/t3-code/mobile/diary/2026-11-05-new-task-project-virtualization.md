# New Task project list virtualization

## Rough

The New Task chooser kept its static subtitle, scratch action and empty state in the same scroll list as every project row. A large catalog therefore mounted every row even though only the visible rows needed to be interactive.

## Lean in

Static chooser content now stays outside the project list. Project rows use a virtualized list with a 49-point estimate, while an empty chooser reports no scroll owner because no list exists.

The mobile route shell now clips its own vertical overflow so a native home tab bar cannot become the fallback page scroll view while the home list is loading or empty. Route lists and feeds remain the scroll owners.

## Needed

A native large-catalog drive is still needed for exact row recycling and VoiceOver order evidence.

## Checkpoints

- Added a Contract regression for the virtualized project list and empty-state scroll ownership.
- Added a route-overflow regression for native tab-bar ownership and verified it on an iOS simulator.
- The full mobile suite remains the validation gate.
