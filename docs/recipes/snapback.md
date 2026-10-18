# Recipe: a Snapback backend (shared server data)

For shared server data, `bun <exact2>/scripts/exact.mjs new app --snapback
[--port <n>]` from the backend’s directory writes a guestbook and wires its
driver, generated operations and device. The origin defaults to 4400.
`bun exact.mjs snapback` wires an existing app. With an existing schema, the
untouched greeting becomes a typed query and mutation explorer with `--online`.
`snapback --port <n>` records an ignored local backend selection.
At bake, `.exact/backend-origin` overrides `app.ts`'s `origin` constant.
`EXACT_BACKEND_ORIGIN` or `test web --backend <url>` selects the origin and matching
grant at bake time without editing authored source or tests. Password sign-in, stored sessions and refresh with Contract time are
in [the adapter guide](../snapback4/README.md#personas-and-password-accounts).
For a server-only schema, add `--online`; `snapback` and `update` preserve it.
On a plain scaffold, `snapback --port <n>` adds the origin and fetch/storage
grants. A computed origin or grants declaration needs an explicit edit.
After upgrading Snapback4, run `bun exact.mjs snapback --rebuild`.
`bun exact.mjs update` also rebuilds a mounted device after updating Exact.
Custom mounts: add `--artifact-only` to rebuild without rewiring.
`app.json`'s `typescript.sources` are virtual import mounts: `./backend/api.ts`
resolves through `backend` to `../snapback/generated/api.ts`, and
`./snapback4/snapback.ts` resolves through `snapback4` to the selected adapter.
These directories need not exist inside the app. A healthy `snapback --rebuild`
prints the resolved mappings and exits successfully.
The device's embedded `snapback4.engine` section records its identity;
`assets/snapback4.wasm.json` is derived diagnostic output.
`snapback`, `contract build`, `test web` and `agent web` refuse a stale device;
web drives compare it with the running server before opening the page.

The backend selection `.exact/backend-origin` is a build input; a test-only `seed.json` does not
make the build stale. Tests against the backend: [snapback-tests](snapback-tests.md); guests,
personas and password sessions: [snapback-auth](snapback-auth.md).
