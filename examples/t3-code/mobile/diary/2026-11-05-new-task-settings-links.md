# New Task settings deep links

## Rough

The mobile New Task flow declares child settings routes for runtime, provider and option choices, but the app-link allowlist stopped at the settings root.

## Lean in

The public link normalizer now admits those declared child routes and keeps their query strings and route visits intact.

## Needed

An external `t3code:` delivery drive is still needed for native URL-delivery evidence.

## Checkpoints

- Added coverage for all three child settings routes.
- The focused navigation-link suite remains the validation gate.
