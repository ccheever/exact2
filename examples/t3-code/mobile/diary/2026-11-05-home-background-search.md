# Home background search parity

## Rough

Mobile Home search only queried the focused environment. A synchronized background environment could appear in Home with a matching thread, but its message excerpt was missing because the search result was never requested for that environment.

## Lean in

Home search now fans out to the focused connection and synchronized fleet connections, using each connection's own transport and generation. Results retain their environment identity, so Home rows resolve excerpts without crossing environments. The sidebar operation delegates to this mobile search adapter while the copied shared source remains explicitly marked as a mobile adaptation.

## Needed

The focused Contract and full mobile test suite pass. A native simulator drive with two synchronized environments is still needed for visual confirmation of the background excerpt.

## Checkpoints

- Home regression coverage verifies a background environment match and its scoped excerpt.
- `bun test examples/t3-code/mobile/client.test.ts` passes: 56 tests.
- `bun test examples/t3-code/mobile/*.test.ts` passes: 4297 tests, 2 skipped.
- Native iOS build and simulator drive remain to be run after this change.
