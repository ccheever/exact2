#!/usr/bin/env bun
// Installs the ACP stand-in (agent.mjs) into a T3 home so the server's ACP Registry lists it and
// Settings › Providers › Add provider can add it. README.md has the recipe.
//
//   bun install.mjs install --home <t3 home> [--mode agent|terminal|both|none] [--auth-delay <ms>] [--no-logout]
//   bun install.mjs status  --home <t3 home>     installed?, mode, signed in?
//   bun install.mjs reset   --home <t3 home>     signs the stand-in out (its state file only)
//   bun install.mjs release --home <t3 home>     ends the turn a "Hold the turn" prompt holds
//   bun install.mjs forget  --home <t3 home>     removes the provider instances a drive added for it (stop the server first)
//   bun install.mjs remove  --home <t3 home>     removes the registry cache and the installed agent
//
// The server's catalog (reference apps/server/src/provider/acp/AcpRegistrySupport.ts) reads the
// index from `<home>/caches/acp-registry/registry.json` when the registry cannot be fetched, and a
// binary agent whose command already sits at `<home>/tools/<id>/<version>/<platform>/<cmd>` is
// used as it is, without a download. A fixture server runs sandboxed off the network
// (second-environment), so the fetch fails and this index is the one it searches.
import { chmodSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

export const AGENT = {
  id: "fixture-acp-standin",
  name: "ACP Stand-in",
  version: "0.0.1",
  description: "A local stand-in agent for checking the Add provider dialog and a completed turn. It signs in to no account and answers a prompt with one line.",
  authors: ["T3 Code clone fixture"],
  license: "MIT",
  website: "https://example.invalid/acp-standin",
};
export const COMMAND = "acp-standin";
export const PLATFORM = `darwin-${process.arch === "arm64" ? "aarch64" : "x86_64"}`;

/** The registry index the server reads: one agent, a binary for this Mac (the archive is never fetched). */
export function registryIndex() {
  return {
    version: "1.0.0",
    agents: [{ ...AGENT, distribution: { binary: { [PLATFORM]: { archive: "https://example.invalid/acp-standin-0.0.1.tar.gz", cmd: `./${COMMAND}` } } } }],
    extensions: [],
  };
}

export function installPaths(t3home) {
  const root = join(t3home, "tools", AGENT.id, encodeURIComponent(AGENT.version), PLATFORM);
  return { registry: join(t3home, "caches/acp-registry/registry.json"), root, executable: join(root, COMMAND), mode: join(root, "mode.json"), state: join(root, "state.json"), release: join(root, "release") };
}

/**
 * Writes the index and the executable (agent.mjs behind this Bun's shebang; a copy, since the
 * server refuses a command whose real path leaves its install directory) and the mode.
 */
export function install(t3home, { methods = "agent", authDelayMs = 0, logout = true } = {}) {
  if (!["agent", "terminal", "both", "none"].includes(methods)) throw new Error(`--mode is agent, terminal, both or none, not ${methods}`);
  const paths = installPaths(t3home);
  mkdirSync(join(t3home, "caches/acp-registry"), { recursive: true });
  writeFileSync(paths.registry, `${JSON.stringify(registryIndex(), null, 2)}\n`);
  mkdirSync(paths.root, { recursive: true });
  const source = readFileSync(join(import.meta.dir, "agent.mjs"), "utf8");
  writeFileSync(paths.executable, `#!${process.execPath}\n${source}`);
  chmodSync(paths.executable, 0o755);
  writeFileSync(paths.mode, `${JSON.stringify({ methods, authDelayMs, logout }, null, 2)}\n`);
  return paths;
}

/** Rewrites an installed executable from agent.mjs when they differ, keeping its mode and state; returns whether it did. */
export function refresh(t3home) {
  const { executable } = installPaths(t3home);
  const wanted = `#!${process.execPath}\n${readFileSync(join(import.meta.dir, "agent.mjs"), "utf8")}`;
  if (!existsSync(executable) || readFileSync(executable, "utf8") === wanted) return false;
  writeFileSync(executable, wanted);
  return true;
}

export function status(t3home) {
  const paths = installPaths(t3home);
  const read = (file) => (existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : null);
  return { installed: existsSync(paths.executable) && existsSync(paths.registry), mode: read(paths.mode), signedIn: read(paths.state)?.signedIn === true, executable: paths.executable };
}

export function reset(t3home) { rmSync(installPaths(t3home).state, { force: true }); }

/** Lets a held turn ("Hold the turn") finish: the stand-in replies once it sees `release`, and removes it. */
export function release(t3home) { writeFileSync(installPaths(t3home).release, ""); }

/**
 * Undoes a drive's Add provider: the `acpRegistry` instances of this agent in the server's
 * settings, their status caches and sign-in confirmations (`caches/<instance>.json`,
 * `caches/acp-auth-*.json`, reference AcpRegistryAuthenticationState.ts), and the stand-in's own
 * state. A running server keeps its copy, so stop it first. Returns the instance ids removed.
 */
export function forget(t3home) {
  const file = join(t3home, "userdata/settings.json");
  const settings = existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : {};
  const ids = Object.entries(settings.providerInstances ?? {}).filter(([, entry]) => entry?.driver === "acpRegistry" && entry?.config?.agentId === AGENT.id).map(([id]) => id);
  for (const id of ids) { delete settings.providerInstances[id]; rmSync(join(t3home, "caches", `${id}.json`), { force: true }); }
  if (ids.length) writeFileSync(file, `${JSON.stringify(settings, null, 2)}\n`);
  const caches = join(t3home, "caches");
  for (const name of existsSync(caches) ? readdirSync(caches) : []) if (/^acp-auth-[0-9a-f]+\.json$/.test(name)) rmSync(join(caches, name), { force: true });
  reset(t3home);
  return ids;
}

export function remove(t3home) {
  const paths = installPaths(t3home);
  rmSync(paths.registry, { force: true });
  rmSync(join(t3home, "tools", AGENT.id), { recursive: true, force: true });
}

if (import.meta.main) {
  const [command] = process.argv.slice(2);
  const flag = (name) => { const at = process.argv.indexOf(name); return at > 0 ? process.argv[at + 1] : undefined; };
  try {
    const home = flag("--home");
    if (!home && command) throw new Error("name the T3 home: --home <dir> (a fixture environment's is target/t3-fixtures/envs/<name>/t3home)");
    const t3home = home ? resolve(home) : "";
    if (command === "install") {
      const paths = install(t3home, { methods: flag("--mode") ?? "agent", authDelayMs: Number(flag("--auth-delay") ?? 0), logout: !process.argv.includes("--no-logout") });
      console.log(`installed ${AGENT.id} ${AGENT.version} at ${paths.executable}`);
    } else if (command === "status") console.log(JSON.stringify(status(t3home), null, 2));
    else if (command === "reset") { reset(t3home); console.log("signed out"); }
    else if (command === "release") { release(t3home); console.log("released"); }
    else if (command === "forget") console.log(`forgot ${forget(t3home).join(", ") || "nothing"}`);
    else if (command === "remove") { remove(t3home); console.log("removed"); }
    else { console.log(readFileSync(import.meta.path, "utf8").split("\n").slice(1, 10).join("\n")); process.exitCode = command ? 2 : 0; }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
