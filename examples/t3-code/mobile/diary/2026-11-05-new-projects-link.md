# New Projects deep link parity

## Rough

The mobile New Task flow already recognized `/new/projects` as its project chooser, but the app-link allowlist rejected that public route before the flow could see it.

## Lean in

The public link normalizer now admits `/new/projects` and maps it to the declared `/new` route. Query strings and the originating route visit remain unchanged.

## Needed

This is a source and Contract-link regression. A device drive from an external `t3code:` link is still needed for native URL-delivery evidence.

## Checkpoints

- Added a navigation-link regression for `/new/projects`.
- Existing route and mobile test suites remain the validation gate.
