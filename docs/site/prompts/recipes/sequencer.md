```prompt Add a step sequencer
Add a step sequencer to the Exact app in this folder: lanes of steps that play sounds in a loop, with Play and Pause. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/sequencer.md as the source for each step. The exact2 checkout has the same page at docs/recipes/sequencer.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not give the lanes (their sounds, steps, beats a step and hits) and the tempo, propose them and ask me before you write code. Use only sound files that are in the app's folder; if there are none, ask me for them.
3. In app.contract, declare each file a lane can name with a `sound` line; an undeclared file is silent. Add the recipe's `Step`, `Track` and `Transport` shapes with their fields in the same order, and a `lane` function.
4. Put the tracks in a `derive` and play them with one `sequence ... when playing`, `play(tracks, bpm=..., from=at)`. Pause saves the beat in `at` from the `exactTransport()` resource. Use a key for a mode, not a second `sequence`.
5. If the app runs on iOS and does not play in the background, add a task on `transport.reason == "background"` that sets `playing` to false.
6. Add a test in app.test.contract that taps Play, moves the clock, and uses `expect sound has "<file>" at <ms>` for each lane, then changes a lane and expects the new sound.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me the lanes, the tempo and the sound files, what the tests and verify reported, and the launch command for each target platform.
```
