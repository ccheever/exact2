# Lifecycle divider chrome

## Rough

The mobile transcript reference uses centered divider rows for context compaction and provider handoffs. The clone rendered those lifecycle entries as ordinary text rows, and the handoff projection discarded its endpoint detail before Contract drew it.

## Lean in

Handoffs now retain their endpoint summary and use a dedicated mobile row kind. Compaction and handoff rows share the reference hairlines, centered symbol, muted label and one-line detail treatment. Failed handoffs keep the danger tint.

## Needed

Provider icons and resolved model names still need a richer mobile snapshot before the handoff divider can match the source endpoint chips exactly.

## Checkpoints

- Focused thread coverage checks the handoff projection and divider declarations.
- Contract generation, the full mobile suite and the iOS build remain to be run after this edit.
