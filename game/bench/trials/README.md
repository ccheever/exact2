# Lanterns headless change trials

Parent-owned instrument, version 3. `createCommand({root, store?})` returns
`{command(request), reload(), tree(), screenshot(path), close(), unavailable,
limitations}`. Requests are BRIEF's ready/start/reset/input/step/state/pause/save/load.
`step` uses the selected ref's `s.world('world').ticks(N)`, verifies the exact
world tick delta, and leaves title/paused/terminal states stationary. State comes
from `state world:*`, world resources/publications and bounded logs. Missing data
is null, listed in reports. Feet are the physics capsule center minus 0.65m.

Linux cannot hold pointer contacts. Input vectors are explicitly quantized to the
nearest of eight ordinary keyboard directions. This is a contract deviation;
continuous analog input cannot be implemented on this carrier without a host
change, forbidden by I1. World pixels are unavailable; CPU screenshots show UI
and flat canvas. Cases requiring screenshots are unsupported, never pass, even
when their mechanics pass. Asset readiness means the world declares the Fox
asset and animation, not that a GPU uploaded it. Save/load uses the existing
world save carrier plus a sidecar for the UI's live title/pause binding, then
restarts the entire host and releases held input. Linux's UI save button remained
at “Saving…” in the probe, so it is not used to claim checkpoint persistence.

Baseline case bodies come from comparison-full/harness/acceptance.mjs. The
transport changes only: native driver keys, process reload, CPU screenshot
classification. The movement helper and case assertions are retained. Task
routes reuse the comparison's ordinary input route. No teleport or test mutation.

Run from the repository root (Bun and Cargo on PATH, development update trust):

```
bun game/bench/trials/build.mjs
bun game/bench/trials/acceptance.mjs
bun game/bench/trials/task-a.mjs
bun game/bench/trials/task-b.mjs
bun game/bench/trials/run-trial.mjs --ref next/baseline a 1
```

Evaluators accept `root=/archive` and `out=/evidence`. Reports distinguish failures
from unsupported checks and exit nonzero for either. `functionalPassed` on task
reports excludes only unavailable rendering. This is not a full comparison pass.
The baseline currently has a direct crate-route failure at the sign; it is
retained, not repaired by changing the game or evaluator's route.

The original request named next/trunk; the later common rule says commit on the
current branch. This clone is next/i1, so increments land there. No engine, host
or game changes are authorized. Parent construction used three adapter rounds:
(1) rejected pointer contact, (2) keyboard mapping and discovery that UI save
stays pending, (3) existing world-save carrier. Baseline records and both negative
controls are committed before any model attempt.

`run-trial.mjs --ref REF <a|b> ATTEMPT` archives the resolved commit into
`~/lanes/gamenext/scratch/trials/REF/TASK-ATTEMPT/workspace`, copies the local warm
Cargo target (no shared writable target), and builds before dispatch. The existing
read-only `../ibex` dependency is linked beside the archive. Setup/build/control
times are outside agent time. Same-ref controls run before every dispatch. The
trial sees only its game brief/change request, README and source workspace as
installed dependencies; inherited instructions, research, earlier trial sources,
history and evaluator are absent. Linux Landlock denies reading the parent
instrument and other attempts, verified by a real read-denial probe. The model
may edit only its game; the parent reports changes outside that directory as
scope violations. The entire source/build/auth copy is removed after evaluation.

Wall timeout records the CLI PID and descendant PID/start-time identities before
signalling them. Agent JSONL, stderr, command inventory, prompt, source diff and
hashes, timestamps and reports survive. A shell command containing multiple
build/proof invocations is one matching command record; proof-internal builds
are excluded. Repair counts and screenshot inspections require retained-log
audit and remain null until audited. Rendering unavailability means a strict
`passed` can never be true on this machine; functional task results are separate.

Runner setup correction 1: git archive did not include the sibling ibex path
dependency. The failed pre-dispatch setup is retained separately; no model was
launched and no timed attempt was consumed. Link the authorized read-only
sibling in scratch and permit read-only dependency access in Landlock.

Runner setup correction 2: the root workspace's filesystem helper also resolves
`snapback-sb4` even for a game bake. Preserve that existing sibling path as another
read-only dependency link. Its sources are not modified; all output remains in
scratch. Both failed setup runs were before model dispatch, outside agent timing.

