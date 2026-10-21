# Diary — Grow a Garden (Godot 4.7.2)

2026-10-06T05:14:26Z — Start. Lane is Godot 4.7.2 stable at /opt/homebrew/bin/godot, GDScript, no editor session. Brief is a deterministic 3D garden (14 crops, weather mutations, shop, orders, offline catch-up) proven by a headless script plus windowed screenshots. Reference shots are low-poly, south-of-gardener camera, HUD chrome around the view.

Choices where the brief is silent (recorded now, before the code locks them in):

- One integer LCG drives every roll (`state = state * 1664525 + 1013904223`, low 32 bits). Shop restock is rolled before the first weather delay, both at new-game, crops in table order.
- Rainbow is its own draw (p=0.001); Gold is a second draw (p=0.01) only if Rainbow missed. Snow is Chilled 0.4, otherwise a separate Frozen 0.1. Thunderstorm is Wet 0.5 and an independent Shocked 0.03.
- Same-timestamp events apply weather, then restock, then plants in (z, x) order, so a fruit that ripens as rain starts sees rain.
- Watering multiplies the remaining phase time by 3/4 and shifts the phase start so the progress fraction (and therefore the stage boundaries) stay put. One water per growth-to-maturity and one per fruiting cycle.
- Feeding sets a flag consumed at the next ripen roll. Feeding again before that roll spends nothing. A ripe single-harvest plant has no upcoming roll, so Feed spends nothing.
- Stage is a pure function of the phase clock (1/4, 1/2, 3/4, done). Only maturity and ripening are simulation events.
- Tile underfoot is half-open: tile x covers world x in [2x−1, 2x+1). Plot numbers shown to the player are 1-based.
- Tie for nearest empty plot: smaller z, then smaller x (south row, then west). That picks Plot 2,1 east of a gardener standing on Plot 1,1.
- Game clock is integer milliseconds. The day clock label is total minutes:seconds (`Day 2:30`), not the 10-minute sky cycle. The sky cycle is 600s, noon at t=0.
- Top-bar money is compact above 1,000 (one decimal). Action notes and the market bonus line use the full sheckel count.
- Weather is shown even when Clear, because every reference shot does.
- Outline priority: ripe green, else fed purple, else watered blue, else growing/fruiting amber, else empty cyan.
- Plants sit 0.55 m east of the tile centre so the gardener does not cover them.
- Walk bounds are the soil rectangle expanded by 20 m. Diagonal speed is capped at 4 m/s. Acceleration is 16 m/s².
- Decorative scatter uses a fixed hash, never the game RNG.
- Presentation effects (dirt, water, harvest chime, rain meshes, rainbow hue) do not touch the RNG or any rule.
- Offline catch-up is the same `advance_ms` the live game calls. Wall time enters only as `now − saved_unix` on load. `GARDEN_NOW_MS` overrides the clock so the proof can open an hour later without changing the machine clock.

2026-10-06T06:03:53Z — Visual pass before the full proof. Vertex colours were in the glTF and the importer left `vertex_color_use_as_albedo` off, so day shots were pale grey. The meadow plane was also culled from above until cull was disabled. Camera now frames the whole plot from the south. Running `sh proof/run_proof.sh`.

2026-10-06T06:09:13Z — Full proof `sh proof/run_proof.sh`. LOGIC PASS, SHOTS PASS, exit 0. Frame times: 100 plants 1.82 ms, 1000 plants 8.23 ms. Rain still is daytime rain. Thunder still landed inside a bolt. Night is a clear midnight, lantern glass lit on the fence.

Friction, one line each:

- GDScript will not infer a type from an untyped RefCounted return, so the proof needed explicit types on every sim local. A script that failed to parse still quit 0 while `fails` stayed empty, and an early "LOGIC PASS" was false until the dump files existed. Cost: a handful of headless reruns.
- `plant_at` was inferred as null, and every subscript of it failed to parse. Declaring the return as Variant fixed it. One rebuild.
- `advance_ms` compared the next event with `<` and skipped a ripen that fell on the exact target millisecond, so the carrot never became ripe at 20 s. One failed proof.
- Godot 4.7 names the filmic mapper `Environment.TONE_MAPPER_FILMIC`. `TONE_MAP_FILMIC` does not exist. One parse error.
- `harvest.wav` imported as "no loader" until a second `godot --import`. A few minutes.
- Daylight shots were white. glTF base factors are white and the PNGs are grey masks; the paint is COLOR_0, and the importer leaves `vertex_color_use_as_albedo` off. Turning the flag on restored the overalls, hat, trees, and flowers. Found by reading the glTF, then one recapture.
- The meadow is a one-sided 600 m plane. From above it drew nothing, so the "ground" was the sky colour (exact background RGB). Disabling cull fixed it. Vertex green is about (32, 73, 8), so the meadow albedo is lifted in place. The texture and the vertices are still the kit. Two recaptures.
- The first camera sat on the south-west corner and cropped the plot. Framing the whole garden from +Z (south), a little west, matches `reference/arrival.png`. Two recaptures.
- Rain parented to the camera, and `emitting = false` left streaks alive for their lifetime, so a clear night still showed rain. Hide the particle node, and restart it when a spell begins.
- A bolt is 90 ms wide. Stepping 40 ms for 1.6 s missed it, and an 800 s walk missed the spell (thunder is 15% of changes). Jumping to the next hashed bucket inside the spell caught one. The still is a blown exposure plus rain, not a drawn lightning mesh.
- The barrel in the kit is brown wood with dark bands. The reference drawing and the "blue barrel" copy are bluer than the mesh. Left the mesh alone.

End. 2026-10-06T06:09:58Z.

- Game code: 2736 lines in `game/*.gd` and `game/*.gdshader` (sim 1233, world 710, ui 608, main 143, shaders 42). Proof scripts 458 lines. Engine, `art/`, and `.godot` imports not counted.
- Files written: `project.godot`, `icon.svg`, `game/main.tscn`, `game/main.gd`, `game/sim.gd`, `game/world.gd`, `game/ui.gd`, `game/soil.gdshader`, `game/outline.gdshader`, `game/harvest.wav`, `proof/logic_proof.gd`, `proof/shot_proof.gd`, `proof/run_proof.sh`, `README.md`, `DIARY.md`, `DONE.json`.
- Iterations to a passing proof: the rule proof was already green before the picture was. This visual pass took three framing captures and four full `sh proof/run_proof.sh` runs. The run at 06:09:13Z is the one that passed with the flash visible.
- Pixel looks: arrival against `reference/arrival.png` five times, then shop, backpack, market, grown, rain, night, and thunder.
- Brief items not done: none. Approximations: the barrel stays the brown cask from the kit; thunder is a timed exposure flash; snow falls in the sim and has particles, and the required stills are rain and night.
