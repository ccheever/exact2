# ACP stand-in

A local executable that speaks enough of the Agent Client Protocol (v1: newline-delimited
JSON-RPC on stdio) for Settings › Providers › Add provider to list it under "Or choose from ACP
Registry", add it, and walk its Sign in step: signed out, signing in, signed in, sign out. It
signs in to no account, runs no turns and sends nothing anywhere; its only state is
`state.json` beside it.

## How the server finds it

The reference's catalog (`apps/server/src/provider/acp/AcpRegistrySupport.ts`) fetches the
registry index on every search and falls back to `<T3 home>/caches/acp-registry/registry.json`
when the fetch fails; a binary agent whose command already sits at
`<T3 home>/tools/<id>/<version>/<platform>/<cmd>` is used without a download. `install.mjs`
writes an index with this one agent and puts `agent.mjs` there behind this Bun's shebang (a copy:
the server refuses a command whose real path leaves its install directory). The fetch must fail,
or the real index replaces this one: install into an environment of `../second-environment/`,
which runs offline, never into a home whose server reaches the network.

## Use

From `examples/t3-code/tools/acp-standin`:

```sh
H=../../../../target/t3-fixtures/envs/second/t3home
bun install.mjs install --home $H [--mode agent|terminal|both|none] [--auth-delay <ms>] [--no-logout]
bun ../second-environment/env.mjs start second
bun ../second-environment/env.mjs drive --pair second --storage p1 -- "prefer keyboard-navigation true" \
  "tap toast-dismiss-1" "tap connection-settings" "clock settle" "tap settings-providers" "clock data" \
  "tap add-provider" "clock data" "tap acp-add-fixture-acp-standin" "clock +6000 real" "clock data" \
  "tap create-provider" "clock +8000 real" "clock data" "tree add-provider-dialog"
bun ../second-environment/env.mjs stop second && bun install.mjs forget --home $H   # undo the Add provider
```

Modes: `agent` (default) advertises one sign-in method the server completes with `authenticate`;
`terminal` a login command (`--fixture-login`, Return signs in) the server runs in a terminal;
`none` no method (the no-account state); `--auth-delay` holds `authenticate` so the dialog shows
sign-in in progress; `--no-logout` drops the sign-out capability. `status` reports the mode and
whether it is signed in, `reset` signs it out, `forget` removes the provider instances a drive
added (with their status caches and sign-in confirmations; stop the server first, it keeps its
own copy) and `remove` the index and the executable. A provider instance stays in the server's
settings after a drive, and Add provider then shows the stand-in as added: `forget` before the
next drive.
