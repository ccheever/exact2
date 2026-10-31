#!/usr/bin/env bun
// A machines list for Settings › Connections: "This machine" and two switched-on saved
// environments, which is what the reference lists under Load balancing and GitHub sharing
// (apps/web/src/components/settings/ConnectionsSettings.tsx loadBalancingEnvironments: the
// primary, then every saved environment that is switched on; LoadBalancingSettings.tsx draws
// nothing under two). Both environments hold "shared-notes" (one Git origin), so the composer's
// Run on menu offers both machines for it. README.md has the recipe.
//
//   bun machines.mjs up [--older] [--many]       starts and seeds the second and third environments; --older puts
//                                       the older-server stand-in in front of third (pairings go through it)
//   bun machines.mjs drive [--many] [--storage S] [--json] -- <op> …
//                                       a drive with This machine and both paired, opened at
//                                       Settings › Connections, then the ops
//   bun machines.mjs down [--many]      stops the selected servers
//   bun machines.mjs older <name> [--version V] [--port N]
//                                       the older-server stand-in in front of a running environment (default: on the
//                                       server's port + 40, reporting OLDER.version)
import { spawn } from "node:child_process";
import { openSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { drive } from "../second-environment/env.mjs";
import { checkPort, envDir, fixturePaths, recorded, routeFile, routeOf, seed, startServer, stopServer } from "../second-environment/lib.mjs";

export const MACHINES = ["second", "third"];
/** Seven saved environments plus This machine exercise searchable settings scope menus. */
export const MANY_MACHINES = [...MACHINES, "fourth", "fifth", "sixth", "seventh", "eighth"];
/**
 * The environment `up --older` fronts with the older-server stand-in, its version, and the stand-in's port (the
 * server's + 40). The version is on the client's channel (stable): the update dialog skips a server on another
 * channel ("on the nightly channel", update-everything-logic.ts collectServerUpdateTargets).
 */
export const OLDER = { name: "third", version: "0.1.4", portOffset: 40 };

/** Starts the older-server stand-in in front of a running environment (detached, recorded in its route.json); returns { pid, port }. */
export async function startOlder(paths, server, { name = OLDER.name, version = OLDER.version, port = server.port + OLDER.portOffset } = {}) {
  const running = routeOf(paths, name);
  if (running) return running;
  checkPort(port);
  const log = openSync(join(envDir(paths, name), "older-server.log"), "a");
  const child = spawn(process.execPath, [join(import.meta.dir, "older-server.mjs"), String(port), String(server.port), "--version", version], { stdio: ["ignore", log, log], detached: true });
  child.unref();
  const entry = { pid: child.pid, port, upstream: server.port };
  writeFileSync(routeFile(paths, name), JSON.stringify(entry));
  for (let i = 0; i < 50; i++) {
    try { if ((await (await fetch(`http://127.0.0.1:${port}/.well-known/t3/environment`)).json()).serverVersion === version) return entry; } catch {}
    await Bun.sleep(100);
  }
  throw new Error(`the older-server stand-in did not answer on ${port} (see ${join(envDir(paths, name), "older-server.log")})`);
}

/** From the threads view to Settings › Connections, data landed. */
export const OPEN_CONNECTIONS = ["tap connection-settings", "clock settle", "tap settings-connections", "clock data", "clock settle"];

if (import.meta.main) {
  const command = process.argv[2];
  const paths = fixturePaths();
  const machines = process.argv.includes("--many") ? MANY_MACHINES : MACHINES;
  try {
    if (command === "up") {
      for (const name of machines) {
        const started = await startServer(paths, name);
        await seed(paths, name);
        console.log(`${name}: pid ${started.pid} port ${started.port} label ${JSON.stringify(started.label)}`);
        if (name === OLDER.name && process.argv.includes("--older")) {
          const older = await startOlder(paths, started);
          console.log(`${name}: older-server stand-in pid ${older.pid} port ${older.port}, reporting ${OLDER.version}`);
        }
      }
    } else if (command === "older") {
      const name = process.argv[3], server = recorded(paths, name);
      if (!server) throw new Error(`${name}: no fixture server running`);
      const flag = (key) => { const at = process.argv.indexOf(key); return at >= 0 ? process.argv[at + 1] : undefined; };
      const version = flag("--version") ?? OLDER.version, port = flag("--port");
      const older = await startOlder(paths, server, { name, version, ...(port ? { port: Number(port) } : {}) });
      console.log(`${name}: older-server stand-in pid ${older.pid} port ${older.port}, reporting ${version}`);
    } else if (command === "down") {
      for (const name of machines) { const stopped = await stopServer(paths, name); console.log(`${name}: ${stopped ? `stopped pid ${stopped.pid}` : "not running"}`); }
    } else if (command === "drive") {
      const argv = process.argv.slice(3), split = argv.indexOf("--");
      const options = split >= 0 ? argv.slice(0, split) : argv, ops = split >= 0 ? argv.slice(split + 1) : [];
      process.exitCode = drive(["--local", "--pair", machines.join(","), ...options, "--", ...OPEN_CONNECTIONS, ...ops]);
    } else { console.log(readFileSync(import.meta.path, "utf8").split("\n").slice(1, 19).join("\n")); process.exitCode = command ? 2 : 0; }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
