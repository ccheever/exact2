# Typed scenes

`exact-game-scene` bakes JSON initial conditions into the existing typed World.
Rust owns component declarations, defaults, serialization and tick behavior;
Contract owns UI. There is no second component schema or expression language.
LLP 1041.006 §7 is the scope.

From the repository root:

```sh
bun game/dev.mjs lanterns --scene-only
bun game/dev.mjs beacons --scene-only
# After the typed baker is built, a content edit needs no Cargo invocation:
bun game/dev.mjs lanterns --scene-only --reuse-baker
```

The first command builds the game's small native `<game>-scene` executable.
Later content edits reuse it; even with Cargo enabled its targets are fresh.
`--reuse-baker` is for JSON edits only: rebuild the baker after changing Rust types,
defaults, behavior or embedded assets. Normal web/native app bakes run the same
step before capturing build inputs. Inspecting an app never runs a bake.

Edit `game/games/lanterns/scene.json` for the ground, ledge, wall, crate or twelve
lantern positions. `lantern.fragment.json` describes each lantern's four entities.
Both Lanterns and Beacons use `games/shared/solid.fragment.json`. Scenes use metres,
Y up, right-handed coordinates and negative Z forward; rotations are XYZW
quaternions, as in `Transform`. Component field names and enum representations are
exactly the existing `Data` JSON codec's:

```json
{
  "units": "metres-y-up",
  "entities": [
    {
      "id": "crate",
      "components": {
        "Transform": {"position": [6, 0.6, 8]},
        "Mesh": {"Box": {"size": [1.2, 1.2, 1.2]}},
        "Material": {"color": [0.45, 0.24, 0.09, 1], "roughness": 0.78}
      }
    },
    {"id": "marker", "parent": "crate", "components": {"Transform": {}}}
  ]
}
```

Omitted record fields use the Rust `Default` value. Enums are one-key objects;
tuple payloads are arrays (`{"Asset":["Fox.glb"]}`), unit variants use `{}`,
and options are zero/one-element arrays. Authoring refuses unknown fields,
unknown variants, wrong scalar types, out-of-range numbers and wrong array/tuple
lengths. Ordinary save decoding retains its existing compatibility rules.
`Types::defaults()` exposes example/default metadata from the Rust `Data` writer;
`Types::component::<T>()` registers the real `Component`/`Data` reader, and
`Types::checked::<T>(fn(&T) -> Result<(), DataError>)` adds domain validation, such
as `Mesh::validate`. Opaque buffers can use
`Types::constructed::<Input, Component>(make, check)` with a Rust `Data` input;
there is no arbitrary evaluator. Components containing runtime entity handles
should use such a boundary; authored hierarchy uses `parent`, never slot numbers.

A fragment has the same `units` and `entities`, plus a `parameters` array of
required names. Inside its component values, `{"$param":"position"}` substitutes
one entire explicitly supplied JSON value. The value is checked by its destination
Rust field reader. An instance names a locally declared fragment:

```json
{
  "id": "crate",
  "fragment": "solid",
  "parameters": {
    "position": [6, 0.6, 8],
    "mesh": {"Box": {"size": [1.2, 1.2, 1.2]}},
    "material": {"color": [0.45, 0.24, 0.09, 1]}
  },
  "overrides": {"": {"Material": {"roughness": 0.78}}}
}
```

Declare the path in `"fragments":{"solid":"../shared/solid.fragment.json"}`.
An empty fragment entity ID is the instance root; other IDs become
`instance/local`, recursively for nested fragments. Local parent names are
qualified the same way; `$root` means this instance's root and `/name` an absolute
scene key. A parent on the instance attaches its root. Forward parents are valid.
Duplicate keys/identities, missing parents/fragments/override targets, parent or
fragment cycles, missing/extra parameters and invalid data fail the bake.

Overrides name local entity IDs and replace **whole components**. In the example,
Material's color becomes its Rust default white. No deep merge, deletion, implicit
inheritance or expressions occur. Entity array order is construction order; stable
authored names survive content reordering, while numeric World slots can change on
a fresh restart. Lantern children are `lantern-1/post`, `/cap`, `/bulb`.

Declare assets as `"assets":{"fox":{"name":"Fox.glb","path":"assets/Fox.glb"}}`
and use `{"$asset":"fox"}` at the string field, for example
`"Mesh":{"Asset":[{"$asset":"fox"}]}`. The baker resolves local files and checks
their SHA-256 against `Game::assets()` bytes; a changed model requires rebuilding
that asset-bearing Rust module. Runtime validation repeats the dependency check.
An asset declaration does not expand the renderer's supported glTF features.

Output goes under ignored `<game>/.scene/`. `scene.contract` exports the pure
`sceneContent()` function imported by app.contract; its value is typed binary data
encoded as hex, passed through the final construction argument `Options.scene`.
It contains no source text or paths. `scene.binhex` is the same data for native
fixtures. Failed validation leaves the last accepted files intact. No JSON source
or generated Contract is a behavior Rust include/build-script input. Only the
app UI shell watches the generated Contract for native rebakes.

In development, `scene.map.json` links authored entity names to declaration,
fragment instance chain and effective component override locations. Diagnostics
include file, line/column and JSON pointer, including the call-site field for a bad
parameter. The sidecar binds its rows to the canonical scene digest and source/asset
file digests. `game/app/scenes.mjs::sceneSource(app, worldDigest, entityName)` refuses
stale content or changed source files. It uses saved `SceneIdentity.digest`, not the
latest requested content. Production bakes remove the sidecar and never package
source paths. Procedural trees/rocks/crates carry `GeneratedBy` with the named Rust
generator and explicit seed/count parameters instead of invented source lines.

A scene is an initial condition. **Restart** uses newly baked content; **Continue**
and **Restore** retain the saved entities, crate pose, lit flags, timer and original
SceneIdentity. Instantiation never patches a running world. Absent registered saved
types, incompatible data, missing assets or changed asset bytes are refused. The
host's Continue binding policy must retain the saved construction argument while
accepting current live inputs (slice C); source lookup explicitly reports when the
world still represents older content. Native builds still require rebuild/relaunch;
this feature does not provide native live code replacement.

Focused checks (no app/server required):

```sh
EXACT_UPDATE_TRUST=development cargo test --manifest-path game/Cargo.toml \
  -p exact-game-scene -p lanterns-logic -p beacons-logic -p exact-game --no-fail-fast
bun test game/app/scenes.test.mjs
```

Tests compare Lanterns' 52 authored entities against the previous Rust constructors
using equal names, handles, parents, transforms and full semantic hashes. They also
cover typed/fragment/asset failures, source spans, opaque construction, refusal
without mutation and runtime progress surviving later scene edits. The build test
edits a lantern in scratch content, checks changed content identity, unchanged native
baker bytes/mtime, zero compiled Cargo targets and failure retention. Those are
simulation and bake proofs; they do not establish GPU rendering or live-host timing.
