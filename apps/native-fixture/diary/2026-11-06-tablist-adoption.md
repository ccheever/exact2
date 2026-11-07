# A tablist rebuilt over the same panels

## Rough

Rebuilding the root tablist leaves native navigation referring to its old node. The new tablist projects a second bar and receives no layout cover. An app-owned container also retains the removed tabs as click targets.

## Lean in

The fixture can rebuild only its tablist while preserving the panels and their navigation stacks. A hosted regression checks adoption, the whole-node cover, hidden authored content and controller identity.

## Needed

Navigation adopts the current tablist on each replacement, resynchronizes the segment host and refreshes an app-owned container’s click targets.

## Checkpoints

The proposed fixture compiles with the same diagnostics as the original. `NavigationTabsIOSTests` initially passes bare and hosted on the iOS 27.1 Duo simulator. The added app-owned selection regression runs before review.
