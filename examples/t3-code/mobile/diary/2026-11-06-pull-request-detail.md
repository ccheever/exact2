surfaces: ios
agent: Codex
task: add the mobile pull request detail route

## Rough

The existing shared detail projection was ready, but the mobile root had no source adapter or route. Adding the route required regenerating the contract source declarations before the TypeScript dispatcher tests could compile.

The EAS checkpoint upload reached the service, but the final GraphQL build request failed with a network error after the 409 MB archive upload. Retrying with the installed and latest EAS CLI versions reproduced the failure; no build ID was returned.

## Lean in

The shared pull request detail projection supplied loading, error, summary, checks, comments and timeline data without duplicating transport logic.

## Needed

none

## Checkpoints

- editing app.contract / app.ts: smooth after regenerating source signatures
- iOS build: smooth
- driving the app: not reached; the device tool was unavailable in this checkout
