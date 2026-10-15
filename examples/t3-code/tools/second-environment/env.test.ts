// The environment fixture's own pieces, offline: its port rule, the lane server's environment,
// the pinned providers, the label shim, the offline proxy's refusal and the drive's ops. No T3
// server starts here.
import { afterAll, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FINISH_WELCOME, pairOps, redact } from "./env.mjs";
import { ENVIRONMENTS, LOCAL_PORT, SEEDS, assertOfflinePortClosed, checkPort, envDir, fixturePaths, offlineEnv, run, seedRepository, seedSettings, serverEnv, setupEnvironment } from "./lib.mjs";

const root = mkdtempSync(join(tmpdir(), "t3-fixture-env-test-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const paths = fixturePaths({ T3_FIXTURE_ROOT: root });

describe("ports", () => {
  test("16500-16599 by default, another lane's range by T3_FIXTURE_PORTS, never 3773", () => {
    for (const port of [LOCAL_PORT, ...Object.values(ENVIRONMENTS).map((entry) => entry.port)]) expect(checkPort(port, {})).toBe(port);
    expect(() => checkPort(16600, {})).toThrow("16500-16599");
    expect(checkPort(16700, { T3_FIXTURE_PORTS: "16700-16710" })).toBe(16700);
    expect(() => checkPort(3773, { T3_FIXTURE_PORTS: "3000-4000" })).toThrow();
  });
});

describe("a lane server's environment", () => {
  test("is built from nothing: no token, home or config of this shell; its homes under the fixture root; offline", () => {
    const built = serverEnv(paths, "second", { USER: "u", GH_TOKEN: "leak", OPENAI_API_KEY: "leak", HOME: "/Users/u", CODEX_HOME: "/Users/u/.codex" });
    expect(Object.values(built)).not.toContain("leak");
    for (const key of ["HOME", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "T3CODE_HOME", "TMPDIR", "GH_CONFIG_DIR"]) expect(built[key]!.startsWith(envDir(paths, "second"))).toBe(true);
    expect(built).toMatchObject({ T3CODE_TELEMETRY_ENABLED: "false", NODE_USE_ENV_PROXY: "1", HTTPS_PROXY: "http://127.0.0.1:16599", NO_PROXY: "127.0.0.1,localhost" });
    expect(built.PATH.split(":")[0]).toBe(join(envDir(paths, "second"), "bin"));
  });

  test("the offline proxy port must refuse: a listener there stops a start", async () => {
    const server = createServer().listen(0, "127.0.0.1");
    await new Promise((done) => server.once("listening", done));
    const port = (server.address() as { port: number }).port;
    const env = { T3_FIXTURE_PORTS: `${port}-${port}`, T3_FIXTURE_OFFLINE_PORT: String(port) };
    expect(offlineEnv(env).HTTPS_PROXY).toBe(`http://127.0.0.1:${port}`);
    await expect(assertOfflinePortClosed(env)).rejects.toThrow("offline proxy port");
    server.close();
    await new Promise((done) => server.once("close", done));
    await expect(assertOfflinePortClosed(env)).resolves.toBeUndefined();
  });

  test("settings pin every built-in provider to a missing binary and keep what a drive added", () => {
    const home = join(root, "settings-home");
    seedSettings(home);
    const file = join(home, "userdata/settings.json");
    const first = JSON.parse(readFileSync(file, "utf8"));
    for (const id of ["codex", "claudeAgent", "cursor", "grok", "pi", "opencode"]) expect(first.providers[id].binaryPath).toStartWith("/nonexistent/");
    writeFileSync(file, JSON.stringify({ ...first, providerInstances: { kept: { driver: "acpRegistry" } }, providers: { ...first.providers, codex: { enabled: false } } }));
    const second = JSON.parse(readFileSync(seedSettings(home), "utf8"));
    expect(second.providerInstances).toEqual({ kept: { driver: "acpRegistry" } });
    expect(second.providers.codex).toEqual({ enabled: false, binaryPath: "/nonexistent/t3-fixture/codex" });
  });

  test("the login shell finds the label shim first, and it answers ComputerName with the lane's label", () => {
    setupEnvironment(paths, "third");
    const env = serverEnv(paths, "third", { USER: "u" });
    const out = run("/bin/zsh", ["-ilc", "command -v scutil; scutil --get ComputerName"], { env }).stdout.trim().split("\n");
    expect(out.slice(-2)).toEqual([join(envDir(paths, "third"), "bin/scutil"), "Lane Third"]);
  });

  test("a seeded repository has one commit and its Git origin, which the clone groups projects by", () => {
    setupEnvironment(paths, "second");
    const entry = SEEDS.second.find((seed) => seed.repo === "shared-notes")!;
    expect(SEEDS.third.find((seed) => seed.repo === "shared-notes")!.origin).toBe(entry.origin);
    const path = seedRepository(paths, "second", entry);
    const env = serverEnv(paths, "second", { USER: "u" });
    expect(run("/usr/bin/git", ["remote", "get-url", "origin"], { env, cwd: path }).stdout.trim()).toBe(entry.origin);
    expect(run("/usr/bin/git", ["rev-list", "--count", "HEAD"], { env, cwd: path }).stdout.trim()).toBe("1");
    expect(seedRepository(paths, "second", entry)).toBe(path);
    expect(existsSync(join(path, "README.md"))).toBe(true);
  });
});

describe("a drive", () => {
  test("pairs the first environment in the open welcome field and each later one through Add a computer", () => {
    expect(pairOps("http://127.0.0.1:16510/pair#token=A", 0)[0]).toBe("type welcome-pairing-link http://127.0.0.1:16510/pair#token=A");
    expect(pairOps("http://127.0.0.1:16511/pair#token=B", 1).slice(0, 2)).toEqual(["tap welcome-add-computer", "type welcome-pairing-link http://127.0.0.1:16511/pair#token=B"]);
    expect(FINISH_WELCOME).toContain("tap welcome-skip-import");
  });
  test("prints no pairing token", () => {
    expect(redact('typed #448 "welcome-pairing-link" · value="http://127.0.0.1:16510/pair#token=LEY3T64Y3TVC"')).toBe('typed #448 "welcome-pairing-link" · value="http://127.0.0.1:16510/pair#token=<redacted>"');
  });
});
