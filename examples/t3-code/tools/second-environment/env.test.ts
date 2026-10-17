// The environment fixture's own pieces, offline: its port rule, the lane server's environment,
// the pinned providers, the label shim, the offline proxy's refusal and the drive's ops. No T3
// server starts here.
import { afterAll, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FINISH_WELCOME, pairOps, redact } from "./env.mjs";
import { forget as forgetStandin } from "../acp-standin/install.mjs";
import { makeHub, HUB_ACCOUNTS } from "./usage-hub.mjs";
import { ENVIRONMENTS, HUB, LOCAL_PORT, SEEDS, STANDIN_INSTANCE, assertOfflinePortClosed, checkPort, envDir, fixturePaths, offlineEnv, run, seedRepository, seedSettings, serverEnv, setupEnvironment, standinInstance, turnCompleted, turnSeeded, welcomeWait } from "./lib.mjs";

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

describe("threads with a turn (seed --turns)", () => {
  test("only `second` has them: a completed turn, one with file edits, one waiting on an approval; and an empty project", () => {
    expect(SEEDS.second.flatMap((entry) => (entry.turns ?? []).map((turn) => [entry.repo, turn.title, turn.prompt, !!turn.approval]))).toEqual([
      ["background-only", "Background thread (completed turn)", "Say hello from the stand-in", false],
      ["agent-edits", "Turn with file edits", "Edit the notes", false],
      ["agent-edits", "Edit awaiting approval", "Ask before editing the notes", true]]);
    expect(SEEDS.second.find((entry) => entry.repo === "empty-project")!.threads).toEqual([]);
    expect(SEEDS.third.some((entry) => entry.turns)).toBe(false);
    expect(STANDIN_INSTANCE).toMatch(/^[a-zA-Z][a-zA-Z0-9_-]*$/); // ProviderInstanceId
  });

  test("a turn is done once the latest run has completed and none is active; an approval turn once its request is pending", () => {
    const completed = { latestRunId: "r1", latestRunCompletedAt: "2026-10-16T00:00:00Z", activeRunId: null };
    expect(turnCompleted(completed)).toBe(true);
    expect(turnCompleted({ latestRunId: null, latestRunCompletedAt: null })).toBe(false);
    expect(turnCompleted({ latestRunId: "r1", latestRunCompletedAt: null, activeRunId: "r1" })).toBe(false);
    expect(turnCompleted(undefined)).toBe(false);
    const waiting = { latestRunId: "r1", latestRunCompletedAt: null, activeRunId: "r1", pendingRuntimeRequest: { id: "q1", kind: "file-change" } };
    expect([turnSeeded({ approval: true }, waiting), turnSeeded({ approval: true }, completed), turnSeeded({ approval: true }, { ...waiting, pendingRuntimeRequest: null })]).toEqual([true, true, false]);
    expect(turnSeeded({}, waiting)).toBe(false);
  });

  test("the stand-in's instance is one install.mjs forget removes", () => {
    const home = join(root, "forget-home");
    seedSettings(home);
    const file = join(home, "userdata/settings.json");
    writeFileSync(file, JSON.stringify({ ...JSON.parse(readFileSync(file, "utf8")), providerInstances: { [STANDIN_INSTANCE]: standinInstance() } }));
    expect(forgetStandin(home)).toEqual([STANDIN_INSTANCE]);
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

describe("the usage hub stand-in (seed --limits)", () => {
  test("lists its accounts, answers each one's usage the way cliproxyApi.ts reads it, and a consumed credit empties the session window", () => {
    const now = Date.parse("2026-10-17T00:00:00Z"), seconds = now / 1000;
    const hub = makeHub(HUB_ACCOUNTS, () => now);
    const [, list] = hub("GET", "/v0/management/auth-files", undefined) as [number, { files: { provider: string; auth_index: string }[] }];
    expect(list.files.map((file) => file.provider)).toEqual(["codex", "codex", "codex", "claude", "claude"]);
    const call = (index: string, url: string, data?: object) => {
      const [status, reply] = hub("POST", "/v0/management/api-call", { auth_index: index, url, ...(data ? { data: JSON.stringify(data) } : {}) }) as [number, { status_code: number; body: string }];
      expect([status, reply.status_code]).toEqual([200, 200]);
      return JSON.parse(reply.body);
    };
    const codex = "https://chatgpt.com/backend-api/wham";
    expect(call("lane-1", `${codex}/usage`).rate_limit.primary_window).toEqual({ used_percent: 35, reset_at: seconds + 7200, limit_window_seconds: 18000 });
    expect(call("lane-1", `${codex}/rate-limit-reset-credits`).credits.map((credit: { id: string }) => credit.id)).toEqual(["lane-1-credit-1", "lane-1-credit-2"]);
    expect(call("lane-1", `${codex}/rate-limit-reset-credits/consume`, { credit_id: "lane-1-credit-1" })).toEqual({ code: "reset" });
    expect(call("lane-1", `${codex}/usage`).rate_limit.primary_window.used_percent).toBe(0);
    expect(call("lane-1", `${codex}/rate-limit-reset-credits/consume`, { credit_id: "lane-1-credit-1" })).toEqual({ code: "no_credit" });
    expect(call("lane-4", "https://api.anthropic.com/api/oauth/usage").five_hour).toEqual({ utilization: 55, resets_at: new Date(now + 3 * 3_600_000).toISOString() });
    expect(hub("POST", "/v0/management/reset-quota", { auth_index: "lane-1" })[0]).toBe(200);
    expect(checkPort(HUB.port, {})).toBe(HUB.port);
  });
});

describe("the wait for the welcome wizard", () => {
  test("20 s until the local home holds the pinned runtime, 5 s after", () => {
    const home = join(root, "wait-home");
    expect(welcomeWait(home)).toBe(20000);
    const { version } = JSON.parse(readFileSync(join(import.meta.dir, "../../server-runtime/runtime-pin.json"), "utf8"));
    mkdirSync(join(home, "runtime/versions", version), { recursive: true });
    expect(welcomeWait(home)).toBe(5000);
  });
});
