surfaces: ios
agent: Codex gpt-6-astra
task: match single-tool work-group headers to the pinned mobile feed

### Rough

The mobile row derivation treated a completed group containing one tool as a direct entry. That skipped the source work-group header and exposed the detail row at the wrong height and disclosure level.

### Lean in

Lone tool activities now use the same group projection as multi-tool activity groups. The existing group toggle owns disclosure, and an expanded group publishes its detail row without changing item identity or copy ownership.

### Needed

An owned iOS drive still needs to compare the 38.5-point header, typography and expanded detail geometry against the reference. This source correction makes no native pixel claim. The Contract build and iOS simulator build now pass; the remaining visual check needs an available device session.

### Checkpoints

- Focused thread/work tests pass: 166 tests and 1,232 assertions.
- Full mobile suite passes: 4,252 tests, 2 skips, 0 failures, 37,772 assertions.
- Contract generation succeeds with no errors, and the iOS simulator build completes.
- `git diff --check` passes.
