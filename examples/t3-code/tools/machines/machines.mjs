#!/usr/bin/env bun
// A machines list for Settings › Connections: "This machine" and two switched-on saved
// environments, which is what the reference lists under Load balancing and GitHub sharing
// (apps/web/src/components/settings/ConnectionsSettings.tsx loadBalancingEnvironments: the
// primary, then every saved environment that is switched on; LoadBalancingSettings.tsx draws
// nothing under two). Both environments hold "shared-notes" (one Git origin), so the composer's
// Run on menu offers both machines for it. README.md has the recipe.
//
//   bun machines.mjs up                 starts and seeds the second and third environments
//   bun machines.mjs drive [--storage S] [--json] -- <op> …
//                                       a drive with This machine and both paired, opened at
//                                       Settings › Connections, then the ops
//   bun machines.mjs down               stops both servers
import { readFileSync } from "node:fs";
import { drive } from "../second-environment/env.mjs";
import { fixturePaths, seed, startServer, stopServer } from "../second-environment/lib.mjs";

export const MACHINES = ["second", "third"];
/** From the threads view to Settings › Connections, data landed. */
export const OPEN_CONNECTIONS = ["tap connection-settings", "clock settle", "tap settings-connections", "clock data", "clock settle"];

if (import.meta.main) {
  const command = process.argv[2];
  const paths = fixturePaths();
  try {
    if (command === "up") {
      for (const name of MACHINES) {
        const started = await startServer(paths, name);
        await seed(paths, name);
        console.log(`${name}: pid ${started.pid} port ${started.port} label ${JSON.stringify(started.label)}`);
      }
    } else if (command === "down") {
      for (const name of MACHINES) { const stopped = await stopServer(paths, name); console.log(`${name}: ${stopped ? `stopped pid ${stopped.pid}` : "not running"}`); }
    } else if (command === "drive") {
      const argv = process.argv.slice(3), split = argv.indexOf("--");
      const options = split >= 0 ? argv.slice(0, split) : argv, ops = split >= 0 ? argv.slice(split + 1) : [];
      process.exitCode = drive(["--local", "--pair", MACHINES.join(","), ...options, "--", ...OPEN_CONNECTIONS, ...ops]);
    } else { console.log(readFileSync(import.meta.path, "utf8").split("\n").slice(1, 15).join("\n")); process.exitCode = command ? 2 : 0; }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
