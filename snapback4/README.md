# Snapback4 for Exact apps

One Snapback4 client for an Exact app's Rust and TypeScript, on every host:
the device (its partition in SQLite, its outbox, local queries and
predictions) and the client protocol over it (opening, sync rounds, store
identity and generation adoption, at-most-once sends with receipts, the
change poll), written once in Rust and performing no I/O. Each HTTP exchange
is handed to whoever drives it. Pinned to Snapback4 **0.4.13**
(`67b2ce28a3`); the npm CLI in the root `package.json` matches.

| | |
|---|---|
| `client/` (`exact-snapback4-client`) | the protocol: `Client` (`read`, `write`, `sync` → `Step::Fetch`/`deliver` → `Step::Done`, `changes`/`changed`, `refresh`/`refreshed`, `outcome`, `status`), the JSON `dispatch`, partition binding |
| `src/` (`exact-snapback4`) | the native device under the app's grants and directories: `Module` (an Exact `NativeModule` or `DataSource`), and `host_request`/`host_reply` for a Rust source's `Answer::Later` |
| `web/` (`exact-snapback4-web`) | the same client as wasm over the device's memory store; `web/build.mjs` builds it with its wasm-bindgen glue |
| `ts/` | the TypeScript driver an app's `app.ts` imports: `Snapback.open`, `read`/`readAll`, `write`, `sync`, `poll`, `refreshSession`, `outcome`, `status` |

## From TypeScript

Mount the driver in the app's `app.json`, and build the wasm (and its glue,
which every host's bake type-checks) before a build:

```json
"typescript": { "sources": { "snapback4": "../exact2/snapback4/ts" } }
```

```sh
node ../exact2/snapback4/web/build.mjs assets/snapback4.wasm   # the web; no argument writes only the glue
```

```ts
import { Snapback } from './snapback4/snapback.ts';
export const grants = `net.fetch ${origin}\nsqlite.open app:/data`;

const db = await Snapback.open({ app: appId, name: 'inbox', origin, viewer: session.principal,
  headers: () => ({ authorization: `Bearer ${session.token}` }), storage, native });
await db.sync();                                       // open on first sync, page, send the outbox
const inbox = await db.read('inbox', {}, now);         // the device answers; pending rows say so
const sent = await db.write('send', { body }, now);    // kept and predicted now, sent by the next sync
if (await db.poll(20)) await db.sync();                // the server's head moved
const renewed = await db.refreshSession(now);          // near expiresAt: keep renewed.session at once
```

A data module has no clock: pass `now` from a source argument
(`exactTime().epochAtZero + now()` in the contract). Natively the device is
the app's native module; link it in the app's Rust:

```rust
struct Snapback(exact_snapback4::Module);
impl exact_js::NativeModule for Snapback {
    fn configure_storage(&mut self, data: PathBuf, cache: PathBuf, temporary: PathBuf) -> Result<(), String> {
        self.0.configure_storage(data, cache, temporary)
    }
    fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> { self.0.call(request) }
}
```

On the web it is the wasm, persisted through Exact SQLite after every call,
one call at a time; a failed save rebuilds the device from the disk and keeps
what the server said. One open partition per page: a second tab finds its
database busy.

Every exchange names itself (`fetch.exchange`); deliver its reply with that
name. A reply for a round that was cancelled, or a client since reopened, is
refused (`done.stale`), as is a superseded or repeated change-poll reply.

`refreshSession` (`POST /auth/refresh`) trades the bearer `headers()` sends
for a fresh session. The server retires the old token before it answers, so
keep the returned session before the next exchange and have `headers()` read
it; `denied.code == "E_AUTH"` means sign in again. When to refresh is the
app's: Snapback's own client asks shortly before `expiresAt`.

## From Rust

```rust
let (client, host) = module.parts();
let client = client.expect("opened");                  // after {op:"open", path, origin, viewer}
let mut step = client.sync(host);
while let Step::Fetch(fetch) = step {
    let outcome = /* the host runs */ host_request(&fetch, origin, &credentials);
    step = client.deliver(host, &fetch.exchange, host_reply(&outcome));
}
```

`tests/client.rs` drives both shapes against a real `snapback4 dev`;
`ts/snapback.test.ts` drives the TypeScript and the wasm the same way.

## Checks

This workspace is outside the root's; its lock carries the private Snapback
source. Run `cargo test -p exact-snapback4 -p exact-snapback4-client`
(`bun install` at the root first, for the server binary), `bun test
snapback4/ts`, and clippy for `exact-snapback4-web` with `--target
wasm32-unknown-unknown`.

What the TypeScript client of Snapback itself does that this one does not
yet: media and assets, native jobs on the device, Following feeds,
ephemeral reads, search state, session refresh, and the 0.2.32 legacy
fallback. `round.rs` names its source in Snapback's `local.ts`.
