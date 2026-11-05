# Files search uses the native search field

## Rough

The standalone Files route used a plain text input for its search query. On iOS that omitted the native search field behavior: lowercase input traits and the clear button that the reference exposes. The model and branch filters had the same mismatch, including the scheduled-task branch picker.

## Lean in

All four mobile filter fields now declare `type="search"`, disable capitalization, autocomplete, and spellcheck, and keep their existing query owners and actions. Exact maps those declarations to the platform search field, so clearing a field remains an ordinary query edit.

## Needed

The standalone Files route still needs a connected-device visual comparison against the reference. This change is limited to the authored field declaration and does not alter file reads or navigation.

## Checkpoints

- Added a source regression for the native search declaration.
- Focused mobile tests and the full mobile suite remain to be run.
