# Pitfalls when writing exact2 games

**Adding an entry:** only a pitfall you hit and reproduced on current `main`, written
as symptom → cause → what to do, with where it was found. Delete the entry in the
change that fixes the footgun, or that makes the compiler, runtime or driver
diagnose it. **Candidate diagnostic** marks one that could become a check cheaply.

> **Lookup reference.** Search it by symptom (`grep -n`); don't read it front to back.
> Look a name up first (`bun exact.mjs game vocab <name>`), then the
> [recipes](../game/recipes/README.md). Contract's pitfalls (menus, the HUD, its
> keys, drags and sounds) are in [agent-pitfalls.md](agent-pitfalls.md).

## The world and its saves

- **A socket follower's local pose is `SocketFollow::offset`.** Its `Transform`
  is the fallback when the socket cannot resolve; rotating that fallback does not
  rotate a resolved attachment. Use `SocketFollow.offset.rotation` for the local
  rotation on a working socket.
- **Entity verbs require an entity handle.** Use `World::get_mut`, `require_mut` or
  `nearest_xz_mut`. Consuming query rows and resource guards have no entity identity;
  their verbs report `entity-handle-required`. Borrowed query iteration yields plain
  references, which do not expose these verbs.
- **Append new `Data` enum variants when existing values must keep their hashes.**
  Saves name variants, but the simulation hash includes their declaration ordinals.
  Keep existing ordinals, including `Mesh::Asset` at five, to preserve model-world
  pins when adding primitives.
- **A handle's `tint` and `offset` change simulation state.** They write the entity's
  `Material` and local `Transform`; they save and hash. For a cosmetic flash or bob,
  derive `Tint` or `Offset` in `Game::present` instead. `glow_to` retargets the saved
  `Glow`; keep authored material emission constant or the intensity is applied twice.
  [Game pickup recipe](../game/recipes/pickup.md); [hit flash](../game/recipes/hit-flash.md)
  derives a flash from a saved hit tick.
- **Builder hex colours are sRGB; `Material::rgb` is linear.**
  `prop(...).color("#808080")` converts the channels to about 0.216, not 0.502.
  [Literal builders](../game/recipes/literal-builders.md) and the builder hash test
  use the same conversion as the CSS colour library.
- **Older saves refuse to restore after adding a generated model.** `restore` names it:
  ``asset `…` identity is missing``. Cause: a model registered with `w.generated_model`
  at setup is part of every later save's identity (`game/engine/README.md`, "Saves and
  assets"), even one that only changes how a unit looks. Fix: a model that is only
  appearance belongs in the bake, under `art/` (`Game::STREAMED` if setup need not wait
  for it): a delivered model carries no save identity. (Skirmish's art trials, LLP
  1046.012 F5, 2026-10-07.)

- **`model.materials[i].name` is empty for a material Blender names.** Cause: the
  `.model` was baked before the bake kept material names (LLP 1139.004 D3) and an
  older bake output is still what loads (a copied `assets/`), or the glTF material
  has no `name`. The engine reads an older model
  with empty names rather than refusing it. Fix: bake again (any build, or
  `sim_with_art!` in a test, bakes `art/` afresh). The bake's material record
  changed then, so the next bake moves every model's bytes and a repin's `inputs`;
  world saves and generated models' identities do not change. Select a material
  by index: names may repeat.

## Input

- **A HUD shortcut's chord does not reach the focused world.** On every host a shown
  button's `aria-keyshortcuts` takes its chord (the key with exactly those modifiers)
  before the canvas hears it; a button with `action=` presses that action once, a
  disabled one nothing. While the button shows, an `Actions` binding of the same key
  does not fire for that chord; the key with other modifiers still reaches the world.
  The bake warns (`analyze-shortcut-world-key`); drop the key from `Actions` or the
  shortcut. [Touch controls](../game/recipes/touch-controls.md).
- **Show on-screen controls by `exactViewport().pointer`, never always.** A stick on a
  desktop is a tell. The starter shows its stick and buttons under `coarse` only;
  `prefer pointer coarse` shows them on a desktop drive.
- **A fire button read with `held` never fires on a tap.** `tap world KeyF`, `sim.tap`
  or a quick click does nothing, while holding the key works. Cause: both edges of a
  tap can arrive before the next tick, so the key is never down at a tick; `pressed`
  still reports the rising edge. Fix: fire on `input.pressed(..)` (or `pressed ||
  held` for automatic fire). The same goes for a mouse button bound as `MouseLeft`
  during a fast flick. [Same-tick press](../game/recipes/same-tick-press.md) is the
  doc-test that shows both. (Rivals diary; Fishslop run 1, 2026-10-18.)

## Tests, the proof and setup

- **A continued proof uses its saved source copy.** Keep the SDK unchanged until
  it finishes. Edit the original game freely during a new run; `--continue` refuses
  if its authored inputs differ from the snapshot, naming the file. Start a new
  proof to verify the edit. `tools/` probes and `.exact/status.json` are not game inputs.
  Declare proof-only JSON or other files with `export const proofData = ["relative/path"];`
  so the snapshot copies them and continuation checks them. Resolve SDK files from
  the shared runner’s `proofSDK`, not by walking up from a relocated proof.
- **Stopping `game/dev.mjs` during a build can leave its build child running.**
  A terminated server does not prove its Cargo/build descendants exited. Record
  the process tree before stopping it, then check those PIDs; terminate only
  processes belonging to that invocation before starting another build.
- **`exact.mjs linux` builds the host, not the game's GPU module.** For the
  `gpu-dev` host/module pair, use `bun proof.mjs linux --build-only` from the
  game directory. See [game build profiles](../game/README.md#exact2-integration).
- **`cargo test --test <name>` says `no test target named` for a new
  `logic/tests/<name>.rs`.** Cause: a game without `logic/Cargo.toml` tests through
  the generated `.shells/logic` member, which lists each test file; it is written by a
  bake, not by Cargo. Fix: run `bun exact.mjs test-rust`, which regenerates the member
  first, or `bun exact.mjs shells` before a direct Cargo command in `.shells/`.
  (Fishslop runs 1 and 2, 2026-10-18; LLP 1132.000 F17.)
- **`exact setup` keeps reporting `MISSING wasm-bindgen` after installing it, and `test web`
  fails with "wasm-bindgen not on PATH".** Cause: setup installs it with `cargo install`,
  whose binaries land in `~/.cargo/bin`, and then looks for it on PATH; a rustup from
  Homebrew puts only `/opt/homebrew/opt/rustup/bin` there, so the install succeeds and the
  check never sees it. Fix: put `~/.cargo/bin` on PATH (`.zprofile`), or symlink
  `~/.cargo/bin/wasm-bindgen` beside `cargo`. **Candidate diagnostic:** setup could check
  `~/.cargo/bin` and say the PATH is the problem. (Fishslop bench setup, 2026-10-17.)
