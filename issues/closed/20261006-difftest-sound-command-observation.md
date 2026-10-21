# Record runtime-owned sound commands in differential observations

**Status:** Closed
**Resolution:** Fixed by 142693a34; the driver observes the voice table commands at Sounds.apply.
**Systems:** semantics differential tests, sound commands
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** semantics/README.md; LLP 1071; LLP 1092

The JS differential driver stops observing sound commands after sound ownership moved into the runtime.

Reproduced with `difftest corpus --js-only`: `corpus/actions/play-sound.contract` (“sounds are commands, in order”) diverges. Rust's next observation is `command playSound "assets/tick.wav" none none none`; JS's next observation is the view, with that command missing. Failure artifacts were retained under `target/difftest/failures/20001-js-27.*`.

The driver at `semantics/difftest/js/drive.mjs:149–153` records commands by replacing Hosts entries. But `host/web-js/rt.js:263–269` gives playSound/playSounds/stopSounds to Sounds.own and returns before calling Hosts. Sounds.apply receives admitted command batches separately. The driver's generated bundle/context does not record that ownership path.

Observe sound commands at their actual admitted commit boundary, without playing audio during a headless semantics test. Preserve the order and all arguments, and distinguish refused/rolled-back command batches.

Acceptance: this fixture and playSounds/stopSounds observations agree with the Rust transcript, including a refused action that must emit no admitted sound. Do not remove the command from the oracle, ignore the divergence, or route runtime-owned audio through the ordinary host facade just to satisfy the test.
