// A stand-in Agent Client Protocol agent: enough of ACP v1 (newline-delimited JSON-RPC 2.0 on
// stdio, https://agentclientprotocol.com/protocol/initialization) for a T3 server's ACP Registry
// driver to list it, prepare it, probe it, sign it in and out, and run a turn. It talks to no
// service and keeps no credential; its only state is `state.json` ({ signedIn }) beside this file.
//
// A turn (`session/prompt`, https://agentclientprotocol.com/protocol/prompt-turn) is answered at
// once with one `agent_message_chunk` update, "Stand-in reply: <the first line of the user's words>", a
// `usage_update` (so the thread has a context meter), and the stop reason `end_turn`, so a fixture
// thread can hold a completed turn. Two first lines do more, in the session's directory:
//   "Edit the notes"                edits README.md and writes notes/plan.md and notes/outline.ts (code,
//                                   for a file preview with a gutter) before the turn ends, so
//                                   the turn's checkpoint (a Git snapshot) has a diff; one `edit` tool call
//   "Write <n> notes [after <s>s]"  waits s seconds (0 by default), then writes n new files under notes/batch-<time>/,
//                                   so each such turn's checkpoint has n files of three lines (Proactive panels opens
//                                   the diff from 3 files); one `edit` tool call
//   "Ask before editing the notes"  asks first (`session/request_permission`, an `edit` tool call), so a
//                                   thread in the runtime mode "approval-required" waits on an approval;
//                                   allowed, it writes notes/approved.md
//   "Ask a question"                asks one free-text question (`elicitation/create`, a form with one string
//                                   field), which the server shows as a pending question; the reply repeats the
//                                   answer and names each attachment the server appended to it, without its path
//   "Read the thread <id> [for <n>s]"
//                                   thinks ("**Checking the other thread**"), then holds a running
//                                   `t3_thread_read` tool call on that thread id for n seconds (0 by
//                                   default), so the live row shows the latest thought over a running tool
//   "Wait on the thread <id> [for <n>s]"
//                                   holds a running `t3_thread_wait` tool call on that thread id for n seconds,
//                                   so the server reports the turn blocked on another thread (Waiting)
//   "Link the thread <id>"          replies with a `t3-thread://v1/<id>` link to it (a thread chip)
//   "Fail the turn"                 fails the turn with a provider's JSON error, whose message is a sentence
//   "Show the Markdown sample [pausing <n>s]"
//                                   replies with Markdown (MARKDOWN_SAMPLE): a Mermaid flowchart in one chunk, then,
//                                   n seconds later (0 by default), a sequence diagram, a table and two display
//                                   formulas, so a drive sees a closed fence drawn while the reply streams
// After `session/new` and `session/load` it advertises one skill, `$frontend-design`
// (`available_commands_update`; T3 lists a `$` command as a skill), so the composer's `$` menu can
// insert a skill chip.
// A session the server opened before it restarted comes back by `session/load`, with no history
// replayed (the stand-in keeps none), so a later turn on the same thread runs too.
//
// `mode.json` beside this file (install.mjs writes it) picks what it advertises:
//   methods     "agent" (sign-in by `authenticate`), "terminal" (a login command the server runs
//               in a terminal), "both", or "none" (no sign-in: the no-account state)
//   authDelayMs how long `authenticate` takes, so a drive can see sign-in in progress
//   logout      whether it advertises sign-out (agentCapabilities.auth.logout)
//
// The terminal login (`--fixture-login`) prints a line, waits for Return and signs in.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";

