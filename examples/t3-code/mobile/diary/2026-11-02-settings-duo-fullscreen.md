# Settings fills an unfolded Duo window

## Rough

An unfolded Duo window showed Settings constrained to the workspace pane while Home used the full window. The Settings route also used the split stack presentation, so its native route inherited pane geometry intended for the underlying workspace.

## Lean in

The route presentation and route content width now agree. Split Settings presents as a fullscreen route and its route root uses the viewport width. The underlying workspace geometry remains unchanged.

## Needed

A post-change Duo drive should confirm that Settings fills the unfolded window and that its scroll content still clears the native bottom inset.

## Checkpoints

- `bun test examples/t3-code/mobile/new-task.test.ts examples/t3-code/mobile/mobile-workspace.test.ts`
- `bun scripts/exact.mjs contract build examples/t3-code/mobile/app.contract --json`
