// The ACP stand-in, offline: what install.mjs writes where the T3 server's ACP Registry catalog
// reads it, the agent's ACP v1 replies in each mode, and `forget`.
import { afterAll, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { AGENT, forget, install, installPaths, registryIndex, status } from "./install.mjs";

const root = mkdtempSync(join(tmpdir(), "t3-acp-standin-test-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));

/** Runs the installed executable with these requests on stdin; returns its replies by id (its notifications under `notes`). */
async function talk(executable: string, requests: { method: string; params?: object }[], notes: { method: string; params: any }[] = []) {
  const input = requests.map((request, index) => JSON.stringify({ jsonrpc: "2.0", id: index + 1, ...request })).join("\n") + "\n";
  const child = Bun.spawn([executable], { stdin: new Blob([input]), stdout: "pipe", stderr: "pipe", env: {}, cwd: root });
  const text = await new Response(child.stdout).text();
  await child.exited;
  const replies = new Map<number, { result?: any; error?: any }>();
  for (const line of text.trim().split("\n")) { const reply = JSON.parse(line); if (reply.id === undefined) notes.push(reply); else replies.set(reply.id, reply); }
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

  test("a prompt is one turn: the user's words echoed in one message chunk and a usage update, then end_turn; a session comes back by session/load", async () => {
    const { executable } = install(join(root, "home-g"), { methods: "none" });
    // A T3 server's prompt: its instructions around the user's words, then more context.
    const wrapped = "<t3_code_instructions>\nBe brief.\n</t3_code_instructions>\n<user_request>\nUser message:\nSay hello\nand more\n</user_request>";
    const notes: { method: string; params: any }[] = [];
    const replies = await talk(executable, [{ method: "initialize", params: { protocolVersion: 1 } }, { method: "session/new", params: { cwd: root, mcpServers: [] } },
      { method: "session/prompt", params: { sessionId: "s1", prompt: [{ type: "text", text: wrapped }, { type: "text", text: "<runtime_info>context</runtime_info>" }] } },
      { method: "session/load", params: { sessionId: "s1", cwd: root, mcpServers: [] } }], notes);
    expect(replies.get(1)!.result.agentCapabilities.loadSession).toBe(true);
    expect(replies.get(3)!.result).toEqual({ stopReason: "end_turn" });
    expect(notes.map((note) => note.params.update)).toEqual([
      { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "Stand-in reply: Say hello" } },
      { sessionUpdate: "usage_update", used: 12480, size: 200000 }]);
    expect(replies.get(4)!.result).toEqual({});
  });

  test("'Edit the notes' edits the session's directory before the turn ends; 'Ask before editing the notes' writes only once allowed", async () => {
    const { executable } = install(join(root, "home-i"), { methods: "none" });
    const repo = join(root, "repo-i");
    mkdirSync(repo, { recursive: true });
    writeFileSync(join(repo, "README.md"), "# repo\n\nA fixture repository.\n");
    // A thread's first turn: the words alone inside <user_request>.
    const first = (words: string) => [{ type: "text", text: `<t3_code_instructions>\nUser message: is not here\n</t3_code_instructions>\n<user_request>\n${words}\n</user_request>` }];
    const notes: { method: string; params: any }[] = [];
    const replies = await talk(executable, [{ method: "session/new", params: { cwd: repo, mcpServers: [] } }], notes);
    const sessionId = replies.get(1)!.result.sessionId;
    await talk(executable, [{ method: "session/load", params: { sessionId, cwd: repo, mcpServers: [] } }, { method: "session/prompt", params: { sessionId, prompt: first("Edit the notes") } }], notes);
    expect(readFileSync(join(repo, "README.md"), "utf8")).toContain("edited by the ACP stand-in");
    expect(readFileSync(join(repo, "notes/plan.md"), "utf8")).toStartWith("# Plan");
    expect(notes.find((note) => note.params.update.sessionUpdate === "tool_call")!.params.update).toMatchObject({ kind: "edit", status: "completed" });

    // The permission request goes to the client; the turn ends with its answer.
    const child = Bun.spawn([executable], { stdin: "pipe", stdout: "pipe", env: {}, cwd: root });
    const lines = child.stdout.pipeThrough(new TextDecoderStream()).getReader();
    let buffered = "";
    const next = async () => { for (;;) { const at = buffered.indexOf("\n"); if (at >= 0) { const line = buffered.slice(0, at); buffered = buffered.slice(at + 1); return JSON.parse(line); } buffered += (await lines.read()).value ?? ""; } };
    const write = (message: object) => { child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`); child.stdin.flush(); };
    write({ id: 1, method: "session/load", params: { sessionId, cwd: repo, mcpServers: [] } });
    expect((await next()).result).toEqual({});
    write({ id: 2, method: "session/prompt", params: { sessionId, prompt: first("Ask before editing the notes") } });
    expect((await next()).params.update).toMatchObject({ sessionUpdate: "tool_call", kind: "edit", status: "pending" });
    const asked = await next();
    expect(asked).toMatchObject({ method: "session/request_permission", params: { sessionId, toolCall: { kind: "edit", title: "Write notes/approved.md" } } });
    expect(asked.params.options.map((option: { kind: string }) => option.kind)).toEqual(["allow_once", "allow_always", "reject_once"]);
    expect(existsSync(join(repo, "notes/approved.md"))).toBe(false);
    write({ id: asked.id, result: { outcome: { outcome: "selected", optionId: "allow" } } });
    const rest = [await next(), await next(), await next(), await next()];
    expect(rest.map((message) => message.params?.update?.sessionUpdate ?? message.result?.stopReason)).toEqual(["tool_call_update", "agent_message_chunk", "usage_update", "end_turn"]);
    expect(readFileSync(join(repo, "notes/approved.md"), "utf8")).toContain("after the approval");
    child.stdin.end();
    await child.exited;
  });

  test("a prompt needs the sign-in an agent mode asks for", async () => {
    const { executable } = install(join(root, "home-h"));
    const replies = await talk(executable, [{ method: "session/prompt", params: { sessionId: "s1", prompt: [{ type: "text", text: "hi" }] } }]);
    expect(replies.get(1)!.error).toEqual({ code: -32000, message: "Authentication required" });
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
