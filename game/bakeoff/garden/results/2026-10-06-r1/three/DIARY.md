# Diary

2026-10-06 05:03:10 UTC — Started. Read BRIEF.md, LANE.md, ART.md, and the reference shots. Engine is three.js from npm, Bun, headless Chromium proof. This diary file was only opened on resume (05:57 UTC); the clock times above and below until 05:57 are the `date -u` readings from that first stretch, not reconstructed.

2026-10-06 05:22:07 UTC — Installed three and playwright. Wrote the seeded sim (`src/rng.js`, `src/data.js`, `src/format.js`, `src/sim.js`) and `proof/logic.mjs`.

2026-10-06 05:22 UTC — Friction: `bun proof/logic.mjs` failed once. `advance(1)` took 59 motion steps and `advance(0.5)+advance(0.5)` took 60, because `1 - 59/60` sits a hair under `1/60`. Fixed the accumulator compare (`STEP - 1e-8`) and clamped a tiny remainder. Re-ran: `logic ok`. Cost a few minutes, one extra run. Carrot had been given `fruit_s: 30` by mistake and was corrected to 0 before that run.

2026-10-06 05:57:27 UTC — Resumed after context compaction. Page shell (`index.html`, `src/style.css`, `src/ui.js`, `server.js`) was already on disk and had never been opened in a browser. Scene, glue, and proof still unwritten. Asset front is unresolved: ART.md says models face −Z, a bounds check said the hat and the legs extend toward +Z. Will render with markers before locking a mount yaw.

2026-10-06 06:24:33 UTC — Logging the visual pass that happened after 05:57 and was not written down then. First meadow render was nearly black (physical lights times dark vertex colors, meadow rgb about 13, 35, 2). Raised hemisphere, ambient, and sun, and later added lantern point lights so night at Day 8:50 stays readable. Cost several screenshot rounds.

2026-10-06 06:24:33 UTC — Friction: ART.md says the kit faces −Z. A mount yaw of 0 put the face on the far side of the head (blue −Z marker on the back of the neck; more head verts on +Z). Mount yaw is π so the face, bib, and hat brim aim with the parent's forward. Close-up confirmed it. Do not flip it again.

2026-10-06 06:24:33 UTC — Friction: the barrel glb is a wood tub with vertex colors, not a blue barrel. Tinted the wood `#2f9fe6` and added a pale water disc. The stall and keeper are in the scene; the stall camera sees the back of the keeper's head because the keeper faces the garden.

2026-10-06 06:24:33 UTC — First full `bun proof/proof.mjs` (06:16 UTC) got through logic, P1–P5, then died on P6 "ripe too soon" with the carrot ripe at t=15. The sim was right (logic.mjs asserts not ripe at 15 and passed in the same command). The proof held the live plant object and read `fruits.ripe` after the following `advance(1)`, which is the moment it ripens. Snapshot the fields before that step. Also the 1,000-plant loop only added 900 after harvest removed one-shot crops; it now keeps planting until the count reaches 1,000. Real-time clock now follows `auto` alone so Pause is actually what stops time in that check.

2026-10-06 06:24:33 UTC — Running the proof again.

2026-10-06 06:26 UTC — `bun proof/proof.mjs` passed. P1–P8, pause, the narrow layout (market, care, and the prompts do not overlap at 390×800), and P10 screenshots. Frame time about 2.6 ms with 101 plants and about 5.5 ms with 1,082. Looked at all nine shots.

2026-10-06 06:26 UTC — Friction, from those shots: the seed shop ran down over the Move button, and Sell all printed `1643180¢`. Shortened the left panel so it stops above Move, and used the compact money format for the backpack total (`1.6M¢`). The title help was wrapping the word "feed" alone, so it is two deliberate lines. Re-ran the proof.

2026-10-06 06:29:37 UTC — Second full pass: `PROOF PASSED` again. Shop, title, and backpack shots rechecked. The planted carrot sprout sits east of the gardener and is visible beside the hat; the prompt pills cover its lower edge.

End stats:
- Game code is 3,041 lines (index.html, server.js, src/), not counting three.js, the art kit, or the proof. The proof is another 661 lines.
- Files written for the game: index.html, server.js, package.json, README.md, src/data.js, src/format.js, src/main.js, src/rng.js, src/sim.js, src/style.css, src/ui.js, src/world.js, proof/logic.mjs, proof/proof.mjs. DIARY.md appended. DONE.json written at the end.
- Iterations: logic self-check failed once (motion step remainder), then passed. Full proof failed twice on a live plant reference in P6, then passed, then passed once more after the panel fix. Four full proof runs, one of them the first green.
- Looked at rendered pixels about 20 times (night, stall, and the narrow layout before they were logged; then the nine proof shots, a crop of the sprout, and title/shop/backpack again).
- Approximated, not skipped: the barrel glb is a wood tub, tinted blue with a flat water disc. Night lanterns throw a wide warm pool across the soil rather than tight circles. Trees are the brief's stage-4 apple, mango, and coconut models scaled up. Thunder flashes and snow are in the renderer; the proof shoots rain and night, not a separate thunder or snow frame. The weather chip stays visible on Clear.
