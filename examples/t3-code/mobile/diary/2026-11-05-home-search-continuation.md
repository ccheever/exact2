surfaces: ios
agent: Codex gpt-6-astra
task: keep related Home search results visible while environments answer

# Home search continuation

## Rough

Mobile Home message search waited for every connected environment before
publishing any result. A slower background environment could therefore blank a
matching list, and refining a query discarded the previous result immediately.

## Lean in

Search state now keeps one answer slot per environment. Each answer publishes
its scoped matches and leaves the search pending until all slots settle. While
a query is a prefix extension or contraction of the previous query, the prior
matches remain visible until the new answer arrives. Unrelated queries remain
empty while pending, and stale searches cannot publish after a newer query
owns the state.

## Needed

Native accessibility readback and a paired-server screenshot comparison remain
open for the filtered Home states. This source change makes no frame-rate
claim.

## Checkpoints

- The focused Home search suite passes 3 tests and 12 assertions.
- Existing Home state tests pass unchanged.
