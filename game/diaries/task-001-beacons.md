# Task 001 — Beacons

The same brief, given unchanged to a fresh builder for each engine (this one, Godot 4,
three.js). The builder keeps a diary (`README.md` here says what goes in it) and
stops when the proof passes.

## The game

A small 3D game, third person, one screen of play.

- A 40 × 40 m ground plane. The player is a capsule (radius 0.4 m, height 1.8 m)
  starting at the centre.
- **Move** with WASD or the arrow keys, relative to the world's axes, top speed
  4 m/s, reaching it smoothly (no instant starts or stops). **Jump** with Space: a
  hop about 1.2 m high under gravity; no double jump.
- Three **beacons** — spheres 0.5 m in radius floating 1 m up — at (8, 0, 0),
  (−6, 0, 7) and (3, 0, −9). Standing within 1.5 m of a beacon and pressing **E**
  lights it: it glows, and its glow eases in over about half a second rather than
  switching on.
- Six crates (1 m cubes) scattered by a **seed**: the same seed always gives the same
  layout; they are scenery (the player may pass through them in this task).
- A camera follows the player from behind and above, smoothly.
- One sun-like light, plain shading. No textures, no sound.

## The interface

- A title screen with the game's name and a **Play** button; the game itself after.
- While playing: "Beacons N / 3" at the top left, a **Pause** button at the top
  right that stops the world and reads "Resume" while paused. When all three are lit,
  a centred "All beacons lit" with a **Play again** button that restarts the world.
- The interface is real UI — text, buttons, keyboard focus, accessible names — not
  pictures of UI.

## The proof (what "done" means)

An automated script, run from the command line with no human watching, that:

1. starts the game, presses Play, and holds W for exactly 1.5 s of game time, then
   asserts the player's position to within 1 mm of a value the script states;
2. runs the same script twice and shows the two runs end in identical state;
3. walks to the first beacon, presses E, and asserts that the lit count is 1, that the
   HUD reads "Beacons 1 / 3", and that the beacon's glow is partway up 0.1 s after the
   press and fully up 1 s after;
4. pauses, advances 2 s, and asserts nothing moved;
5. saves the game mid-run, restores it in a fresh process, and shows the restored game
   continues exactly as the original did;
6. takes one screenshot of the playing game for a human to glance at.

If the engine cannot do one of these, the diary says which and why, and what the
closest honest substitute was.

## What to hand back

The game; the script; the diary; and three numbers — lines of game code (not counting
the engine), wall time from reading this brief to a passing proof, and how many times
you had to look at pixels to get there.
