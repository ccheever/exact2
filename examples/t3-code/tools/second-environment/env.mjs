#!/usr/bin/env bun
// Extra T3 environments for drives of the clone. README.md has the recipe.
//
//   bun env.mjs start <name> [--port N]   an offline lane server for second | third (16510, 16511)
//   bun env.mjs seed <name> [--turns]     its projects and threads (idempotent); --turns adds the
//                                         threads with a completed turn, answered by the ACP stand-in
//   bun env.mjs status                    each environment: pid, port, id, label, projects and threads with ids
//   bun env.mjs stop <name|all>           stops the servers this fixture started (recorded pids only)
//   bun env.mjs drive [--local] [--pair second,third] [--keep-welcome] [--storage S] [--size WxH] [--json] -- <op> …
//                                         an agent drive of the bundle: with --local "This machine" too,
//                                         each named environment paired in the welcome wizard (fresh
//                                         tokens, kept off the terminal), the wizard finished, then the ops
//   bun env.mjs forget <name>             removes the Keychain token a normal launch saved for it
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { ENVIRONMENTS, LOCAL_PORT, offlineEnv, checkPort, checkout, envDir, example, fixturePaths, forgetKeychain, localHome, pairingUrl, welcomeWait, recorded, seed, shellOf, startServer, stopServer } from "./lib.mjs";

const paths = fixturePaths();
const nameArg = (index = 3) => {
  const name = process.argv[index];
  if (!ENVIRONMENTS[name]) throw new Error(`name an environment: ${Object.keys(ENVIRONMENTS).join(" | ")}`);
  return name;
};
const flag = (argv, name) => { const at = argv.indexOf(name); return at >= 0 ? argv[at + 1] : undefined; };

/**
 * The newest bundle's executable (`build.mjs t3-code-macos --bundle` writes it under
 * target/clients/<hash>/): the dev binary carries no embedded server.
 */
export function bundleBinary() {
  const clients = join(example, "target/clients");
  const found = (existsSync(clients) ? readdirSync(clients) : [])
    .map((hash) => join(clients, hash, "com.exact.t3code.macos/macos/T3 Code (Exact).app/Contents/MacOS/T3 Code (Exact)"))
    .filter((file) => existsSync(file)).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return found[0] ?? "";
}

/**
 * The ops that pair environments in a fresh store: the welcome wizard's Connect step opens with
 * its pairing link field (the first pairing); "Add a computer" opens it again for the next.
 */
export const pairOps = (url, index) => [...(index > 0 ? ["tap welcome-add-computer"] : []), `type welcome-pairing-link ${url}`, "tap welcome-pair", "clock +4000 real", "clock data"];

/** Through the wizard's Agents step, then "Do not import projects": the drive starts at the threads view. */
export const FINISH_WELCOME = ["tap welcome-continue", "clock settle", "tap welcome-agents-continue", "clock settle", "tap welcome-skip-import", "clock data", "clock settle"];

/** The driver echoes a typed value; a pairing token never reaches the terminal or a log. */
export const redact = (text) => text.replace(/#token=[^\s"'\\]+/g, "#token=<redacted>");

/** Runs one agent drive with fresh pairings; returns the driver's exit status. */
export function drive(argv) {
  const split = argv.indexOf("--");
  const options = split >= 0 ? argv.slice(0, split) : argv, ops = split >= 0 ? argv.slice(split + 1) : [];
  const local = options.includes("--local");
  const pair = (flag(options, "--pair") ?? "").split(",").filter(Boolean);
  for (const name of pair) if (!ENVIRONMENTS[name]) throw new Error(`--pair names ${Object.keys(ENVIRONMENTS).join(", ")}, not ${name}`);
  const binary = process.env.EXACT_MAC_BIN || bundleBinary();
  if (!binary || !existsSync(binary)) throw new Error(`no bundle: EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`);
  mkdirSync(paths.drives, { recursive: true });
  const env = { ...process.env, EXACT_APP_DIR: example, EXACT_MAC_BIN: binary };
  // "This machine" inherits the app's environment: its provider homes are the fixture's (else the
  // welcome wizard offers this Mac's real Codex and Claude history for import) and it is offline too.
  if (local) Object.assign(env, { T3_LOCAL_HOME: localHome(paths), T3_LOCAL_PORT: String(checkPort(Number(process.env.T3_FIXTURE_LOCAL_PORT ?? LOCAL_PORT))),
    CODEX_HOME: join(paths.local, "codex"), CLAUDE_CONFIG_DIR: join(paths.local, "claude"), ...offlineEnv() });
  else { delete env.T3_LOCAL_HOME; delete env.T3_LOCAL_PORT; }
  const driverFlags = ["--size", flag(options, "--size") ?? "1280x840", "--storage", flag(options, "--storage") ?? "t3-fixtures", ...(options.includes("--json") ? ["--json"] : [])];
  // The tokens go to the driver's argv only: a drive prints each op's reply, never the op.
  const pairing = pair.length || local ? [`clock +${local ? welcomeWait(join(paths.local, "t3home")) : 5000} real`, ...pair.flatMap((name, index) => pairOps(pairingUrl(paths, name, "Fixture drive"), index)),
    ...(options.includes("--keep-welcome") ? [] : FINISH_WELCOME)] : [];
  const result = spawnSync(process.execPath, [join(checkout, "scripts/agent.mjs"), "macos", ...driverFlags, ...pairing, ...ops], { env, stdio: ["ignore", "pipe", "pipe"], encoding: "utf8", maxBuffer: 64 << 20 });
  process.stdout.write(redact(result.stdout ?? ""));
  process.stderr.write(redact(result.stderr ?? ""));
  return result.status ?? 1;
}

if (import.meta.main) {
  const command = process.argv[2];
  try {
    if (command === "start") {
      const name = nameArg();
      const port = flag(process.argv, "--port");
      console.log(JSON.stringify(await startServer(paths, name, port ? { port: Number(port) } : {})));
    } else if (command === "seed") console.log(JSON.stringify(await seed(paths, nameArg(), undefined, { turns: process.argv.includes("--turns") }), null, 2));
    else if (command === "stop") {
      const names = process.argv[3] === "all" ? Object.keys(ENVIRONMENTS) : [nameArg()];
      for (const name of names) { const stopped = await stopServer(paths, name); console.log(`${name}: ${stopped ? `stopped pid ${stopped.pid}` : "not running"}`); }
    } else if (command === "status") {
      for (const name of existsSync(paths.envs) ? readdirSync(paths.envs) : []) {
        const running = recorded(paths, name);
        console.log(`${name}: ${running ? `pid ${running.pid} port ${running.port} id ${running.environmentId} label ${JSON.stringify(running.label)}` : "stopped"}`);
        if (running) for (const { project, projectId, threads } of await shellOf(paths, name)) {
          console.log(`  ${project} (${projectId})`);
          for (const thread of threads) console.log(`    ${thread.title} (${thread.id})`);
        }
      }
    } else if (command === "drive") process.exitCode = drive(process.argv.slice(3));
    else if (command === "forget") {
      const name = nameArg();
      const file = join(envDir(paths, name), "environment.json");
      if (!existsSync(file)) throw new Error(`${name} has never started`);
      const { origin, environmentId } = JSON.parse(readFileSync(file, "utf8"));
      console.log(forgetKeychain(origin, environmentId) ? "removed" : "none saved");
    } else { console.log(readFileSync(import.meta.path, "utf8").split("\n").slice(1, 12).join("\n")); process.exitCode = command ? 2 : 0; }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
