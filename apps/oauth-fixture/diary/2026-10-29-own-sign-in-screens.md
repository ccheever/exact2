surfaces: web, linux, macos, ios
agent: Claude Code (Opus 5.5)
task: a sign-in through the app's own screens (ForgeRock's tree), then the authorization request the app sends with the tree's SSO token

### Rough

- On the web the app's authorization POST cannot work: a browser answers `redirect: "manual"` with an opaque redirect, no `Location`, and drops the `Cookie` the app sets. Natively both work, so the case runs `on linux, macos, ios` and the web skips it, saying so; a web build would send that request from a server.
- The tree's SSO token signs in as the person, so it stays in the data module: a source that answered it to Contract would leave it in an answer, which is kept and shown.

### Lean in

- `@exact/oauth`'s `authorize` took the app's own request and did the rest (PKCE, `state`, the exchange, the ID token's checks), so the own-screens sign-in is one source and one button.
- `on linux, macos, ios` on a test says what only native hosts can do, and the web run reports the skip.

### Needed

- sign-in with the app's own screens: provided (`signIn`'s `authorize`, LLP 1146 D8)

### Checkpoints

- 2026-10-29: `app.test.contract`'s own-screens case passes on the Linux host, macOS and the iOS simulator, and the web skips it; the ForgeRock and spec cases pass on the web and the Linux host.
