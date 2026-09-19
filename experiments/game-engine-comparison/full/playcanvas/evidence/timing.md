# Timing

- Setup and pinned dependency installation: **1.03 seconds** (`bun install`, cold on the assigned fleet host).
- Production bundle: **0.34 seconds** after warm install (`bun run build`, Rolldown).
- Implementation, browser debugging, and self-verification: **about 22 minutes** wall clock, excluding dependency setup.
- Independent baseline acceptance: **18.72 seconds**, 10/10 cases passed.

The independent time comes from `acceptance/acceptance.json`. It covers title/asset loading, real keyboard input and animation, fixed movement and jumping, pause, wall collision, dynamic crate pushing, persistent reload, the full physical lantern route and victory, timeout/restart, and batch tick equivalence.
