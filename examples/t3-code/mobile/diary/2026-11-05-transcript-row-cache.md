surfaces: ios
agent: Codex gpt-6-astra
task: avoid rebuilding transcript rows during unchanged scroll renders

# Transcript row cache

## Rough

The projection index was reused during an unchanged scroll redraw, but the
shared transcript presentation still rebuilt every fold, activity and message
row before the mobile adapter walked them.

## Lean in

The mobile presenter now reuses the complete transcript presentation while the
canonical revision, selected thread, local fold state, timestamp format,
worktree setup state and activity-detail presentation are unchanged. Local
disclosure and fetched output changes advance a small presentation revision,
so they invalidate the cache without requiring a synthetic server update.

## Needed

The focused and full mobile suites pass. A device frame trace comparing a long
feed before and after this change is still needed before reporting a numeric
frame-rate gain.

## Checkpoints

- `bun test examples/t3-code/mobile/thread.test.ts` passes: 92 tests.
- `bun test examples/t3-code/mobile/thread-work.test.ts` passes: 50 tests.
- `bun test examples/t3-code/mobile/client.test.ts` passes: 56 tests.
- Contract build, strict TypeScript and `git diff --check` pass.
