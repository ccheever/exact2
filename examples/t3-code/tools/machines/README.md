# Machines list

Settings › Connections lists machines under Load balancing and GitHub sharing once there are
two: the reference's `loadBalancingEnvironments` is the primary ("This machine") followed by every
saved environment that is switched on (`apps/web/src/components/settings/ConnectionsSettings.tsx`;
`LoadBalancingSettings.tsx` draws nothing under two). No server endpoint or record makes a
machine; pairing does. This fixture is "This machine" plus the `second` and `third` environments
of `../second-environment/` (its README has the isolation, ports and paths), so the list has three
rows, each with its load-preference and GitHub-routing menus. Both environments hold
`shared-notes` under one Git origin, so a new thread there offers both in the composer's Run on
menu.

## Use

From `examples/t3-code/tools/machines`, with the runtime staged and the bundle built:

```sh
bun machines.mjs up [--older]                      # starts and seeds second and third
bun machines.mjs drive --storage m1 -- "tap load-balancing-switch" "clock data" "tap load-balancing-toggle" \
  "type load-weight-<second id> key Enter" "tree load-balancing"
bun machines.mjs down
```

`drive` pairs both environments with This machine on, finishes the welcome wizard and opens
Settings › Connections; your ops follow. `bun ../second-environment/env.mjs status` prints the
environment ids the row and menu targets carry (`machine-<id>`, `load-weight-<id>`,
`load-weight-<id>-option-<25|50|100|0>`, `github-routing-<id>`). The load-preference menus are
disabled until Load balancing is switched on; the switch and the menus' choices last only for
that drive's store.

For the Run on menu: open a `shared-notes` thread (`tap thread-fleet:<id>:<thread id>`), `tap
new-thread`, `type palette-input key Enter` (the project), `tap workspace-mode`; the machine
rows are `run-on-<environment id>`.

## A server older than the client

`bun machines.mjs up --older` also puts the older-server stand-in (`older-server.mjs`) in front of
`third`, on its port + 40 (16551), and the drive pairs `third` through it. Everything passes
through except the environment descriptor's version, which reads `0.0.46-nightly.20261001.1`
(older than the client's `CLIENT_VERSION`, `version-skew.ts`) with the capabilities of a server
under T3's service launcher (`serverSelfUpdate: "boot-service"`, with progress), and the update
request, which the stand-in answers itself as the reference streams it: "downloading" (6 s),
"installing" (3 s), "complete". The descriptor then keeps the old version for 10 s, the
"restart", then reports the client's version and closes the app's sockets, so it reconnects to an
up-to-date server. Nothing is downloaded or installed, and the real server never sees the request.

So the clone offers the update and shows its busy states: Settings › Connections' row, and, with
Load balancing on and a new thread on `shared-notes` that is balanced across both machines, the
composer's Auto balance banner "Update available for 1 machine", whose title opens the machines
popover (`notice-machines-<id>`) with its Update; while it runs the rows read "Downloading…" and
then "Restarting…". The stand-in updates once: `down` and `up --older` again for another run. Its
log is `envs/third/older-server.log`.
