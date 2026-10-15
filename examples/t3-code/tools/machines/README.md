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
bun machines.mjs up                                # starts and seeds second and third
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