Runner setup correction 3 (final repair in this loop): copied Cargo dep-info
retained absolute generated-source paths into the original checkout, which the
build receipt correctly refused. Invalidate copied Cargo fingerprints and warm
at the new archive path before dispatch. This can recompile dependencies during
setup; no setup time is charged to the model. If this repair fails, do not add a
fourth repair or manufacture timed trial results.

A1 launch audit found `/etc/resolv.conf` resolves to
`/run/systemd/resolve/stub-resolv.conf`; the first Landlock policy denied reading
that symlink target. A1 is retained, never replaced or timed from a reset point.
Subsequent attempts permit the resolver's exact real path and probe that it is
readable alongside evaluator denial. This is an instrument defect, not evidence
about Lanterns authoring difficulty. Agent parameters and evaluator remain fixed.

A2's live log revealed that the archive still exposed other games in the source
workspace: it searched `game/games` for existing proof examples. This exceeds
“only the game” context, so A2 must carry that protocol deviation. Later archives
remove other game consumers, retaining only Lanterns and its shared scene
fragments. Engine/path dependencies remain source-available for Rust compilation.
No session is restarted or supplied feedback as a result of this audit.

A2 also exposed a second isolation omission: the directory-capability filesystem
helper opens `/` before walking to authorized assets. Landlock denied that open,
so the model's Linux proof bake failed with EACCES even though parent setup built.
Future policy grants READ_DIR (directory listing only) at `/`; READ_FILE remains
restricted and the evaluator-content denial probe remains mandatory. A2 keeps
this instrument-induced time loss and is not a clean cross-engine measurement.

Final launcher repair round: A2's log also showed Git failing to read the global
`.gitconfig` and the installed `rg` shim denied execution. Future sessions use
`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, and read/execute permission
for the installed rg directory. This adds no project/history access. These
instrument failures are quoted separately from game/API difficulties. No fourth
launcher repair is authorized in this run.

The first B1 context-trim setup failed before dispatch: the renderer declares
other games as dev-dependencies, and Cargo requires their manifests even for a
release Lanterns build. Context-packaging repair 1 retains those original
manifests with empty library targets, without example/game implementation bytes.
These dev-only crates are not compiled by the release Lanterns build or its
package-scoped tests. Lanterns and the engine dependency sources remain at the
selected ref. The failed pre-dispatch setup is retained separately. This is a
new packaging failure introduced by context trimming, not a fourth repair of the
previously resolved target relocation or CLI-access failure loops.

Version 2 corrects event classification globally: “all lanterns lit” is an
observation, not necessarily victory after Task B. Win events come from the
world journal's `publish phase: ... "won"` entry and retain its actual tick.
The correction is included in the final replay of the four unchanged submissions
and baseline; original timing and version-1 evaluation files remain available. No model is
restarted and no requirement is relaxed.

Version 3 also releases every supported key through existing key-up operations
after load. A focused probe saved while W was held and observed another 0.75m of
movement after load; the corrected probe observes no movement and no held keys.
All unchanged candidates receive the final version-3 evaluation, with no model
restarts. The version-2 A1 intermediate evaluation is retained separately.

## I2 interleaved after run

The runner now loads `prompt-a.txt` / `prompt-b.txt` from this directory. These
are byte-for-byte copies of the original retained baseline prompts (both
attempts of each task agree), eliminating reads from another lane. SHA-256:
A `94682fced5fc4c9e3c0c6036976dc049c09a3713f541821d7cd8e8a688f431a7`;
B `5bdd23dd35416be1f5b5c18847b87fc27995b8e6d8d218e48e3a2f6fd71fe75a`.
The scratch destination is `~/lanes/gamenext/scratch/I2/trials/`; original
attempts remain untouched. Pin after to `7c65726`, because `next/trunk` has
moved. The sequence is after A1, baseline A3, after B1, baseline B3, after A2,
after B2. Evaluator version 3 and all six evaluator hashes remain unchanged.
The 900-second dispatch ceiling, three-repair prompt, same-ref missing-feature
negative controls, isolation read-denial probe and cleanup remain in force.
