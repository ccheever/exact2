# Lanterns — Three.js implementation attempt

Three.js0.186.0 plus cannon-es, the shared animated Fox and Lanterns level.
This implementation builds and renders, but is **not accepted as a complete
working game**: independent verification failed wall collision, meaningful
crate pushing and the physical winning route. The other seven cases passed.
See [the retained report](../evidence/three-round1/acceptance.json).

```sh
bun install --frozen-lockfile
bun run build
bun run start
```

Use the ordinary URL to play; `?agent=1` exposes explicit60Hz advancement.
Production output lives in ignored `dist/`. No commit or public deployment was
made. The implementation was stopped after its physics repair budget was
exhausted. Do not treat its size or startup as those of an equivalent accepted
full game, or silently admit it to warmed change trials.

The user subsequently authorized one additional sleep/reset repair round.
That rebuilt version also scored7/10 with the same three failures. The unproven
patch was reverted; `../evidence/three-authorized-extra-round/` preserves the
patch, tested source hash and full report. No further repair was attempted.
