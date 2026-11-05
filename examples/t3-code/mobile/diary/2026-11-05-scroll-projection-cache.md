surfaces: ios
agent: Codex gpt-6-astra
task: avoid rebuilding the transcript projection index during scroll renders

# Scroll projection cache

## Rough

The feed's scroll action can redraw the route without changing the selected
thread projection. Each redraw still walked every visible turn item to rebuild
the source lookup used by message and activity rows. That work was separate
from the existing bounded Markdown block cache and grew with the full history.

## Lean in

The mobile thread presenter now keeps that lookup per client while the source
array, client revision, thread epoch and selected thread remain unchanged. A
live projection update changes the client revision, and a selection change
changes the epoch or thread ID, so both rebuild the index before presenting the
new rows. The map retains the same row and item objects, allowing in-place
streaming updates to remain visible without a second copy of the transcript.

## Needed

The source change qualifies less JavaScript work for an unchanged scroll
snapshot. A device-side frame trace comparing a seeded long feed before and
after this change is still needed before reporting a numeric frame-rate gain.

## Checkpoints

- The focused mobile thread suite passes 90 tests and 503 assertions.
- `git diff --check` passes.
