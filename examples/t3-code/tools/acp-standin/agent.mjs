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
//   "Show the italic sample"        replies with emphasis (ITALIC_SAMPLE), so a font's italic face is drawn
//   "Show the HTML image sample"    writes four PNGs under shots/ in the session's directory and replies with raw HTML
//                                   images (HTML_IMAGE_SAMPLE): one inside a line, one in a <div>, a refused source,
//                                   data-dark-src and a <picture> left open
//   "Show the HTML layout sample"   the same PNGs and more, laid out as READMEs write raw HTML (HTML_LAYOUT_SAMPLE):
//                                   top-level images side by side, a centred paragraph of images, a linked image, and
//                                   a sized image and a <picture> inside a line
//   "Show the prose sample"         replies with headings, lists, inline and fenced code, a quote, a link and a rule
//                                   (PROSE_SAMPLE), the blocks a transcript draws besides diagrams, tables and math
//   "Hold the turn"                 streams "Holding the turn." and keeps the turn running until `install.mjs
//                                   release` writes `release` beside this file, or the server cancels the turn
//                                   (`session/cancel`, as a steer or Stop does), then replies or stops "cancelled"
//   "Which effort?"                 replies with the session's reasoning effort, so a drive reads back the choice
// Each session advertises a reasoning effort select (`configOptions`, category `thought_level`; EFFORT), which the
// server lists as the model's effort option and sets with `session/set_config_option`.
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
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { crc32, deflateSync } from "node:zlib";

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

/** "Show the italic sample": emphasis with both markers, in prose and in a list. */
export const ITALIC_SAMPLE = "Italic in an answer: *the notes stay in one place*, and _each plan has an owner_.\n\n- A list item with *emphasis*\n- **Bold and *italic* together**\n";
/** "Show the prose sample": every common Markdown block once. */
export const PROSE_SAMPLE = "# Release notes\n\nThe notes now live in **one place**, with `notes/plan.md` as the index. See [the plan](https://example.com/plan).\n\n"
  + "## What changed\n\n1. The plan moved to `notes/`.\n2. Each outline has an owner:\n   - drafts stay in `notes/drafts/`\n   - reviews go to the thread\n3. Old links redirect.\n\n"
  + "### Example\n\n```ts\nexport function outline(title: string): string[] {\n  return [`# ${title}`, \"\", \"- First point\"];\n}\n```\n\n"
  + "> A quote from the review: keep the outline short, and link the plan rather than copying it.\n\n---\n\nThat is all for this release.\n";
/** "Show the HTML image sample": the raw HTML images T3 Code's sanitizer draws, its open <picture> last as a streaming
 *  reply leaves one. The sources are the PNGs SHOTS writes. */
export const HTML_IMAGE_SAMPLE = [
  "Inline: before <img src=\"shots/inline.png\" alt=\"inline\"> after.",
  "<div><img src=\"shots/div.png\" alt=\"div\"></div>",
  "<img src=\"javascript&colon;alert(1)\" alt=\"colon entity\">",
  "<img src=\"shots/light.png\" data-dark-src=\"shots/dark.png\" alt=\"data-dark-src\">",
  "<picture>\n<source media=\"(prefers-color-scheme: dark)\" srcset=\"shots/dark.png\">\n<img src=\"shots/light.png\" alt=\"unclosed picture\">",
].join("\n\n");
/** "Show the HTML layout sample": raw HTML images as READMEs lay them out, which T3 Code's sanitizer keeps: images that
 *  follow each other at the top level (on one line box, a blank line or not), a `<p align="center">` of images, an
 *  `<a href>` around an image, and an authored size and a theme's source on images inside a line. */
export const HTML_LAYOUT_SAMPLE = [
  "<img src=\"shots/a.png\" width=\"120\" height=\"48\" alt=\"first\">\n<img src=\"shots/b.png\" width=\"120\" height=\"48\" alt=\"second\">",
  "<img src=\"shots/c.png\" alt=\"third\">",
  "Between the rows.",
  "<p align=\"center\">\n<img src=\"shots/a.png\" width=\"60\" height=\"24\" alt=\"centred a\">\n<img src=\"shots/b.png\" width=\"60\" height=\"24\" alt=\"centred b\">\n</p>",
  "<a href=\"https://example.com/docs\"><img src=\"shots/c.png\" width=\"80\" height=\"32\" alt=\"linked\"></a>",
  "Sized: <img src=\"shots/inline.png\" width=\"16\" height=\"16\" alt=\"sized\"> and themed: <picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"shots/dark.png\"><img src=\"shots/light.png\" width=\"40\" height=\"16\" alt=\"themed\"></picture> in a line.",
].join("\n\n");
/** Each sample PNG: its size and its one colour. */
export const SHOTS = { "inline.png": [32, 32, [13, 148, 136]], "div.png": [240, 96, [234, 88, 12]], "light.png": [240, 96, [253, 230, 138]], "dark.png": [240, 96, [49, 46, 129]],
  "a.png": [240, 96, [37, 99, 235]], "b.png": [240, 96, [220, 38, 38]], "c.png": [320, 128, [22, 163, 74]] };
