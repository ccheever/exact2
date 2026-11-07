# Compact Home search lifecycle

## Rough

The compact Home search controller was recreated whenever the Contract echoed a typed query. On iOS, replacing an active controller can dismiss its field after the first character.

## Lean in

Home chrome now keeps the active `UISearchController` attached while its query is echoed. It only installs the search button when the field is inactive, preserving the native text field and first responder during filtering.

## Needed

A current iPhone drive should type multiple characters into Home search and confirm the field stays active while the list filters.

## Checkpoints

- Focused mobile tests
- Contract build
- iOS no-install build
- EAS TestFlight build scheduled from the fix commit
