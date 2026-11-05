surfaces: ios, web
agent: Codex gpt-6-astra
task: expose filtered Home message matches to assistive technology

# Home search accessibility

## Rough

Filtered Home rows already painted the server's message excerpt below the
thread title, but the row button's accessibility name stopped at the title.
VoiceOver could therefore select a result without hearing why it matched.

## Lean in

The Home thread row now derives one accessibility name from the same title and
excerpt that the row paints. Queued rows retain their existing queued-message
suffix. The visual layout and row identity stay unchanged.

## Needed

Native accessibility readback and a paired filtered Home screenshot remain
open. This source correction makes no native performance claim.

## Checkpoints

- Focused Home, Home state and search tests pass: 77 tests and 6,254 assertions.
- The full T3 Code suite passes: 13,177 tests, 4 intentional skips and 220,376 assertions.
- Strict app TypeScript and the Contract build pass.
