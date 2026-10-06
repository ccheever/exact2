The serial runtime owner + env()/keyboard lane is parked on `parked/runtime-owner`
(2583be3, base 17350d0 — pre-fonts) and the main worktree is clean of it. LLP 1022
holds the measurements (fails the macOS smoke ~6 of 8; macOS boot +26 ms for a
shell frame; iOS −27 ms from the prepare overlap), the four cherry-picks worth
taking without the owner thread, and the conditions for reviving it. A revival
rebases across 43b0c0c and lands only through a green smoke.

*Filed under “Later”.*