const here = dirname(process.argv[1] ?? import.meta.path);
const statePath = join(here, "state.json");
const mode = { methods: "agent", authDelayMs: 0, logout: true, ...(existsSync(join(here, "mode.json")) ? JSON.parse(readFileSync(join(here, "mode.json"), "utf8")) : {}) };
const signedIn = () => existsSync(statePath) && JSON.parse(readFileSync(statePath, "utf8")).signedIn === true;
const setSignedIn = (value) => writeFileSync(statePath, `${JSON.stringify({ signedIn: value })}\n`);
/** "Show the Markdown sample": its two chunks. */
export const MARKDOWN_SAMPLE = [
  "Here is the sample.\n\n```mermaid\nflowchart LR\n  Draft[Draft reply] --> Review{Review}\n  Review -->|approve| Ship[Ship it]\n  Review -->|revise| Draft\n```\n\n",
  "```mermaid\nsequenceDiagram\n  participant App\n  participant Server\n  App->>Server: Subscribe to the thread\n  Server-->>App: Snapshot, then each event as it happens\n```\n\n"
    + "| Part | Role | Notes long enough to wrap when the table cells are expanded |\n| --- | --- | --- |\n"
    + "| `pele` | Mermaid renderer | Lays each diagram out for the lane's width and again when the window narrows |\n"
    + "| KaTeX | Math typesetter | Typesets display formulas while Settings › Appearance › Render math is on |\n"
    + "| Tables | Markdown | Expanded cells wrap, and a wide lane fits the whole table without scrolling |\n\n"
    + "The energy of a body at rest:\n\n$$\nE = mc^2\n$$\n\nThe sum of the first n integers:\n\n\\[\n\\sum_{k=1}^{n} k = \\frac{n(n+1)}{2}\n\\]\n",
];

