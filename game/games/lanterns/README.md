# Lanterns Exact

A three-minute third-person game built on Exact's game add-on. Move the animated
Fox with WASD, arrows, or the left side of a touch screen. Jump with Space or the
purple button and press E or LIGHT within 1.5 metres of a lantern. Push the crate
to reach the twelfth lantern on the ledge.

From the repository root:

```sh
sh game/games/lanterns/run.sh test
sh game/games/lanterns/run.sh proof
sh game/games/lanterns/run.sh build-web
sh game/games/lanterns/run.sh dev
sh game/games/lanterns/run.sh macos
sh game/games/lanterns/run.sh ios
```

The original Fox.glb from Khronos glTF Sample Models remains unchanged; its
attribution and licence are next to the asset. Game state, Rapier bodies, input,
timer, lanterns, and current animation clip are all included in Exact world saves.

`build-web` also injects the development-only `window.lanterns.command` adapter
used by the independent comparison harness. It is inert unless the URL contains
`?agent=1`; the adapter sends platform input into the real Exact surface, advances
Exact's owned clock, and persists the surface's public save carrier in localStorage.

Initial terrain and the twelve-lantern roster live in `scene.json`; repeated lantern
parts live in `lantern.fragment.json`. Run `bun game/dev.mjs lanterns --scene-only`
after content edits, or add `--reuse-baker` to skip Cargo completely once the typed
baker exists. Rust behavior remains unchanged. Restart uses the new content;
Continue/Restore keeps saved progress. See [typed scene usage](../../scene/README.md)
for parameters, overrides, asset validation and digest-bound development source links.