/** A PNG of one colour (8-bit RGB, https://www.w3.org/TR/png-3/). */
function png(width, height, rgb) {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]), out = Buffer.alloc(data.length + 12);
    out.writeUInt32BE(data.length, 0); body.copy(out, 4); out.writeUInt32BE(crc32(body), data.length + 8);
    return out;
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 2;
  const row = Buffer.alloc(1 + width * 3);
  for (let x = 0; x < width; x++) row.set(rgb, 1 + x * 3);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", header),
    chunk("IDAT", deflateSync(Buffer.concat(Array.from({ length: height }, () => row)))), chunk("IEND", Buffer.alloc(0))]);
}
/** The reasoning effort select each session advertises: three levels drawn as bars, and Max, which is named. */
export const EFFORT = { id: "reasoning_effort", name: "Reasoning effort", description: "How long the stand-in thinks", category: "thought_level", type: "select",
  currentValue: "medium", options: [{ value: "low", name: "Low" }, { value: "medium", name: "Medium" }, { value: "high", name: "High" }, { value: "max", name: "Max" }] };

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
  const efforts = new Map(); // session id → its reasoning effort
  const cancelled = new Set(); // sessions whose running turn the server cancelled
  const configOptions = (sessionId) => [{ ...EFFORT, currentValue: efforts.get(sessionId) ?? EFFORT.currentValue }];
  const releaseFile = join(here, "release");
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
      return { sessionId, configOptions: configOptions(sessionId) };
    },
    "session/load": ({ sessionId, cwd }) => {
      if (methods.length > 0 && !signedIn()) throw authRequired;
      if (typeof cwd === "string") sessions.set(sessionId, cwd);
      advertise(sessionId);
      return { configOptions: configOptions(sessionId) };
    },
    "session/set_config_option": ({ sessionId, configId, value }) => {
      if (configId !== EFFORT.id || !EFFORT.options.some((option) => option.value === value)) throw { code: -32602, message: `Unknown option ${configId}=${value}` };
      efforts.set(sessionId, value);
      return { configOptions: configOptions(sessionId) };
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
      } else if (/^answer, then check/i.test(first)) {
        update(sessionId, { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "Stand-in reply: the answer comes before its check." } });
        const toolCall = { toolCallId: `check-${Date.now()}`, title: "Check the notes", kind: "read", rawInput: { path: "README.md" } };
        update(sessionId, { sessionUpdate: "tool_call", ...toolCall, status: "in_progress" });
        update(sessionId, { sessionUpdate: "tool_call_update", toolCallId: toolCall.toolCallId, status: "completed" });
        update(sessionId, { sessionUpdate: "usage_update", used: 12480, size: 200000 });
        return { stopReason: "end_turn" };
      } else if (/^show the italic sample/i.test(first)) {
        text = ITALIC_SAMPLE;
      } else if (cwd && /^show the html (image|layout) sample/i.test(first)) {
        mkdirSync(join(cwd, "shots"), { recursive: true });
        for (const [name, [width, height, rgb]] of Object.entries(SHOTS)) writeFileSync(join(cwd, "shots", name), png(width, height, rgb));
        text = /^show the html image/i.test(first) ? HTML_IMAGE_SAMPLE : HTML_LAYOUT_SAMPLE;
      } else if (/^show the prose sample/i.test(first)) {
        text = PROSE_SAMPLE;
      } else if (/^which effort\?/i.test(first)) {
        text = `Stand-in reply: effort ${efforts.get(sessionId) ?? EFFORT.currentValue}`;
      } else if (/^hold the turn/i.test(first)) {
        update(sessionId, { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "Holding the turn.\n\n" } });
        cancelled.delete(sessionId);
        rmSync(releaseFile, { force: true }); // a release left from an earlier hold is not this one's
        while (!existsSync(releaseFile) && !cancelled.has(sessionId)) await Bun.sleep(200);
        if (cancelled.delete(sessionId)) return { stopReason: "cancelled" };
        rmSync(releaseFile, { force: true });
        text = "Stand-in reply: released";
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
    if (message.method === "session/cancel") { cancelled.add(message.params?.sessionId); return; }
    if (message.id === undefined || typeof message.method !== "string") return; // notifications
    const handler = handlers[message.method];
    if (!handler) return send({ id: message.id, error: { code: -32601, message: `Method not found: ${message.method}` } });
    try { send({ id: message.id, result: await handler(message.params ?? {}) }); }
    catch (error) { send({ id: message.id, error: typeof error?.code === "number" ? error : { code: -32603, message: String(error?.message ?? error) } }); }
  });
}
