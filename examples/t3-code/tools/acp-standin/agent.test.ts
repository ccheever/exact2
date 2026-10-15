// The ACP stand-in, offline: what install.mjs writes where the T3 server's ACP Registry catalog
// reads it, the agent's ACP v1 replies in each mode, and `forget`.
import { afterAll, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { AGENT, forget, install, installPaths, registryIndex, status } from "./install.mjs";

const root = mkdtempSync(join(tmpdir(), "t3-acp-standin-test-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));

/** Runs the installed executable with these requests on stdin; returns its replies by id. */
async function talk(executable: string, requests: { method: string; params?: object }[]) {
  const input = requests.map((request, index) => JSON.stringify({ jsonrpc: "2.0", id: index + 1, ...request })).join("\n") + "\n";
  const child = Bun.spawn([executable], { stdin: new Blob([input]), stdout: "pipe", stderr: "pipe", env: {}, cwd: root });
  const text = await new Response(child.stdout).text();
  await child.exited;
  const replies = new Map<number, { result?: any; error?: any }>();
  for (const line of text.trim().split("\n")) { const reply = JSON.parse(line); replies.set(reply.id, reply); }
  return replies;
}

describe("install", () => {
  test("writes the registry index and a regular executable at the path the catalog resolves", () => {
    const home = join(root, "home-a");
    const paths = install(home);
    const index = JSON.parse(readFileSync(join(home, "caches/acp-registry/registry.json"), "utf8"));
    expect(index).toEqual(registryIndex());
    const agent = index.agents[0];
    expect(agent.id).toMatch(/^[a-z0-9][a-z0-9._-]*$/);
    const target = Object.values(agent.distribution.binary)[0] as { archive: string; cmd: string };
    expect(new URL(target.archive).protocol).toBe("https:");
    // <home>/tools/<id>/<version>/<platform>/<cmd> (AcpRegistrySupport.ts agentInstallRoot, binaryPaths)
    expect(paths.executable).toBe(join(home, "tools", AGENT.id, AGENT.version, Object.keys(agent.distribution.binary)[0]!, target.cmd.replace("./", "")));
    const info = statSync(paths.executable);
    expect(info.isFile() && (info.mode & 0o111) !== 0).toBe(true);
    expect(readFileSync(paths.executable, "utf8").split("\n")[0]).toBe(`#!${process.execPath}`);
  });
});

describe("the agent", () => {
  test("agent mode: sign-in required for a session, authenticate signs in, logout signs out", async () => {
    const home = join(root, "home-b");
    const { executable } = install(home);
    const first = await talk(executable, [{ method: "initialize", params: { protocolVersion: 1 } }, { method: "session/new", params: { cwd: root, mcpServers: [] } }, { method: "nope" }]);
    expect(first.get(1)!.result).toMatchObject({ protocolVersion: 1, agentCapabilities: { auth: { logout: {} } }, authMethods: [{ id: "fixture-account" }] });
    expect(first.get(2)!.error).toEqual({ code: -32000, message: "Authentication required" });
    expect(first.get(3)!.error.code).toBe(-32601);
    const signIn = await talk(executable, [{ method: "authenticate", params: { methodId: "fixture-account" } }]);
    expect(signIn.get(1)!.result).toEqual({});
    expect(status(home).signedIn).toBe(true);
    const session = await talk(executable, [{ method: "session/new", params: { cwd: root, mcpServers: [] } }, { method: "logout", params: {} }]);
    expect(session.get(1)!.result.sessionId).toStartWith("standin-");
    expect(status(home).signedIn).toBe(false);
  });

  test("terminal mode advertises a login command; none needs no sign-in; --no-logout drops the capability", async () => {
    const terminal = install(join(root, "home-c"), { methods: "terminal", logout: false });
    const init = (await talk(terminal.executable, [{ method: "initialize", params: { protocolVersion: 1 } }])).get(1)!.result;
    expect(init.authMethods).toEqual([expect.objectContaining({ type: "terminal", args: ["--fixture-login"] })]);
    expect(init.agentCapabilities.auth).toBeUndefined();
    const none = install(join(root, "home-d"), { methods: "none" });
    const replies = await talk(none.executable, [{ method: "initialize", params: { protocolVersion: 1 } }, { method: "session/new", params: { cwd: root, mcpServers: [] } }]);
    expect(replies.get(1)!.result.authMethods).toEqual([]);
    expect(replies.get(2)!.result.sessionId).toBeString();
  });

  test("the terminal login signs in after Return", async () => {
    const home = join(root, "home-e");
    const { executable } = install(home, { methods: "terminal" });
    const child = Bun.spawn([executable, "--fixture-login"], { stdin: new Blob(["\n"]), stdout: "pipe", env: {} });
    expect(await new Response(child.stdout).text()).toContain("Signed in.");
    expect(await child.exited).toBe(0);
    expect(status(home).signedIn).toBe(true);
  });
});

test("forget removes this agent's provider instances, their caches and the sign-in, nothing else", () => {
  const home = join(root, "home-f");
  install(home);
  mkdirSync(join(home, "userdata"), { recursive: true });
  writeFileSync(join(home, "userdata/settings.json"), JSON.stringify({ providers: { codex: { binaryPath: "/nonexistent/x" } }, providerInstances: {
    acpRegistry_acp_stand_in: { driver: "acpRegistry", config: { agentId: AGENT.id } }, other: { driver: "acpRegistry", config: { agentId: "devin" } } } }));
  for (const name of ["acpRegistry_acp_stand_in.json", "other.json", `acp-auth-${"a".repeat(64)}.json`]) writeFileSync(join(home, "caches", name), "{}");
  writeFileSync(installPaths(home).state, JSON.stringify({ signedIn: true }));
  expect(forget(home)).toEqual(["acpRegistry_acp_stand_in"]);
  const settings = JSON.parse(readFileSync(join(home, "userdata/settings.json"), "utf8"));
  expect(Object.keys(settings.providerInstances)).toEqual(["other"]);
  expect(settings.providers.codex.binaryPath).toBe("/nonexistent/x");
  expect(existsSync(join(home, "caches/acpRegistry_acp_stand_in.json"))).toBe(false);
  expect(existsSync(join(home, "caches/other.json"))).toBe(true);
  expect(existsSync(join(home, `caches/acp-auth-${"a".repeat(64)}.json`))).toBe(false);
  expect(status(home).signedIn).toBe(false);
});
