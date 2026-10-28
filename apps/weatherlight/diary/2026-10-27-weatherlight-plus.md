surfaces: web, macos, ios
agent: Claude Code, Claude Opus 5.5
task: Weatherlight+, the consumer of @exact/purchases and @exact/store-review (LLP 1133.015 §10)

## Rough

- Nothing in exact2 could start a package's implementation before its first
  call, keep a module source's answer for the next launch, or ask a source
  again at a time its answer named. All three were built in exact2 for this
  (LLP 1133.015 D16 items 1, 2 and 4): `"start": "load"`, `"kept": true`, and
  `exact:stale` beside an answer. About half the work of the stage.
- `Result<'visit'>` in `app.ts` was refused: `Result` takes the source's
  name (`visits`), not the resource's. The note under the error named every
  source, so the fix took a minute.
- `visits(time.epochAtZero)` drew `lower-resource-launch-identity`: the date
  changes every launch, so a kept answer could never show. `with
  time.epochAtZero` passes it without counting it, as the warning said.
- StoreKit's local test session (`SKTestSession`) runs only inside a hosted
  XCTest, and on this Mac a hosted test bundle got
  `SKInternalErrorDomain` 4 from the StoreKit service (ad hoc signed; the
  development identity cannot sign without a person unlocking the keychain).
  The real StoreKit path is compiled and not yet run against a configuration
  file; see the LLP's as-built notes.

## Lean in

- The two facts read cleanly in Contract: `plus` is a derive over
  `listPurchases`, and the delivery task is keyed by the unfinished tokens, so
  a purchase approved days later and one just made take the same path.
- The purchase ticket's taps (`buy`, `pending`, `unavailable`, `fail`) made
  every path of the Support panel drivable without a store.

## Needed

- purchases: provided (`@exact/purchases`)
- store review: provided (`@exact/store-review`)
- an App Store id for a "Rate Weatherlight" row and a privacy policy page
  for the subscription screen: not yet (no App Store record)

## Checkpoints

- editing app.contract: smooth
- the delivery loop in app.ts: smooth
- driving purchases under the agent: see the LLP's as-built notes
