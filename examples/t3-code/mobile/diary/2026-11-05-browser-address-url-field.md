# Mobile browser address bar uses URL input traits

## Rough

The mobile preview browser rendered its address bar as a generic text input. That
gave iOS no URL keyboard and allowed autocorrection or spell checking to alter a
location before navigation.

## Lean in

The address bar now declares a URL input, disables capitalization, autocomplete,
autocorrection and spell checking, and keeps the selected tab's existing draft
owner and Enter action. Navigation behavior is unchanged.

## Needed

A connected preview tab is still needed for a native keyboard screenshot. This
change makes no claim about browser rendering or navigation success.

## Checkpoints

- Added a Contract regression for the URL field declaration and traits.
- Browser-focused and full mobile checks cover the change.
