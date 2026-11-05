surfaces: ios
agent: Codex gpt-6-astra
task: expose a distinct Retry control for failed older-history reads

# History retry presentation

## Rough

When an older-history request failed, the mobile feed kept the ordinary “Load
earlier activity” button beside the error text. The action retried correctly,
but the visible label and accessibility name did not describe the recovery
state.

## Lean in

The ordinary control is now hidden while an error is present. The error keeps
its status announcement and receives a separate “Retry” button with the
reference retry label and an explicit disabled state for inactive or missing
route owners. Loading and automatic paging keep their existing ownership and
single-request guards.

## Needed

A native paired-server failure drive is still needed for light, dark, loading
and error screenshots. This source correction makes no frame-rate claim.

## Checkpoints

- The focused history suite passes 34 tests and 232 assertions.
- Contract compilation, strict TypeScript, generated-source verification and
  the full mobile suite pass.
- The normal signed iOS simulator build installs and opens on the local iPhone
  Duo.
