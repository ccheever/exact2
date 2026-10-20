# Small game

`bun proof.mjs` verifies gameplay, Contract text/accessibility and saves on the GPU-less Linux host. `bun proof.mjs web` adds pixels. First run `bun /path/to/exact2/game/prove.mjs .` to fill empty pins after every mode and host agrees; accept later intentional changes with `--repin`.

`proof.mjs` opens with engine facts that hold for any game, then one block headed
`// starter: replace these with your game's checks`. The facts: on every GPU host the
renderer reports itself ready and the canvas, less what its own subtree draws over it
(the HUD), is not one cleared colour; when the HUD covers the whole canvas the fact rests
on the renderer's reports alone and the proof says so. And a save restored in a fresh
process continues identically, byte for byte. Keep the facts, replace the starter
block (its "covers most of the canvas" band is the starter scene's own number), and
change `play` if your title screen differs. A proof script
elsewhere in the game, such as `tools/playtest.mjs`, proves the game around it (the
nearest directory above it with `app.json` or `logic/src/lib.rs`), or the one
`--app <dir>` names; it imports `proof.mjs` from the SDK with one more `../` than this
file does.

Rust hostless tests read simulation time with `sim.world().now()` and save with
`sim.save()`. JavaScript app proofs inspect the running host with
`await session.world('world').snapshot()`.

The simulation defaults to 120 Hz: the measured 60 Hz tick-wait p95 was about 17 ms. Choosing 60 Hz is an explicit latency/CPU trade, not the starter default.

The HUD is one structure the screen's shape lays out: a `hud-top` group (what the
player reads at a glance, Pause and Restart), a `hud-bottom` group (the on-screen
stick and buttons, shown under `pointer: coarse`) and a `panel` (what the player
opens on purpose: the controls, the Touch controls setting, a shop). In landscape
the groups sit in the corners and the panel beside the world; when
`exactViewport()`'s `height` exceeds its `width` they become a top bar and a bottom
strip and the panel a sheet behind the bar's Menu button, so nothing covers the
middle third of the world. Add your HUD to these three groups rather than placing
new corner boxes; the proof drives `resize 420x900`, checks the middle third is
clear and taps Touch controls through the sheet.

The canvas binds `world(seed=7, paused=paused, restart=again)` by the names in
`Options`. Reorder these arguments freely; omitted fields use `Options::default()`.
`world()` takes every default; short positional calls take the remaining defaults.
Raw host JSON key order is not a hash input.

Every proof ends with its look: each world drawn offscreen (on this machine's GPU,
even from the GPU-less Linux host), its tells, and `LOOK` paths to open once before
you report. A frame that is near black, near white or clipped, particles that cover
nothing or a shadowed sun with no caster fail the look result, printed beside the
simulation's (`simulation: PASS · look: PASS, 0 tells`); a look the game intends is
silenced by name in `app.json` (`"game": {"look": {"silence": ["near-black"]}}`). A
world with no picture or a measurement not taken leaves the look `UNVERIFIED`. The
proof also shows the scene from above (`look(session, {top: 30}, 'plan')`).
Capture the picture below with `bun proof.mjs web --screenshot-only` at `artifacts/web/game.png`.
This reports `UNVERIFIED` because it skips gameplay checks; `bun proof.mjs web` runs the full browser proof.
The grid, sky-colored height fog, sun shadows, beacon pads and bloom are starter defaults.
Colours are CSS, as in `app.contract`: `Material::hex("#e7aa59")`, `Color::hex("#bad6d1")`;
`Material::rgb` takes linear light, where 0.5 is not mid-grey.
Victory uses the beacon count from setup; extending the Rust scene list needs no Contract edit.
Gameplay reloads preserve the running world. To apply edits inside `setup`, pause
and choose **Restart**; setup runs again and play resumes from the beginning.
`Glow(Tween)` on a `Material::glow` mesh is sampled by the renderer at frame time;
the tick only retargets the tween. The scene steps `Follow` after your tick.
`scene::follow(w)` remains optional when you need to choose an earlier ordering.

![Starter proof capture](artifacts/web/game.png)

The critter sleeps until the player comes within 2.5 m. Awake, it keeps 3 m from a
lit beacon (`steer::flee`), else follows the player and stops 1.2 m short
(`steer::arrive`), else wanders (`steer::wander`). A `Spawner` in it drops a coin
every 2 s, which `collect::drift` brings to the player within 3 m. It starves 40 s
after it last ate; the shop's Feed (1) unlocks when it is half hungry, and Shoes (2)
adds a metre a second to the player, three times. The rows, prices and lock reasons
are published as `shop` and the HUD's buttons hold the same actions as the keys.
While it is awake and fed the critter declares `busy`, so `clock settle` does not
report rest between coins; asleep or starved it does not. At most five coins lie on
the ground; the ones it owes beyond that wait in `Critter.owed`, in order.
`SaveContract` versions the saves: `Game::restorable` refuses a save of another
version by name. Bump it when a save from before a change cannot run after it.
`a_bot_plays_five_minutes_and_the_loop_pays` in `logic/tests/sim.rs` plays five
minutes on three seeds in about a second and checks the critter never starves and
the income pays for all three pairs of shoes: change a price or the income and it
says whether the loop still closes. The SDK's `game/recipes/` (steering, pickup,
spawner, shop) are the text for this code.

One `nearest_xz_mut` result supplies both the lighting write and the HUD name;
`w.count::<Beacon>(|b| b.lit)` supplies the total. The proof checks W against
`d = a*m*(m+1)/(2*h*h) + (n-m)*v/h`, where `m=min(n,floor(v*h/a))`.
Character uses constant acceleration up to its speed limit, so this is the sum
of its semi-implicit velocity steps, with a 1 mm float tolerance. Hash pins still
check the exact saved simulation. Space and Enter on the focused Pause button
activate that button without reaching the world.

This game is the files you write: `logic/src/lib.rs`, `app.contract` and
`proof.mjs`, with `logic/tests/` for hostless tests and the tool-written
`pins.json`. Identity derives from the directory name and `Game::ID`; add an
`app.json` only for keys you author (a title, `game.audio`, a bundle id). The
bake generates everything else, ignored, under `.shells/`. Linux proofs use the
incremental `gpu-dev` profile; web, Apple and deploy retain their production
profiles. `EXACT_GAME_PROOF_PROFILE=release bun proof.mjs` checks Linux against
release without changing the game.

Run the Rust tests, from a fresh clone too, with
`bun /path/to/exact2/game/app/shells.mjs . --test`: it generates `.shells/`
(so a new `logic/tests/*.rs` is a test target at once),
resolves against the SDK's lock offline and locked, and checks the SDK's
determinism lints (exact2's `game/README.md`, "Determinism — the contract").
To depend on another crate, write `logic/Cargo.toml` (`package.workspace =
"../.shells"`, SDK crates as `exact-game.workspace = true`; a game with `art/`
also needs `[dev-dependencies]` `exact-game-bake.workspace = true` for
`sim_with_art!`), then capture
this game's own lock with `bun /path/to/exact2/game/app/shells.mjs . --update-lock`
and commit `Cargo.lock`.
