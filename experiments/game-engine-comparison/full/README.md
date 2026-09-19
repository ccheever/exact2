# Full Lanterns games: four-engine comparison

Four independent implementations of the same bounded 3D game, following
[the shared brief](BRIEF.md). This is the full-game follow-up to the earlier
microfixtures; it is diagnostic work, not an Exact runtime dependency.

| Engine | Source and build instructions | Physics |
|---|---|---|
| Godot 4.7.2 | [godot](godot/) | CharacterBody3D and RigidBody3D |
| Three.js 0.186.0 | [three](three/) | cannon-es |
| PlayCanvas 2.22.2 | [playcanvas](playcanvas/README.md) | Ammo/Bullet |
| Babylon.js 9.27.0 | [babylon](babylon/README.md) | Havok 1.3.10 |

Each game uses the same [level](shared/level.json) and unchanged animated
[Fox asset and attribution](shared/assets/ATTRIBUTION.md). Collect twelve
lanterns before the three-minute nightfall. Push the crate, jump onto it,
and climb the ledge for the last lantern. WASD/arrows move, Space jumps,
and E lights a nearby lantern. The games include a title, HUD, pause,
sound, touch controls, win/loss, restart and persistent saves.

Build each game in its directory using its README/package scripts. Generated
`dist/`, dependency installations and Godot import caches are ignored by Git.
The source, lockfiles, evaluator, reports and evidence are retained. Raw trial
logs and diffs are losslessly gzip-compressed (`agent.jsonl.gz`, `changes.diff.gz`). Fleet
builds used Bun 1.3.14; the repository pin is Bun 1.3.12. No deployed public
service is required. Browser saves are local to the serving origin.

## Recorded outcome

Babylon and PlayCanvas pass all ten independent gameplay checks. Godot passes
nine, including the physical victory route, and naturally loses in normal
human mode; its large clock request still times out late in the full suite.
Three.js passes seven and retains wall/crate/win failures. These are measured
implementation outcomes, not claims that an engine cannot support the game.

Eight fresh change attempts ran on the two qualified starters. All eight pass
the external functional evaluator; seven comply with the repair budget.
PlayCanvas B1 exceeded the three-repair cap. The eight planned Godot/Three
attempts were not run. See [trial totals](evidence/trials-summary.json).

## Independent verification

[harness/acceptance.mjs](harness/acceptance.mjs) is authored separately from
all four games. It uses each game's `window.lanterns.command` interface and
real browser keyboard events, reads live physics state, and never teleports
or directly marks a lantern collected. Ten aggregated cases cover assets,
keyboard/animation, movement/jump, pause, wall collision, dynamic crate,
persistence across reload, the complete physical winning route, nightfall/
restart, and one batch versus individual fixed ticks. Reports retain failures.

```sh
cd harness
bun install --frozen-lockfile
CHROMIUM=/path/to/chrome bun acceptance.mjs \
  url=http://127.0.0.1:4173/?agent=1 engine=playcanvas out=/tmp/lanterns-proof
```

`?agent=1` disables the ordinary wall clock. It is for evaluation; use the
URL without that query to play normally. Rendering cadence in this mode is
not a shipping frame-rate measurement. Godot accelerates scheduling while
preserving each physics delta at 1/60 second; the browser engines explicitly
step their physics worlds.

[TRIALS.md](TRIALS.md) fixes two change requests, two fresh common-model
attempts per engine, a 15-minute agent ceiling, and an external evaluator.
A failed baseline is ineligible for warmed change trials; do not count an
unrun attempt as either a successful trial or an observed model failure.
Evaluator repairs apply to every affected engine. Agent-written self-tests
are supplementary to external evaluation.

[harness/measure.mjs](harness/measure.mjs) records five fresh Chromium
launches, package bytes/hashes, readiness, paint opportunities, frame
intervals and memory proxies. SwiftShader is software rendering; these
numbers do not measure phone/GPU performance, completed GPU frames, native
installed size, or cold filesystem startup. Source maps are itemized and
excluded from the payload subtotal. No cross-host deterministic equivalence
is claimed.

Current local previews: [PlayCanvas](http://127.0.0.1:4781/),
[Babylon](http://127.0.0.1:4782/), [Godot](http://127.0.0.1:4783/).
These are local development servers, not public deployments. Godot has a
known large-step automation failure; Three.js has unresolved collision failures.

Final outcomes and tracking recommendations belong in
[LLP 1041.003](../../../llp/1041.003-game-engines-for-agents.research.md).
