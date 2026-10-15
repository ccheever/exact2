// A stand-in Agent Client Protocol agent: enough of ACP v1 (newline-delimited JSON-RPC 2.0 on
// stdio, https://agentclientprotocol.com/protocol/initialization) for a T3 server's ACP Registry
// driver to list it, prepare it, probe it, sign it in and out, and run a turn. It talks to no
// service and keeps no credential; its only state is `state.json` ({ signedIn }) beside this file.
//
// A turn (`session/prompt`, https://agentclientprotocol.com/protocol/prompt-turn) is answered at
// once with one `agent_message_chunk` update, "Stand-in reply: <the first line of the user's words>", and the
// stop reason `end_turn`, so a fixture thread can hold a completed turn. It reads no file and runs
// no tool. A session the server opened before it restarted comes back by `session/load`, with no
// history replayed (the stand-in keeps none), so a later turn on the same thread runs too.
//
// `mode.json` beside this file (install.mjs writes it) picks what it advertises:
//   methods     "agent" (sign-in by `authenticate`), "terminal" (a login command the server runs
//               in a terminal), "both", or "none" (no sign-in: the no-account state)
//   authDelayMs how long `authenticate` takes, so a drive can see sign-in in progress
//   logout      whether it advertises sign-out (agentCapabilities.auth.logout)
//
// The terminal login (`--fixture-login`) prints a line, waits for Return and signs in.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";

const here = dirname(process.argv[1] ?? import.meta.path);
const statePath = join(here, "state.json");
const mode = { methods: "agent", authDelayMs: 0, logout: true, ...(existsSync(join(here, "mode.json")) ? JSON.parse(readFileSync(join(here, "mode.json"), "utf8")) : {}) };
const signedIn = () => existsSync(statePath) && JSON.parse(readFileSync(statePath, "utf8")).signedIn === true;
const setSignedIn = (value) => writeFileSync(statePath, `${JSON.stringify({ signedIn: value })}\n`);

if (process.argv.includes("--fixture-login")) {
  process.stdout.write("Fixture sign-in for the ACP stand-in (no account, nothing is sent anywhere).\nPress Return to finish: ");
  createInterface({ input: process.stdin }).once("line", () => { setSignedIn(true); process.stdout.write("Signed in.\n"); process.exit(0); });
} else {
  const methods = [];
  if (mode.methods === "agent" || mode.methods === "both") methods.push({ id: "fixture-account", name: "Fixture account", description: "Signs the stand-in in; no account or network" });
  if (mode.methods === "terminal" || mode.methods === "both") methods.push({ id: "fixture-terminal", name: "Fixture terminal sign-in", description: "A login command in a terminal", type: "terminal", args: ["--fixture-login"] });

  const send = (message) => process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`);
  const authRequired = { code: -32000, message: "Authentication required" };
  const handlers = {
    initialize: () => ({
      protocolVersion: 1,
      agentCapabilities: { loadSession: true, promptCapabilities: { image: false, audio: false, embeddedContext: false }, ...(mode.logout ? { auth: { logout: {} } } : {}) },
      agentInfo: { name: "fixture-acp-standin", title: "ACP stand-in", version: "0.0.1" },
      authMethods: methods,
    }),
    authenticate: async ({ methodId }) => {
      if (!methods.some((method) => method.id === methodId && method.type !== "terminal")) throw { code: -32602, message: `Unknown sign-in method: ${methodId}` };
      if (mode.authDelayMs > 0) await Bun.sleep(mode.authDelayMs);
      setSignedIn(true);
      return {};
    },
    logout: () => { setSignedIn(false); return {}; },
    "session/new": () => {
      if (methods.length > 0 && !signedIn()) throw authRequired;
      return { sessionId: `standin-${Date.now()}` };
    },
    "session/load": () => {
      if (methods.length > 0 && !signedIn()) throw authRequired;
      return {};
    },
    "session/prompt": ({ sessionId, prompt }) => {
      if (methods.length > 0 && !signedIn()) throw authRequired;
      const words = (Array.isArray(prompt) ? prompt : []).filter((block) => block?.type === "text").map((block) => String(block.text)).join("\n");
      // A T3 server wraps the user's words in its instructions (`<user_request>…User message:\n<words>\n</user_request>`).
      const asked = [...words.matchAll(/User message:\n([\s\S]*?)\n<\/user_request>/g)].at(-1)?.[1] ?? words;
      const text = `Stand-in reply: ${asked.split("\n").find((line) => line.trim())?.trim() ?? "(no text)"}`;
      send({ method: "session/update", params: { sessionId, update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text } } } });
      return { stopReason: "end_turn" };
    },
  };

  createInterface({ input: process.stdin }).on("line", async (line) => {
    let message;
    try { message = JSON.parse(line); } catch { return; }
    if (message.id === undefined || typeof message.method !== "string") return; // notifications and replies
    const handler = handlers[message.method];
    if (!handler) return send({ id: message.id, error: { code: -32601, message: `Method not found: ${message.method}` } });
    try { send({ id: message.id, result: await handler(message.params ?? {}) }); }
    catch (error) { send({ id: message.id, error: typeof error?.code === "number" ? error : { code: -32603, message: String(error?.message ?? error) } }); }
  });
}
