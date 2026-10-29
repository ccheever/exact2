surfaces: web, ios, android
agent: Claude Code (Claude Opus 5.5)
task: serve a passkey relying party and its association files at a public origin

### Rough

- `scripts/origin.mjs` has no EAS Hosting adapter (LLP 1030.003 D7), so
  `origin.mjs` writes an `expo export` server layout by hand and drives `eas`
  itself. An adapter would let any app's backend routes deploy the same way.
- A worker's isolates share no memory, so the relying party cannot keep its
  passkeys in a `Map` as the fixture server did. Each passkey became an
  HMAC-signed record the app keeps (`secret.keep fixture.passkeys`) and sends
  back, and "forget" became the app dropping a record.
- An earlier run's fixture server from another checkout held port 4331, and the
  fixture's port is written in four files (`app.ts`'s grants among them). The
  port was moved and then moved back; a port the fixture and its drives read
  from one place would have avoided the edit.
- The deploy itself is refused to an agent's session (a production deploy needs
  a person's permission), so the origin waits for Charlie to run
  `bun apps/auth-fixture/origin.mjs`.

### Lean in

- Grants keyed by origin (`docs/reference.md`) switch the whole relying party,
  native RP ID included, with one build input, `EXACT_BACKEND_ORIGIN`, while the
  default build still needs no network under the agent.
- The web bake prints the association entries an RP ID off the app's origin
  needs, which checked `origin.mjs`'s files.

### Needed

- passkeys: provided (`@exact/passkeys`); the relying party is the app's own.
- hosting a backend route: by hand (an `eas deploy` export written by
  `origin.mjs`).

### Checkpoints

- The bundled route module, loaded as two isolates sharing only the key:
  registration across isolates, replay, a tampered record and a wrong origin
  refused.
- Stage 1's Chromium drive against `fixture.mjs` with the signed records.
- Drives 1 to 6 on the web under the agent and on the iOS simulator against
  `fixture.mjs` with the signed records.
