# Beacons — Godot 4.7.2

A small third-person game made entirely from GDScript and a text scene. No editor,
assets, plugins, downloads or engine modifications required.

From this directory:

```sh
GODOT="$HOME/Library/Caches/exact2-game/godot/Godot.app/Contents/MacOS/Godot"
"$GODOT" --path . --log-file "$PWD/artifacts/play.log"
python3 run_proof.py
```

WASD or arrows move along world axes; Space jumps; E lights a nearby beacon.
Tab changes keyboard focus and Enter activates buttons. Pause freezes the world,
including the follow camera and glow. Play again resets the same seeded layout.
Use `-- --seed=123` when launching to choose another crate layout.

The Python script only launches processes and compares bytes. `proof.gd` injects
keyboard events into Godot's Input system (including Enter on focused native
buttons), observes actual 60 Hz physics callbacks, and performs all gameplay
assertions. Headless runs use `--fixed-fps 60`; screenshot mode launches a real
window and captures the viewport after `RenderingServer.frame_post_draw`.

The numerical expectation for holding W is `(0, 0.9, -5.3666666667)` after 90
physics ticks. Horizontal acceleration is 12 m/s², capped at 4 m/s. Gravity is
9.8 m/s²; ground contact is an analytic clamp, not a rigid-body solver. Scenery is
intentionally non-colliding. Emissive materials supply the half-second eased glow;
the Compatibility renderer has no bloom in this project.

`artifacts/summary.json` records the aggregate result; per-run JSON lists assertions,
`.state` files are continuation endpoints, `.final` files are full-script endpoints,
and `midrun.save` is a native binary Variant checkpoint. Saves include velocity,
player and camera transforms, pending actions, tick count, beacon state and crate
transforms. Held keys are external inputs, released at the checkpoint boundary and
reapplied in the same way to both continuations. The script removes stale endpoint
files before comparisons. Process runs are sequential and timeout after 60 seconds.

Current result: proof steps 1–5 pass on this macOS/Godot build. Both full runs have
28 passing assertions, and original/repeated/restored continuation states are
byte-identical. Step 6 is blocked here: all three windowed attempts aborted with
SIGABRT during macOS NSApplication registration, before project startup. The
aggregate script therefore exits 1; no screenshot or visual-quality claim is made.
The startup log also reports denied creation of Godot's default user-data directory;
actual game saves/logs go inside this project and succeed. Those startup diagnostics
are preserved in `.launch.json` and `.stdout.log` instead of hidden.

The diary is [001-beacons-godot.md](001-beacons-godot.md). It remains here because the
instruction to write only inside this project conflicts with the requested diary
path outside it, and no exception was received. No repository files outside this
project were edited. No commits, clones, remote commands or subagents were used.

## Live feel probe

The verification notes above describe the original game proof. From the repo root,
`bun game/bench/feel.mjs godot` now runs a separate visible, vsynced, live-clock
diagnostic three times as shipped and three times with physics interpolation on.
The adjacent `feel.gd` node is inactive unless `-- --feel` is supplied; the runner
also supplies `--feel-config=<schedule.json>`. `--feel-interpolation` enables the
runtime equivalent of `physics/common/physics_interpolation=true`. `main.gd` is
unchanged. See [the benchmark's Feel section](../../../bench/README.md#feel--beacons-live-clock)
for the method, raw traces and numbers. This is engine event-delivery-to-drawn-state
latency, not OS/USB input or scanout, and it does not replace the functional proof.
