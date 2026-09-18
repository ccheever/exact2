# Native engine lane evidence, 2026-09-17

Host: ccheever@100.84.186.119, expose-builder-nothing, Linux x86_64, Python3.12.3. Everything installed in /tmp/exact-engine-comparison-20260917/native. No global installs, no clone, no repository edits. Zero human interventions; zero visual/pixel inspections. Pygame's SDL dummy surface was drawn and a PNG emitted, but not visually checked. No shipping/export or real keyboard/controller injection verified.

Versions: Godot4.7.2.stable.official.ed1daf0bf, LOVE11.5, Pygame2.6.1, Defold1.13.1. versions.json includes engine binary SHA256. The three engine sources were baseline-hashed before H1 was disclosed to implementation; baseline-sha256.txt and *-baseline/ retained (hashing was after disclosure receipt, before edits).

Baseline: actual engine objects; rectangle player moves x0->120 via60 explicit2px movements; three lanterns at30/60/90 collected; reset. Godot uses ColorRect+Rect2.intersects, Pygame Sprite+Surface+spritecollide, LOVE real Box2D dynamic body and static sensor fixtures with world:update(1/60). Godot/Pygame are application-controlled logic ticks, not manual physics engine stepping. LOVE x119.99996185 is within.001 tolerance. All3 passed5/5 process repetitions, not5 independent authoring trials. Median process startup+proof: Godot.15836s, LOVE.02093s, Pygame.13141s. Downloads separately: Godot1.547s, LOVE.616s, Bob11.232s; Pygame pip target-directory1.576s. Extraction and agent authoring not included in these setup subtotals.

Held-out H1: fourth lantern at160; advance20 more ticks; application boolean act(down) handles rising-edge collection; two held-true invocations leave three collected, false then true collect fourth; save, reset, restore final x/count. Godot/Pygame serialize JSON in memory; LOVE saves Lua table in memory. No persistent save-file test, no native input path, no subsequent collision replay after restore. H1 final passes5/5 each. One research agent made all changes. No meaningful per-engine prompt-to-proof authoring timing was captured: heldout-first-runs.json's generated_edit_application_to_first_proof_seconds explicitly excludes authoring and is not such a measurement.

Negative controls remove rising-edge guard and execute each H1: all3 now exit1 and reject. Godot's initial assert failure only logged and hung; negative-control-first.json records timeout. Changed Godot to explicit quit(1), reran negative control and restored valid source, then reran all final5x: negative-control.json and heldout-runs.json. This is a useful warning against assuming headless assertion implies nonzero exit.

Defold setup did not reach runtime. Initial Java17 cannot load Bob's Java25 class files. Adoptium redirect download returned403, direct GitHub JRE25 succeeded2.130s. Bob then needed libXext.so.6 despite requested headless build. Downloaded/extracted user-local Ubuntu shared libraries; Bob reached project compile and reported missing default /input/game.input_binding. Stopped bounded setup repairs. defold-build.json retains last actual failure; initial errors are described here (earlier build JSON was overwritten). This is a fixture/setup failure, not proof that Defold cannot run headless. No runtime or Automation Bridge tested.

## Tracking inference

Godot: strongest general engine in this lane to track, broad2D/3D+text GDScript/scenes+CLI headless+exports. Probe only2D and no export/rendering. Core physics explicitly has no determinism guarantee. Arbitrary fixed logic tick control is straightforward; headless by itself supplies neither external state API nor rewind.

Defold: deserves active automation watch, possibly second engine slot if native/mobile2D matters. Official discoverable Editor HTTP API and debug-only Automation Bridge inspect scene/nodes, send input, take screenshots, and support optional app-defined state synchronization. These are vendor tooling; extension is separate from engine core. Sync waits/polling are not arbitrary clock stepping. Documentation specifically disclaims official MCP necessity; community MCP exists but is not vendor-supported. Native extensions may add remote build/toolchain costs; actual setup here is materially harder than Godot/LOVE/Pygame.

LOVE: excellent tiny code-first Lua and explicitly stepped Box2D reference; no scene-editor overhead; browser publishing uses separate love.js port. Keep as lean-loop reference/conditional Lua choice, not prioritize an extra general-engine tracking slot unless Castle/Lua reuse is decisive. Native shipping not tested.

Pygame: strongest when Python integration is the requirement; very easy integer Rect and sprite inspection, but much more game structure and shipping integration remains application work. Do not add active engine tracking slot for it absent Python consumer. Browser routes are external packaging rather than core runtime, untested.

## Primary sources checked

- https://docs.godotengine.org/en/stable/tutorials/editor/command_line_tutorial.html — headless CLI, fixed-fps disables real-time sync, export commands.
- https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_dedicated_servers.html — Godot4 common headless binary, editor/export-template distinction.
- https://docs.godotengine.org/en/stable/tutorials/physics/physics_introduction.html — no deterministic physics guarantee.
- https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_web.html — web-specific deployment limitations.
- https://love2d.org/wiki/love.run — app-owned run loop.
- https://www.love2d.org/wiki/World%3Aupdate — explicit dt advances physics.
- https://love2d.org/wiki/love.physics — Box2D binding.
- https://www.love2d.org/wiki/Game_Distribution — desktop/mobile packaging and love.js browser route.
- https://www.pygame.org/docs/ref/sprite.html — Sprite, Group, spritecollide.
- https://www.pygame.org/docs/ref/time.html — Clock/ticks are real timing; application may own logical ticks.
- https://defold.com/manuals/bob/ — CLI builds and Java25 requirement.
- https://defold.com/manuals/engine-service/ — core debug service versus official extension.
- https://defold.com/manuals/automated-testing/ — headless Bob bundle, app-defined sync and browser testing, separate evidence levels.
- https://defold.com/manuals/ai-agents/ — model-neutral APIs, OpenAPI, official bridge versus community MCP.
- https://github.com/defold/extension-automation-bridge — actual official vendor extension repository.