if (process.argv.includes("--fixture-login")) {
  process.stdout.write("Fixture sign-in for the ACP stand-in (no account, nothing is sent anywhere).\nPress Return to finish: ");
  createInterface({ input: process.stdin }).once("line", () => { setSignedIn(true); process.stdout.write("Signed in.\n"); process.exit(0); });
} else {
  const methods = [];
  if (mode.methods === "agent" || mode.methods === "both") methods.push({ id: "fixture-account", name: "Fixture account", description: "Signs the stand-in in; no account or network" });
  if (mode.methods === "terminal" || mode.methods === "both") methods.push({ id: "fixture-terminal", name: "Fixture terminal sign-in", description: "A login command in a terminal", type: "terminal", args: ["--fixture-login"] });

  const send = (message) => process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`);
  const update = (sessionId, body) => send({ method: "session/update", params: { sessionId, update: body } });
  const authRequired = { code: -32000, message: "Authentication required" };
  const COMMANDS = [{ name: "$frontend-design", description: "Build distinctive, production-grade frontend interfaces" }];
  // After the reply, as an agent's advertisement follows the session it is for.
  const advertise = (sessionId) => setTimeout(() => update(sessionId, { sessionUpdate: "available_commands_update", availableCommands: COMMANDS }), 0);
  const sessions = new Map(); // session id → its directory (session/new's and session/load's `cwd`)
  const asked = new Map(); // id of a request to the client → its reply's resolver
  const request = (method, params) => new Promise((resolve) => { const id = `standin-${asked.size + 1}-${Date.now()}`; asked.set(id, resolve); send({ id, method, params }); });
  const editNotes = (cwd) => {
    const readme = join(cwd, "README.md"), plan = join(cwd, "notes/plan.md"), outline = join(cwd, "notes/outline.ts");
    const before = existsSync(readme) ? readFileSync(readme, "utf8") : "";
    writeFileSync(readme, `${before.replace("A fixture repository.", "A fixture repository, edited by the ACP stand-in.")}\n## Notes\n\n- Keep the plan in notes/plan.md.\n- Review it before each release.\n`);
    mkdirSync(join(cwd, "notes"), { recursive: true });
    writeFileSync(plan, "# Plan\n\n1. Write the outline.\n2. Fill in each section.\n3. Read it once more.\n\nOpen questions:\n\n- Who reviews the draft?\n");
    writeFileSync(outline, "export const sections = [\n  \"Introduction\",\n  \"Method\",\n  \"Results\",\n];\n\nexport function title(index: number): string {\n  return sections[index] ?? \"Appendix\";\n}\n");
    return [readme, plan, outline];
  };
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
    "session/new": ({ cwd }) => {
      if (methods.length > 0 && !signedIn()) throw authRequired;
      const sessionId = `standin-${Date.now()}`;
      if (typeof cwd === "string") sessions.set(sessionId, cwd);
      advertise(sessionId);
      return { sessionId };
    },
    "session/load": ({ sessionId, cwd }) => {
      if (methods.length > 0 && !signedIn()) throw authRequired;
      if (typeof cwd === "string") sessions.set(sessionId, cwd);
      advertise(sessionId);
      return {};
    },
    "session/prompt": async ({ sessionId, prompt }) => {
      if (methods.length > 0 && !signedIn()) throw authRequired;
      const words = (Array.isArray(prompt) ? prompt : []).filter((block) => block?.type === "text").map((block) => String(block.text)).join("\n");
      // A T3 server wraps the user's words in its instructions: `<user_request>\n<words>\n</user_request>` on a
      // thread's first turn, with a "User message:" line before the words on a later one.
      const userWords = [...words.matchAll(/<user_request>\n(?:(?:(?!<\/user_request>)[\s\S])*?User message:\n)?([\s\S]*?)\n<\/user_request>/g)].at(-1)?.[1] ?? words;
      const first = userWords.split("\n").find((line) => line.trim())?.trim() ?? "";
      const cwd = sessions.get(sessionId);
      let text = `Stand-in reply: ${first || "(no text)"}`;
      if (cwd && /^ask before editing the notes/i.test(first)) {
        const path = join(cwd, "notes/approved.md");
        const toolCall = { toolCallId: `approve-${Date.now()}`, title: "Write notes/approved.md", kind: "edit", status: "pending", locations: [{ path }], rawInput: { path: "notes/approved.md" } };
        update(sessionId, { sessionUpdate: "tool_call", ...toolCall });
        const reply = await request("session/request_permission", { sessionId, toolCall, options: [
          { optionId: "allow", name: "Allow once", kind: "allow_once" }, { optionId: "allow-always", name: "Always allow", kind: "allow_always" },
          { optionId: "reject", name: "Reject", kind: "reject_once" }] });
        const outcome = reply?.result?.outcome, allowed = outcome?.outcome === "selected" && String(outcome.optionId).startsWith("allow");
        if (allowed) { mkdirSync(join(cwd, "notes"), { recursive: true }); writeFileSync(path, "# Approved\n\nWritten after the approval.\n"); }
        update(sessionId, { sessionUpdate: "tool_call_update", toolCallId: toolCall.toolCallId, status: allowed ? "completed" : "failed" });
        text = allowed ? "Stand-in reply: wrote notes/approved.md" : "Stand-in reply: left the notes as they were";
      } else if (/^ask a question/i.test(first)) {
        const reply = await request("elicitation/create", { sessionId, mode: "form", message: "Which window shows the problem?",
          requestedSchema: { type: "object", properties: { answer: { type: "string", title: "Window" } }, required: ["answer"] } });
        const accepted = reply?.result?.action === "accept", answer = accepted ? String(reply.result.content?.answer ?? "") : "";
        // The server appends one `Attached <type> "<name>": "<path>"` line per attachment (userInputAttachments.ts).
        const line = /^Attached (image|file) ("(?:[^"\\]|\\.)*"): ("(?:[^"\\]|\\.)*")$/gm;
        const attached = [...answer.matchAll(line)].map(([, type, name, path]) => `${type} ${JSON.parse(name)}${existsSync(JSON.parse(path)) ? "" : " (missing)"}`);
        const typed = answer.replace(line, "").trim();
        text = !accepted ? "Stand-in reply: the question was cancelled"
          : `Stand-in reply: answer "${typed}"${attached.length ? ` with ${attached.length} attachment${attached.length === 1 ? "" : "s"}: ${attached.join(", ")}` : ""}`;
      } else if (/^read the thread\s+\S+/i.test(first)) {
        const [, threadId, seconds] = /^read the thread\s+(\S+)(?:\s+for\s+(\d+)s)?/i.exec(first) ?? [];
        update(sessionId, { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: "**Checking the other thread**\n\nIt holds the plan this turn follows." } });
        const toolCall = { toolCallId: `read-${Date.now()}`, title: "t3_thread_read", kind: "other", rawInput: { threadId } };
        update(sessionId, { sessionUpdate: "tool_call", ...toolCall, status: "in_progress" });
        await Bun.sleep(Number(seconds ?? 0) * 1000);
        update(sessionId, { sessionUpdate: "tool_call_update", toolCallId: toolCall.toolCallId, status: "completed", rawOutput: { threadId } });
        text = "Stand-in reply: read the other thread";
      } else if (/^wait on the thread\s+\S+/i.test(first)) {
        const [, threadId, seconds] = /^wait on the thread\s+(\S+)(?:\s+for\s+(\d+)s)?/i.exec(first) ?? [];
        const toolCall = { toolCallId: `wait-${Date.now()}`, title: "t3_thread_wait", kind: "other", rawInput: { threadId } };
        update(sessionId, { sessionUpdate: "tool_call", ...toolCall, status: "in_progress" });
        await Bun.sleep(Number(seconds ?? 0) * 1000);
        update(sessionId, { sessionUpdate: "tool_call_update", toolCallId: toolCall.toolCallId, status: "completed", rawOutput: { threadId } });
        text = "Stand-in reply: the other thread finished";
      } else if (/^link the thread\s+\S+/i.test(first)) {
        const [, threadId] = /^link the thread\s+(\S+)/i.exec(first) ?? [];
        text = `Stand-in reply: the plan continues in [the other thread](t3-thread://v1/${threadId}).`;
      } else if (/^show the markdown sample/i.test(first)) {
        const [, seconds] = /pausing\s+(\d+)s/i.exec(first) ?? [];
        update(sessionId, { sessionUpdate: "agent_message_chunk", content: { type: "text", text: MARKDOWN_SAMPLE[0] } });
        await Bun.sleep(Number(seconds ?? 0) * 1000);
        text = MARKDOWN_SAMPLE[1];
      } else if (/^fail the turn/i.test(first)) {
        throw { code: -32603, message: JSON.stringify({ error: { message: "The stand-in could not finish this turn.", code: "fixture_failure", status: 500 } }) };
      } else if (cwd && /^write \d+ notes?\b/i.test(first)) {
        const [, count, seconds] = /^write (\d+) notes?(?:\s+after\s+(\d+)s)?/i.exec(first) ?? [];
        await Bun.sleep(Number(seconds ?? 0) * 1000);
        const batch = join(cwd, `notes/batch-${Date.now()}`), paths = [];
        mkdirSync(batch, { recursive: true });
        for (let index = 1; index <= Math.min(Number(count), 20); index++) { const path = join(batch, `note-${index}.md`); writeFileSync(path, `# Note ${index}\n\nWritten by the ACP stand-in.\n`); paths.push(path); }
        update(sessionId, { sessionUpdate: "tool_call", toolCallId: `write-${Date.now()}`, title: `Write ${paths.length} notes`, kind: "edit", status: "completed", locations: paths.map((path) => ({ path })) });
        text = `Stand-in reply: wrote ${paths.length} notes`;
      } else if (cwd && /^edit the notes/i.test(first)) {
        const paths = editNotes(cwd);
        update(sessionId, { sessionUpdate: "tool_call", toolCallId: `edit-${Date.now()}`, title: "Edit README.md and write the notes", kind: "edit", status: "completed", locations: paths.map((path) => ({ path })) });
      }
      update(sessionId, { sessionUpdate: "agent_message_chunk", content: { type: "text", text } });
      update(sessionId, { sessionUpdate: "usage_update", used: 12480, size: 200000 });
      return { stopReason: "end_turn" };
    },
  };

  createInterface({ input: process.stdin }).on("line", async (line) => {
    let message;
    try { message = JSON.parse(line); } catch { return; }
    if (message.id !== undefined && typeof message.method !== "string") { asked.get(message.id)?.(message); asked.delete(message.id); return; } // a reply to the stand-in's request
    if (message.id === undefined || typeof message.method !== "string") return; // notifications
    const handler = handlers[message.method];
    if (!handler) return send({ id: message.id, error: { code: -32601, message: `Method not found: ${message.method}` } });
    try { send({ id: message.id, result: await handler(message.params ?? {}) }); }
    catch (error) { send({ id: message.id, error: typeof error?.code === "number" ? error : { code: -32603, message: String(error?.message ?? error) } }); }
  });
}
