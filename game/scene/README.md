# Typed scenes

`exact-game-scene` bakes JSON initial conditions into the existing typed World.
Rust owns component declarations, defaults, serialization and tick behavior;
Contract owns UI. There is no second component schema or expression language.
LLP 1041.006 §7 is the scope.

From the repository root:

```sh
bun game/dev.mjs lanterns --scene-only
# After the typed baker is built, a content edit needs no Cargo invocation:
bun game/dev.mjs lanterns --scene-only --reuse-baker
```

The first command builds the game's small native `<game>-scene` executable.
Later content edits reuse it; even with Cargo enabled its targets are fresh.
`--reuse-baker` is for JSON edits only: rebuild the baker after changing Rust types,
defaults, behavior or baked assets. Normal web/native app bakes run the same
step before capturing build inputs. Inspecting an app never runs a bake.

Edit `game/games/lanterns/scene.json` for the ground, ledge, wall, crate or twelve
lantern positions. `lantern.fragment.json` describes each lantern's four entities.
Lanterns uses `games/shared/solid.fragment.json`. Scenes use metres,
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
tuple payloads are arrays (`{"Asset":["fox.model"]}`), unit variants use `{}`,
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

Declare assets as `"assets":{"fox":{"name":"fox.model","path":"assets/fox.model"}}`
and use `{"$asset":"fox"}` at the string field, for example
`"Mesh":{"Asset":[{"$asset":"fox"}]}`. The baker resolves local files and checks
their SHA-256 against scene-owned `Asset::new(name, digest)` descriptors from the
baked asset manifest; the game declares `Game::ASSETS`, and the ordinary art bake
produces the model and textures. Changed assets require rebuilding the behavior module. Runtime validation repeats the dependency check.
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

A scene is an initial condition. Restart instantiates current content. Carry restore
first decodes the saved typed world, then applies the three-way authored merge:
unchanged saved fields take new defaults, runtime edits survive, and the bounded
reload report names applied/kept/added/removed/unmatched paths. Open restore retains
saved declarations exactly with current bindings. `SceneIdentity` participates in
the same merge; source lookup refuses a digest that does not match the sidecar.
Native code changes still require rebuild/relaunch.

Admission limits are 100,000 entities, 256 parent levels, 32 fragment levels and
64 JSON levels. A source/asset read is at most 16 MiB; all reads, including repeated
fragments and final race checks, share 64 MiB. Expansion/cloning shares 256 MiB of
charged source work. Binary components share a 64 MiB decode allocation allowance.
Exceeding a limit refuses before publishing output. Parent validation is bounded
by entities × 256; typed components retain their own domain bounds.

Focused checks:

```sh
cargo test --manifest-path game/Cargo.toml -p exact-game-scene
bun game/app/shells.mjs game/games/lanterns
cargo test --manifest-path game/games/lanterns/.shells/Cargo.toml -p lanterns-logic
bun test ./game/app/scenes.test.mjs
```

Scenes use EXSCENE v1 and the combined runtime uses EXSIM v7. Older simulation
saves are refused deterministically; there is no migration. Tests cover typed data,
fragments, asset bytes, source spans, shared work limits, failed-bake retention and
runtime progress through changed authored content. The build test measures a real
content-only rebake with unchanged executable bytes and zero compiled Cargo targets.
