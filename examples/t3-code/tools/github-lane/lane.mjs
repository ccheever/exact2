#!/usr/bin/env bun
// The real-GitHub lane: isolated T3 servers whose `gh` is the real GitHub CLI on a lane config
// dir the user signed in to. README.md has the recipe.
//
//   bun lane.mjs setup                 homes, wrappers, profiles; says which accounts are signed in
//   bun lane.mjs whoami                each signed-in account's login (through the lane gh)
//   bun lane.mjs start <account> [--port N]   a lane server for that account (16300-16799)
//   bun lane.mjs which <account>       which gh and config dir that server's children resolve
//   bun lane.mjs pair <account>        a single-use pairing URL, written to <server>/pairing-url (never printed)
//   bun lane.mjs project <account>     clones the sandbox for the server and adds it as a project
//   bun lane.mjs stop <account>        stops the server this lane started (its recorded pid only)
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { ACCOUNTS, PORTS, api, connect, ensureProject, example, git, lanePaths, note, readSandbox, run, serverDir, serverEnv, setup, signedIn } from "./lib.mjs";

const paths = lanePaths();

/** The server: `T3_LANE_SERVER` (a `t3` executable or a `bin.mjs` run by node), else the staged release. */
export function serverCommand() {
  const chosen = process.env.T3_LANE_SERVER;
  if (chosen) return chosen.endsWith(".mjs") || chosen.endsWith(".js") ? [process.env.T3_LANE_NODE || Bun.which("node") || "node", chosen] : [chosen];
  const pin = JSON.parse(readFileSync(join(example, "server-runtime/runtime-pin.json"), "utf8"));
  const tree = join(paths.runtime, pin.version), t3 = join(tree, "t3");
  if (!existsSync(t3)) {
    const archive = join(example, "server-runtime", pin.asset);
    if (!existsSync(archive)) throw new Error(`no staged server: run \`bun examples/t3-code/stage-runtime.mjs\` first, or set T3_LANE_SERVER`);
    mkdirSync(tree, { recursive: true });
    run("/usr/bin/tar", ["-xzf", archive, "-C", tree, "--strip-components=1"]);
  }
  return [t3];
}

const portOf = (account) => {
  const flag = process.argv.indexOf("--port");
  const port = flag > 0 ? Number(process.argv[flag + 1]) : PORTS[account];
  if (!(port >= 16300 && port <= 16799)) throw new Error(`lane ports are 16300-16799 (each task lane its own range), not ${port}`);
  return port;
};
const accountArg = () => {
  const account = process.argv[3];
  if (!ACCOUNTS.includes(account)) throw new Error(`name an account: ${ACCOUNTS.join(" | ")}`);
  return account;
};
const pidFile = (account) => join(serverDir(paths, account), "server.pid");
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch { return false; } };
const recorded = (account) => {
  if (!existsSync(pidFile(account))) return null;
  const { pid, port } = JSON.parse(readFileSync(pidFile(account), "utf8"));
  return alive(pid) ? { pid, port } : null;
};
const t3 = (account, args) => run(serverCommand()[0], [...serverCommand().slice(1), ...args], { env: serverEnv(paths, account), cwd: join(serverDir(paths, account), "repos") });

async function waitReady(port, ms = 60_000) {
  const started = Date.now();
  while (Date.now() - started < ms) {
    try { const response = await fetch(`http://127.0.0.1:${port}/.well-known/t3/environment`, { signal: AbortSignal.timeout(1000) }); if (response.ok) return await response.json(); } catch {}
    await Bun.sleep(200);
  }
  throw new Error(`the lane server did not answer on ${port} within ${ms / 1000} s (see ${join(serverDir(paths, "primary"), "server.log")})`);
}

export async function start(account, port = PORTS[account]) {
  setup(paths);
  if (!signedIn(paths, account)) console.error(`${account}: ${paths.configs[account]} has no token yet; the server's gh calls will be refused (README)`);
  const running = recorded(account);
  if (running) return running;
  const s = serverDir(paths, account);
  const log = openSync(join(s, "server.log"), "a");
  const [command, ...pre] = serverCommand();
  const child = spawn(command, [...pre, "serve", "--base-dir", join(s, "t3home"), "--port", String(port), "--host", "127.0.0.1", join(s, "repos")],
    { env: serverEnv(paths, account), cwd: join(s, "repos"), stdio: ["ignore", log, log], detached: true });
  child.unref();
  writeFileSync(pidFile(account), JSON.stringify({ pid: child.pid, port, startedAt: new Date().toISOString() }));
  note(paths, `start ${account} pid ${child.pid} port ${port}`);
  const environment = await waitReady(port);
  return { pid: child.pid, port, serverVersion: environment.serverVersion };
}

