surfaces: web, linux
agent: Claude Code (Opus 5.5)
task: an OAuth sign-in that stays signed in, against a fake provider with several providers' behaviour

### Rough

- A native answer let go mid-refresh lost the refresh's reply and read the store as empty: a rotated refresh token was spent and its replacement lost, which signed the person out. Fixed in exact2 (LLP 1146): a request with effects runs to its end when its answer is let go, and what its reply's turn writes stands; a store read with no store now throws instead of reading as "signed out".
- A package exporting `index.ts` failed the build's type check: the bake stages only a package's `.d.ts` files for the checker. `@exact/oauth` ships `index.d.ts` beside its code. A `file:` dependency is a copy made at install, so a changed package needs `bun install` again (about 10 minutes to find).
- A data module's store write between commits waited for an unrelated commit on the web, and a refused commit cleared its dirty flag, so the screen stayed signed in after the provider ended the session. Fixed in exact2: such a write schedules a commit of its own.
- The Linux host refused sign-in with 501 even under the agent, so no headless native drive could sign in. Fixed in exact2: under the agent a sign-in is held on every host.
- A state the tests choose before a relaunch (`profile`) reset at relaunch; `persist` fixed it.
- `app.json` needs `"deploy": {"store": …}` for the Linux binary to start; without it the binary exits naming "the delivery adapter".

### Lean in

- Authored tests that drive the app against the app's own server (`commands.server`, `request POST "/control"`) made every provider behaviour a one-line setup.
- The Linux host runs the native executor headless in about 8 seconds a file; it caught what the web could not.

### Needed

- sign-in: provided (`openAuthSession`), by hand for the token half until `@exact/oauth`
- secure storage: provided (`secret.keep`)

### Checkpoints

- 2026-10-26: 73 cases (9 profiles) pass on the Linux host, and all but GitHub's on the web, whose token endpoint answers no browser (GH-8). Without the lock, the Linux host fails case 5 with a reused refresh token, as the reported sign-outs did.
- 2026-10-27: the lock is gone. A let-go answer's POST now runs to its end natively, as a browser lets it, and its continuation keeps what it writes, so `@exact/oauth` serialises its work with a plain promise and nothing else changed for the app. Every case passes on the Linux host, and all but GitHub's on the web; with no let-go request kept, the Linux host fails case 5 with a reused refresh token again. Case 5 now also asserts that each renewal happened, which a session stuck on a renewal that never landed had passed.
