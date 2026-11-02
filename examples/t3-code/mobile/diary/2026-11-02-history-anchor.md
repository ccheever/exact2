surfaces: ios
agent: Codex gpt-6-astra
task: preserve the visible row while older history is prepended

# History anchor preservation

## Rough

The history feed records the first visible row and its offset before loading an
older page. The prepend can emit another scroll event before the restore action
runs. That event replaced the saved anchor with a row from the shifted list.

## Lean in

The root keeps the existing anchor record while a newer history revision is
waiting for restoration. Once the restore advances the saved revision, normal
scroll events can record a new anchor again.

## Needed

scroll anchoring: by hand (a populated-thread iOS drive is still needed to
verify that the restored row keeps its offset)

## Checkpoints

- The focused history suite passes 33 tests and 217 assertions.
- The pre-fix regression failed while the prior 32 tests passed.
- `git diff --check` passes.