export function stop(account) {
  const running = recorded(account);
  if (!running) { rmSync(pidFile(account), { force: true }); return null; }
  process.kill(running.pid, "SIGTERM");
  rmSync(pidFile(account), { force: true });
  note(paths, `stop ${account} pid ${running.pid}`);
  return running;
}

/** A single-use pairing token for the account's server (kept off the terminal). */
export function pairingToken(account) {
  const text = t3(account, ["pair", "--base-dir", join(serverDir(paths, account), "t3home"), "--ttl", "15m", "--label", "GitHub lane"]).stdout;
  const token = /Token: (\S+)/.exec(text)?.[1];
  if (!token) throw new Error("t3 pair printed no token");
  return token;
}
export function pair(account) {
  const running = recorded(account);
  if (!running) throw new Error(`${account}: no lane server running`);
  const file = join(serverDir(paths, account), "pairing-url");
  writeFileSync(file, `http://127.0.0.1:${running.port}/pair#token=${pairingToken(account)}\n`, { mode: 0o600 });
  return file;
}

/** What a child of the server resolves: the login shell's gh and GH_CONFIG_DIR, as the server rebuilds PATH. */
export function which(account) {
  const out = run("/bin/zsh", ["-ilc", 'command -v gh; printf "%s\\n" "$GH_CONFIG_DIR"'], { env: serverEnv(paths, account), cwd: serverDir(paths, account) }).stdout.trim().split("\n");
  return { gh: out.at(-2), configDir: out.at(-1), expectedGh: join(paths.bins[account], "gh"), expectedConfig: paths.configs[account] };
}

/** The sandbox clone a server works in, and its project there. Returns { path, projectId?: string }. */
export function cloneSandbox(account) {
  const sandbox = readSandbox(paths);
  if (!sandbox) throw new Error("no sandbox.json yet: run seed.mjs first");
  const path = join(serverDir(paths, account), "repos", sandbox.repo.split("/")[1]);
  if (!existsSync(join(path, ".git"))) git(paths, account, ["clone", "--quiet", `https://github.com/${sandbox.repo}.git`, path]);
  else git(paths, account, ["fetch", "--quiet", "--prune", "origin"], { cwd: path });
  return path;
}
/** Connects to the account's running server with a fresh pairing token. */
export async function connectTo(account) {
  const running = recorded(account);
  if (!running) throw new Error(`${account}: no lane server running (bun lane.mjs start ${account})`);
  return connect(`http://127.0.0.1:${running.port}`, pairingToken(account));
}
export async function addProject(account) {
  const path = cloneSandbox(account);
  const conn = await connectTo(account);
  try { return { path, projectId: await ensureProject(conn, path) }; } finally { conn.close(); }
}

if (import.meta.main) {
  const command = process.argv[2];
  try {
    if (command === "setup") {
      setup(paths);
      for (const account of ACCOUNTS) console.log(`${account}: ${signedIn(paths, account) ? "signed in" : "not signed in"} (${paths.configs[account]})`);
      console.log(`lane: ${paths.lane}`);
    } else if (command === "whoami") {
      for (const account of ACCOUNTS) {
        if (!signedIn(paths, account)) { console.log(`${account}: not signed in`); continue; }
        const { status, body } = api(paths, account, ["user"]);
        console.log(`${account}: ${status === 200 ? body.login : `HTTP ${status}`}`);
      }
    } else if (command === "start") console.log(JSON.stringify(await start(accountArg(), portOf(accountArg()))));
    else if (command === "stop") console.log(JSON.stringify(stop(accountArg())));
    else if (command === "pair") console.log(`pairing URL written to ${pair(accountArg())}`);
    else if (command === "which") console.log(JSON.stringify(which(accountArg()), null, 2));
    else if (command === "project") console.log(JSON.stringify(await addProject(accountArg())));
    else if (command === "servers") for (const account of readdirSync(paths.servers)) console.log(account, JSON.stringify(recorded(account)));
    else { console.log(readFileSync(import.meta.path, "utf8").split("\n").slice(1, 11).join("\n")); process.exitCode = command ? 2 : 0; }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
