# Lanterns playable comparison — common implementation brief

Authorized by Charlie: “ok do full games”, following the four-engine proposal.
This is a diagnostic comparison, not an Exact core implementation or a gate.
Implementers: Codex fleet lanes, 2026-09-17. Build the complete bounded game
below in Godot, Three.js, PlayCanvas and Babylon.js. Same level/asset bytes;
engine-specific implementations, no shared gameplay simulation disguised as
four engines. The existing Exact game worktree belongs to another lane.

## Game

One playable 3D island, twelve lanterns, three-minute dusk countdown, fox.
Player walks/runs, jumps with gravity, collides with solid obstacles and pushes
a dynamic crate. An elevated final lantern is on a ledge: crate then jump to
reach it. A real imported Fox.glb plays Survey/Walk/Run as appropriate (jump may
be procedural). Directional sunset light and shadows; each lit lantern glows.
Use the supplied level.json, Fox.glb and attribution unchanged. No downloaded
art in addition; procedural geometry elsewhere. Ground may be a flat island
with seeded decorative trees/rocks. One signpost explains the crate puzzle.

Start/title, visible controls, live count/time HUD, pause/resume, sound toggle,
save/resume after browser reload/process restart, explicit win and lose screens,
restart. Play with WASD/arrows, Space jump, E act; click/touch buttons for these
actions and start/pause/save/load. No pointer lock required. User can play all
the way through with ordinary controls. Keyboard state clears on blur.

Use real engine collisions/physics or an established integrated physics library,
not scripted coordinates triggered by the acceptance route. Game time and
animation have a 60 Hz simulation step. Rendering is independent. Use a real
pushable dynamic body. Match rules and tolerances, not undocumented bit equality
between different solvers. Keep files <1,500 lines. Pin versions and use Bun;
browser production bundling uses Rolldown. No global installs or fresh clones.

## Level semantics

Right-handed shared world: x right, y up, z toward viewer. Player position in
state is feet position; floor top y=0. `moveX` and `moveZ` are world-relative
[-1,1], normalize diagonal. Constant run speed 4.5 units/s; jump speed 6.4,
gravity 12, radius .35, height 1.3. Native engine axis conversion stays internal.
Lantern positions are interaction targets at feet height, not mesh centers.
Act is press-edge only and lights the nearest unlit lantern within Euclidean
distance 1.5. Final ledge target y=2.4 prevents collecting from below.
Win when all level lanterns lit before duration; otherwise lose at duration.
Pause freezes simulation. Timer duration180s. Falling resets player at spawn
but does not reset elapsed time or lit lanterns. Platform tops match level.

## Common browser inspection/control adapter (development only)

Expose `window.lanterns.command(request)` as an async function returning JSON.
Requests:
- `{op:'ready'}` -> `{ready:true,engine,version}` only once assets ready.
- `{op:'start'}` starts fresh and enters playing; `{op:'reset'}` title, fresh.
- `{op:'input',moveX:0,moveZ:0,jump:false,act:false}` sets held input. Jump/act
  are rising-edge, matching keyboard; unspecified values reset to zero/false.
- `{op:'step',ticks:60}` advances exactly N simulation ticks and returns state.
  `?agent=1` disables wall-clock auto advancing; manual frames may be async in
  Godot. No wall sleeps for simulation; never teleport player for proof.
- `{op:'state'}` -> `{phase:'title'|'playing'|'paused'|'won'|'lost',ticks,
  elapsed,remaining,player:{x,y,z,vx,vy,vz,grounded,animation},
  crate:{x,y,z,vx,vy,vz},lanterns:[{id,x,y,z,lit}],count,total,
  events:[{type,tick,...}],assetReady:true}`.
- `{op:'pause'}` toggles pause. `{op:'save'}` persists actual resumable state
  and returns `{saved:true}`; `{op:'load'}` restores persisted state and returns
  state. Save includes timer, player velocity, crate transform/velocity, lit
  set, phase; release held inputs after load. Test with page/process restart.

Return state derived from the live engine world. No test-only command that
sets collected count, moves bodies, wins or bypasses collision. Logs/events
include lantern-lit, jump, save/load, win/lose; keep bounded. Rendering and HUD
must show that same state. `step` while title/paused/terminal does not advance
gameplay. Human mode automatically advances at60Hz using accumulator.

## Lane deliverable

Source, pinned lock, build/start commands, runnable web output, readable README
with controls and limitations, setup timing separated from implementation,
self-test evidence and screenshots. Preserve source locally in assigned
`full/<engine>/` only. Shared files parent-owned. Run on assigned fleet box in
isolated /tmp/exact-full-games-20260917/<engine>; use prior dependencies where
possible. Record PID at server launch and only stop your own recorded PIDs.
No edits outside assigned subtree, no commits, no spawning. Three fixes per
failure loop, report true blocker if exhausted. Parent writes independent
acceptance and later fresh agent trials; do not anticipate their change tasks.
