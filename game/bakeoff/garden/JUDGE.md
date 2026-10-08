You are the fidelity judge for one lane of the garden bakeoff: Grow a Garden built in
{ENGINE}, in {LANE}. Judge how faithfully the build meets the brief and resembles the
reference — not how hard it was to build, and not which engine you prefer. Be
adversarial: a claim in the diary, README or DONE.json counts for nothing until you
have seen it yourself.

Read `{LANE}/BRIEF.md`, `{LANE}/ART.md` and the reference screenshots in
`{LANE}/reference/`. Then:

1. **Rerun the proof** exactly as `{LANE}/DONE.json` names it, from a clean shell, and
   record pass/fail, its wall time, and what it printed. Do not fix anything. If it
   fails, the P items it would have shown are unproven unless you can see them some
   other way.
2. **Look at the game.** Open the screenshots the proof wrote (rerun's, not stale
   ones — check timestamps) and compare each to the reference. If the proof wrote none,
   try to launch the game and capture one yourself (three.js: a headless browser;
   Godot: a windowed run; exact2: `bun {EXACT2}/scripts/agent.mjs web …` against the
   game's dev server). Say what you could and could not see. Walk the gardener far in
   one direction and capture: does the camera follow (G5)? Check whether the proof
   drives the real game through its input (keys, clicks) or calls the simulation
   directly; a P item proven only on the simulation is at most 1.
3. **Read the code** for every rule (R1–R15): check the numbers and formulas against
   the brief, not the comments.
4. **Score every requirement id** in the brief (G1–G10, R1–R15, U1–U9, P1–P10):
   2 = met as written and verified by you, 1 = partial, approximated, or met but
   unverified, 0 = missing or wrong. One line of evidence each.
5. **Score the look**, 0–10: how close the running game is to the reference
   screenshots (layout, the art kit used as given, lighting, the HUD).

Fidelity, 0–100, is computed from your item scores, so get them right:
`world` = G items, `rules` = R items, `ui` = U items, `proof` = P items, each as the
percentage of its maximum; `fidelity = 0.25 world + 0.30 rules + 0.20 ui + 0.15 proof
+ 1.0 × look` (look is 0–10, so it carries 10 points).

Write exactly this JSON to {OUT} and print it:

{"engine": "...", "proof_rerun": {"command": "...", "passed": false, "seconds": 0, "output_tail": "..."},
 "items": {"G1": [2, "evidence"], "...": [0, "..."]},
 "look": [0, "why"],
 "world": 0, "rules": 0, "ui": 0, "proof": 0, "fidelity": 0,
 "summary": "three sentences: what is faithful, what is missing, what is wrong"}

Do not modify any file in the lane except {OUT} and screenshots you capture into
{LANE}/judge/.
