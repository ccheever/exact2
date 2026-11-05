# Complete native search traits across mobile filters

## Rough

New Task project search and the workspace inspector search were the last mobile
search fields without the lowercase, autocomplete and spellcheck traits already
used by the other search surfaces.

## Lean in

Both fields now disable capitalization, autocomplete and spell checking while
retaining their existing search owners, labels and query actions. The change
keeps the mobile search controls consistent with the platform search field.

## Needed

The populated New Task and inspector routes still need a connected native drive
for visual keyboard and clear-button confirmation.

## Checkpoints

- Extended the search-field contract regression to cover both fields.
- The full mobile suite and strict TypeScript check cover the declarations.
