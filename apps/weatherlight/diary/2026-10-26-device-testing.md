surfaces: ios, web
agent: Codex, GPT-6
task: verify location permission errors

## Rough

Safari could refuse a location request while its status reader did not report denial. The app now also displays the request failure. The new regression passes; the existing forecast-label contrast checks remain a separate failure. Debugging time was not measured.

## Lean in

A request-failure case can be reproduced by the app test without changing browser permissions manually.

## Needed

- location: provided, rough (see Rough)

## Checkpoints

- web dev loop: smooth
- editing app.contract: smooth
- driving the app: rough; the existing forecast contrast tests fail
