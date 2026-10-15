# Second environment

Extra T3 environments for drives of the clone: lane T3 servers (the official runtime the app
stages, `server-runtime/`), each on its own homes under `target/`, seeded with projects and
threads through the server's own API, and paired into the app as saved environments. With one
paired the app has a background environment (the legacy sidebar's background rows and drops,
Settings › Projects for a project only a background environment has, its Mark unread); with
two, and "This machine", Settings › Connections has a machines list (`../machines/`).

## What it isolates

A lane server's environment is built from nothing (`serverEnv`): `HOME`, `CODEX_HOME`,
`CLAUDE_CONFIG_DIR`, `XDG_*`, `T3CODE_HOME`, `TMPDIR` under its own directory, telemetry off,
none of the calling shell's tokens. Every built-in provider's binary is pinned to
`/nonexistent/t3-fixture/<id>` in its settings. Its HTTP(S) goes to a proxy on a closed loopback
port (`T3_FIXTURE_OFFLINE_PORT`, default 16599; Node's fetch under `NODE_USE_ENV_PROXY=1`, Git and
curl by the variables), so it reaches no provider, no registry and no GitHub; a start refuses
while anything listens there. A sandbox profile cannot do this: the server runs the setuid
`/bin/ps`, which a sandboxed process may not. A shim first on its `PATH` answers the server's
`scutil --get ComputerName` with the lane's label, so environments read "Lane Second" and
"Lane Third", not this Mac's name.

Ports are 16500-16599 (`T3_FIXTURE_PORTS=lo-hi` names another lane's range; 3773 never):
"This machine" 16500, `second` 16510, `third` 16511. Everything is written under
`T3_FIXTURE_ROOT` (default `<checkout>/target/t3-fixtures`): `runtime/` (the unpacked release),
`envs/<name>/` (homes, `repos/`, `server.log`, `server.pid`, `environment.json`), `local/` (This
machine's home for `--local` drives), `drives/`.

## Use

Stage the runtime and build the bundle first (the app README's Build and run). From
`examples/t3-code/tools/second-environment`:

```sh
bun env.mjs start second && bun env.mjs seed second
bun env.mjs status                       # pid, port, label; projects and threads with their ids
bun env.mjs drive --local --pair second --storage s1 -- "tree sidebar" "screenshot /abs/out.png window"
bun env.mjs stop all
```

The seed: `shared-notes` in both environments (one Git origin, so the clone groups it as one
logical project) with a thread each, and in `second` only `background-only` with "Background
thread (no run)" and "Background follow-up". Threads are created with `thread.create` and never
get a turn. `seed second --turns` adds "Background thread (completed turn)": the ACP stand-in
(`../acp-standin/`) becomes a provider of that server and answers one prompt, so the thread holds
a completed turn, and the seed visits it at its completion, so it starts read. Run again, the seed
skips a thread that has its turn and gives one to a thread an interrupted seed left without.

`drive` runs `scripts/agent.mjs macos` on the newest bundle (`EXACT_MAC_BIN` overrides) at
1280x840. In a fresh store the app opens its welcome wizard: the drive pairs each `--pair`
environment there (a fresh single-use token each; the driver's echo of it is redacted), then
finishes the wizard with "Do not import projects", and runs your ops. `--local` also starts
"This machine" on the fixture's local home and port, with that home's provider directories and
the offline proxy, so its wizard offers no history of this Mac. `--keep-welcome` stops before
finishing the wizard. A drive waits 5 s for the wizard, 20 s while the local home has not unpacked
the pinned runtime yet (its first start). Agent runs keep pairings and credentials in memory, so every drive pairs
again and nothing reaches the Keychain. A background row's thread is
`thread-fleet:<environment id>:<thread id>` (ids from `status`).

Another lane's ports: `T3_FIXTURE_PORTS=16300-16399 T3_FIXTURE_OFFLINE_PORT=16399
T3_FIXTURE_LOCAL_PORT=16300`, and `start <name> --port 16310`.

### Mark unread on a completed thread

With `seed second --turns`, "Background thread (completed turn)" is read: its legacy row shows no
status. Mark unread (its context menu) rewinds the server's visit watermark to just before the
completion (`thread.mark-unread`), and the row shows the reference's green "Completed" status
(LegacySidebar.tsx `resolveThreadStatusPill`, Sidebar.logic.ts `hasUnseenCompletion`); opening
the thread reads it again. The menu needs real input (below); the row's answer to the command can
be driven by sending it through the server's API while the app runs.

What an agent drive cannot do here: choose an item of a native context menu (under the agent it
answers dismissed: nothing is chosen and its items are not logged), or drop a file on a sidebar row (the row takes drops
through a native hatch, not a Contract `drop`). Those rows need real input.

## Real input (a normal launch)

A normal launch saves the pairing in the Keychain (service `com.exact.t3code.macos.access-token`,
account `<origin>\n<environment id>`) and the saved list in the app's defaults. Launch a lane copy
of the bundle (its own bundle id) with `T3_LOCAL_HOME=<root>/local/t3home T3_LOCAL_PORT=16500`,
pair with a URL from `bun -e 'import {fixturePaths,pairingUrl} from "./lib.mjs"; await Bun.write("/abs/pairing-url", pairingUrl(fixturePaths(), "second"))'`
(paste it from that file), and afterwards remove the token with `bun env.mjs forget second`
(only that fixture account; "none saved" when there was none).
