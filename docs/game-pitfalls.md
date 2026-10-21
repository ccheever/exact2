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

- **A model fails with ``baked before model records (record 1, no material names)``.**
  Cause: a `.model` from a bake older than material names (LLP 1139.004 D3) still
  loads, as from a copied `assets/`; the engine reads model record 2 and refuses
  another by name. Fix: bake again (any build, or `sim_with_art!` in a test, bakes
  `art/` afresh). The new record changes every baked model's bytes and so a repin's
  `inputs`; world saves and generated models' identities do not change. An empty
  `materials[i].name` then means the glTF material has no name. Select a material by
  index: names may repeat.

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
- **`tap world stick left 0 1` walks the player backward.** The agent's stick is a
  pad's raw axes, as the Gamepad API reports them: positive y is down. `input.stick`
  turns it so positive y is forward. Forward is `tap world stick left 0 -1`. (LLP
  1127.003, 2026-10-18.)
- **A pad's stick moves the player a little short of a key.** Cause: the stick's radial
  dead zone (0.15 unless `Stick::dead_zone` names another) rescales a partial deflection;
  only full deflection reads 1, as a key does. A test comparing a key and a stick drives
  the stick to 1. (LLP 1127.003, 2026-10-18.)

- **Two taps before one tick are one tick's presses, handled in the tick's order.**
  `sim.tap("Digit2"); sim.tap("Digit1"); sim.run(20.)` in a shop where item 2 needs
  item 1 buys both: the tick sees both rising edges and its loop buys 1, which unlocks
  2, then 2. Cause: `pressed` reports every rising edge since the last tick, and a tap
  is down and up before the next one. Fix: in a test or drive, advance the clock
  between presses meant as separate turns; in the game, decide whether a lock is read
  before or after the tick's purchases. ([Shop recipe](../game/recipes/shop.md),
  LLP 1139.001, 2026-10-19.)
- **`clock settle` after a post into a paused world reports `NOT settled (world)`,
  `changing: ["paused","input queued"]`.** Cause: a paused world holds a posted message
  (the page's `postMessage` or the driver's `post`) for its first tick after the pause,
  and a held input counts as work. Fix: resume (`tap pause` in the starter); the first
  tick after it reads the message once. `clock +<ms>` while paused returns with the
  message still held and does not settle.

## Tests, the proof and setup

- **`clock settle` reports rest between a game's scheduled events.** A world whose
  creatures have stopped answers quiescent though a `Spawner`'s next spawn, or hunger
  computed from a saved tick, comes later. Cause: quiescence observes changes and
  declared work, not a game's schedule (LLP 1128 I2). Fix: call `w.busy(reason)` each
  tick such work is pending, as the starter's critter and the
  [spawner recipe](../game/recipes/spawner.md) do. (Starter drive, LLP 1139.001.)
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
- **A phone's world frame rate read in the first seconds after Play is far below its
  steady rate.** The scaffold's starter on a Pixel 5 drew 2.2 fps over the 10 s after
  Play and 16 fps from 20 s on, at the same rung. The renderer makes each pipeline
  variant when it is first drawn; which start-up work dominates was not measured.
  Fix: let the world run 20 s before resetting `perf` and measuring
  (`state … perf_reset`, then `perf frames`). (LLP 1139.003 profile, 2026-10-19.)
- **`exact setup` keeps reporting `MISSING wasm-bindgen` after installing it, and `test web`
  fails with "wasm-bindgen not on PATH".** Cause: setup installs it with `cargo install`,
  whose binaries land in `~/.cargo/bin`, and then looks for it on PATH; a rustup from
  Homebrew puts only `/opt/homebrew/opt/rustup/bin` there, so the install succeeds and the
  check never sees it. Fix: put `~/.cargo/bin` on PATH (`.zprofile`), or symlink
  `~/.cargo/bin/wasm-bindgen` beside `cargo`. **Candidate diagnostic:** setup could check
  `~/.cargo/bin` and say the PATH is the problem. (Fishslop bench setup, 2026-10-17.)
- **A proof reports `no matching rustc unit dep-info` after changing directory spelling.**
  Cargo's cached dep-info records absolute output paths. On macOS, alternating
  `/tmp/game` and `/private/tmp/game` can reuse a unit whose recorded spelling the
  receipt reader refuses. Use the canonical game and target paths from the first
  build (`pwd -P`); if already mixed, rebuild in a fresh private target directory.
  (Fresh `--game --assets` baseline, LLP 1139.000, 2026-10-19.)
