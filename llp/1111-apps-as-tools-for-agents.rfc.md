# LLP 1111: Apps as tools for a person's agent — an MCP server and a connector, with the plumbing done for you

**Type:** RFC
**Status:** Draft r3 (2026-10-07): round 2 by Astra (max) and Grok 4.7 (xhigh), both NOT READY; r3 specifies only the local MCP slice for Fieldnotes; the other doors are direction (`llp/reviews/1111-apps-as-tools-for-agents.{astra,grok}.md`; dispositions in §9). r2 (2026-10-06) folded in r1 and Charlie's direction: "when you make an app with exact2, you get an MCP server for it and connectors for free (if you want them)", softened to "or maybe not for free but you get a lot of things that make life easy for you, the same way as the cli." It proposes; it specifies nothing until it has an implementer and a date.
**Renumbered (2026-10-08):** drafted and reviewed as LLP 1105, a number origin had meanwhile given another document; its reviews keep the old numbers (1105→1111, 1106→1112, 1107→1113). Code citations were checked at `2ce193976`, 542 commits behind the commit that adds this file.
**Parked (2026-10-07), after round 3** (Astra and Grok 4.7 both NOT READY; their round-3 sections are in the review files). The remaining blockers are invocation completion, certainty values, app-wide serialization and the Fieldnotes tests racing the list refresh, and most of them follow from what a call owns. LLP 1113 (settled and the two clocks) now owns that rule; r4 replaces D4's ownership text with a citation of LLP 1113 D4 under D5's tool-call column, then takes the round-3 reviews.
**Systems:** Contract (the `tool` line; a lint of each tool's consequence and of the commands it reaches), Plan (a `tools` table, one format-version bump), Runner (`runner/src/tools.rs`: a call's lifecycle and ownership), the app manifest (`app.json` `tools`), Apple host (macOS: Allow Agents, the socket, the session rule, the consent alert), Tooling (`exact tools <app>`, the stdio relay; the deploy dry run; `scripts/agent.mjs`, the authored-test `call` and `consent` steps; the `exact new` verb), `rules/DEFERRED.md` (a feature admission on an existing host)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Implementer:** none yet.
**Related:**
- LLP 1012 (the agent API: development only, ten operations; §6's "no server", `llp/1012-agent-api-v1.spec.md:541`; line 318, an agent never invokes an action by name), LLP 1069.007 D2 (agent mode only in a development bake), LLP 1069.007 D4 (held device requests, `tap @N`).
- LLP 1069.008 (grants), LLP 1018 (the store; D5, the bearer-token pattern), LLP 1016 (requests in flight; D5, a sent `POST` is not unsent), LLP 1054.000.000 D3 (a kept ticket), LLP 1089 (action composition), LLP 1092 (queued sends), LLP 1097 (background storage), LLP 1030 D3a and 1030.000 (the compatibility id, the deploy table).
- Siblings. LLP 1101.003 (the command line): the model for this RFC's framing. LLP 1112: D1 **a URL under a state**, D1a the whole-tree *settled* procedure (which a tool call does not use: it tracks its own invocation, D4), D10 the anonymous mode, D11 its deadlines.
- `rules/DEFERRED.md` §Agent API, §Features carried over as "no" (server-driven UI stays refused).
- External: MCP revision 2025-11-25 ([tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), [schema and annotations](https://modelcontextprotocol.io/specification/2025-11-25/schema#toolannotations), [cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation), [transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [authorization](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)); WebMCP ([spec](https://webmachinelearning.github.io/webmcp/), [implementation status](https://github.com/webmachinelearning/webmcp/blob/main/implementation-status.md), [WebKit's position](https://github.com/WebKit/standards-positions/issues/670)); Apple's App Intents.

## Summary

The aim: an app made with Exact opts in and is reachable by a person's agent
through MCP. It can be a local server on the person's Mac, later a remote
connector that a service such as claude.ai adds by URL, and later a WebMCP
page. Exact does the plumbing and the app reuses what it already has, as
LLP 1101.003 does for the command line, where routes become argv, `--help`
comes from the route table, and the app's own actions do the writes.

**What you get.** Slice 1, specified here, gives you:
- the transport: `exact tools <app>`, a stdio MCP server relaying to the
  running app;
- `tools/list` and the input and output schemas, generated from the plan's
  types;
- a call path that runs the app's own action and answers with what
  happened and how sure that is (D4);
- consent drawn by the host (D5);
- a deploy listing of everything each tool reaches (D3).

The remote connector, WebMCP, routes as resources and App Intents are
direction (§4).

**What you write:** one line per tool, and a key in `app.json`.

**Slice 1 is Fieldnotes on macOS**, whose notes are local SQLite that only a
local door can reach. The whole change, D11's complete declarations:

```contract
+ tool findNotes reads closed "Search the notes by title and text" = search returns library fails when not library.ready
+ mutation created as shape Saved
+ derive createFailed = match created { case some(s) => s.failed, case none => false }
+ action create(title: string, body: string)
+   send created = saveNote("", title, body, false, 0)
+ tool addNote writes additive closed "Create a note with this title and body" = create returns created fails when createFailed
+ mutation deleted as shape Status
+ derive deleteFailed = match deleted { case some(s) => s.failed, case none => false }
+ action deleteById(id: string)
+   send deleted = deleteNote(id)
+   if id == editingId
+     session = dirty ? Session(nextSession("", session), draft=some(fields)) : nextSession("", session)
+ tool deleteNote destroys closed "Delete the note with this id" = deleteById returns deleted fails when deleteFailed
```

and `"tools": { "mcp": true }` in `app.json`. That is three tools in twelve
lines. Nine of the lines are the two write actions and their mutations,
which keep an agent's write off the editor's `saved` slot (D2). A read
costs one line.

## 1. Where this sits

Verified in the code at `2ce193976`:

- **The GUI agent channel is development apparatus**, sealed in production
  (`ExactKit/Session.swift:44-54`; `host/linux/src/app.rs:145-151`,
  `:487-504`; `host/web-js/build.mjs:346-352`). The terminal's headless
  operations run outside that gate (`host/terminal/src/cli.rs:176`).
- **exact2 has no MCP server; LLP 1012 §6 refused one.** It refused exact1's
  development server (`~/projects/exact/.mcp.json` →
  `packages/exact-devtools/src/agent/mcp-stdio.ts`), which LLP 0150 measured
  at 2,932 lines, a third hand-maintained edit site per tool. This server
  serves an app's declared production tools to the person's agent, never
  the development driver. It is generated from the plan's tool table, so it
  adds no edit site.
- **Action parameter types are inferred** before lowering and written into
  the plan (`contract/types/src/component.rs:208`, `contract/lower/src/lib.rs:346`).
- **The runner replaces requests three ways** (`forget`, `enqueue`'s drop,
  and its kept ticket; D4's table cites each), and a resource can keep a
  standing value while a request is in flight (`settlement.rs:421-463`).
- **A Mac app has a session per window** (`ExactMac/main.swift:132`, `:161`).
  `frontWindow()` falls back from key to main to newest (`:328`). Builds
  are unsandboxed (`DocumentPickers.swift:11`). App data under
  `~/Library/Application Support/exact/<id>/data` (`NativeModule.swift:468-470`)
  is readable by any same-user process.
- **Fieldnotes** keeps notes in SQLite (`app.ts:29`) on a worker. Its sources
  report failure as data: `library` as `ready:false` (`:140`), `saveNote` and
  `deleteNote` as `failed:true` (`:158`, `:201`).
- **`HOST_COMMANDS`** (`contract/syntax/src/lib.rs:83-146`) has 32 names.

## 2. When this is worth it

If the app's backend has an API, that backend's own MCP server is the
better door. Exact's door earns its place where the app is the door: the
data is local (Fieldnotes), the rules live in client actions, or the app
holds the person's session. A view's guard does not carry over. Fieldnotes'
save button is `disabled=(busy or not dirty …)` (`app.contract:261`), while
`action save` sends unconditionally (`:115`). A tool binds an action whose
preconditions are inside it.

## 3. Decisions

### D1 — Tools are declared

Exact does not turn the screen into tools. A button knows a label, not a
meaning; consequence can't be derived; and the platforms' agents already
read the screen through the accessibility tree, so a second reading would
be the parallel projection `runner/src/agent.rs:6-7` warns against. Only the
schemas are derived: input from the bound action's parameters, output from
`returns`.

### D2 — A tool is one line naming a root action

```contract
tool <name> <reads | writes [additive] | destroys> [idempotent] [closed] "<description>" = <action> [returns <expr>] [fails when <expr>] [available when <expr>]
```

- **It names an existing root action.** The parameters, their names and
  their types come from it. The dry run marks a schema that changed.
- **`returns`, `fails when`, `available when`** are expressions over the
  root's slots, derives, resource and mutation values, `pending(x)`, and
  `failed(x)` for a resource only (`docs/contract-grammar.md:565`). A
  mutation reports failure in its value, as Fieldnotes' `s.failed` does.
  `available when` is evaluated after every commit. While it is false the
  tool is not listed, and a call is refused (`unavailable`).
- **Root only in v1:** the root has one instance per session.
- **A tool-shaped action** is written when a write would share the editor's
  state. An agent's save through Fieldnotes' `saved` would switch the note
  being edited (`app.contract:79-80`), and the person's Save would
  supersede it (D4, trace 2). Such an action sends into a mutation only it
  uses.
- **The modifiers** are the author's assertions for MCP's annotations (D3):
  `additive` (adds, never overwrites), `idempotent` (a repeat has no further
  effect), `closed` (touches only the app's own data).
- Syntax is the compiler's to settle (LLP 1006).

### D3 — Consequence is the author's word, linted and listed

`reads`, `writes` or `destroys`, with no default.

- **The reach** is the action and everything it reaches through props,
  `then` and LLP 1089 calls. It also includes the actions of `task … when`
  gates that read a slot the reach writes. Their commits are not the
  invocation's (D4), but their effects follow from it.
- **The lint.** In every tool, a command D4a refuses fails the compile,
  naming the line. A `reads` tool also fails on a store write or a run-class
  command other than `preventDefault` and `stopPropagation`. Skip-class
  commands are allowed everywhere. A `send` is allowed in `reads`; what a
  source does is the module's business (Fieldnotes' `library` can run
  `CREATE TABLE`, `app.ts:28-36`). So `reads` is a statement, not a proof.
- **The listing.** The dry run (LLP 1030.000) prints each tool's name,
  consequence, modifiers, bound action and schema (marked if changed), and
  the sends, sources, gated tasks and commands in its reach. A reviewer
  reads the reach, not the adjective.
- **MCP annotations, with MCP's meanings and defaults**
  (`readOnlyHint: false`, `destructiveHint: true`, `idempotentHint: false`,
  `openWorldHint: true`):

| Exact | `readOnlyHint` | `destructiveHint` | `idempotentHint` | `openWorldHint` |
|---|---|---|---|---|
| `reads`, no `send` in the listed reach | `true` | — | — | default unless `closed` |
| `reads` with a `send` | `false` | `false` | default unless `idempotent` | default unless `closed` |
| `writes` | `false` | `false` only with `additive` | default unless `idempotent` | default unless `closed` |
| `destroys` | `false` | `true` | default unless `idempotent` | default unless `closed` |

Grants do not decide `openWorldHint`: a search reaching one granted API is
still open-world. `findNotes` reaches a resource and no `send`, so it is
`readOnlyHint: true`. All three Fieldnotes tools are `closed`.

### D4 — A call's lifecycle

A door hands the runner `{tool, args}` on a connection bound to a session
(D7). **Calls are serialized per session:** while one is between dispatch and
terminal, another is refused (`busy`). Queueing would pile up consent and
widen every race below.

**Phases:** validate → awaiting consent (`destroys` only) → dispatched →
completing → terminal.

1. **Validate.** The tool must be listed and available.
   - The arguments are decoded by a new walker over the plan type
     (`runner/src/tools.rs`). It rejects unknown fields, missing fields and
     non-finite numbers, maps `null` to `none` for `option<T>` (which stays
     required), and reports the first failing path. `Value::conforms`
     (`plan/src/value.rs:240`) only answers yes or no.
   - **A call whose `returns` or `fails when` reads a pending resource is
     refused (`busy`), not joined.** The standing value may answer an older
     ask (`settlement.rs:421-463`).
2. **Awaiting consent** (D5), outside the time bound.
3. **Dispatched.** The action runs as one event with an invocation id, on
   the press path. The journal line is `tool <name> <phase|outcome>`, never
   the arguments.
4. **Completing.** The 10 s bound starts at dispatch, on the wall clock. The
   person's events keep committing; the runner never blocks.
   - **Owned requests:** every request enqueued by the invocation's commit,
     or by a commit that fulfils an owned request (its `then`, LLP 1089; its
     queued sends, LLP 1092). A resource ask is owned only for a resource
     that `returns` or `fails when` reads. Other re-answers the writes cause
     are not waited for.
   - **Done** when every owned request has settled. `Answer::Now` settles in
     its own commit. An action that enqueues nothing (a search for the query
     already shown) is done at its commit.
   - **Refused in v1:** an owned request answered with a stream, or a source
     scheduling LLP 1097 background work while answering one
     (`runner/src/runner/background.rs`). That ends `failed` (`stream` or
     `background`, `uncertain`).
5. **Terminal.** `returns` and `fails when` are evaluated once, at the commit
   where the last owned request settled.

**Outcomes.** Each carries a **certainty**: `none` (the action did not run),
`applied` (it ran and its owned requests settled), or `uncertain` (it ran;
an effect may or may not have landed).

| outcome | when | certainty |
|---|---|---|
| `ok` | done; `fails when` false | `applied` |
| `failed` | `fails when` true; an owned request failed; a stream or background work | `uncertain` |
| `superseded` | another commit dropped an owned request (`enqueue` `:997-1001`, `forget` `:933-940`) or moved its kept ticket (`:964-995`) | `uncertain` for a send (LLP 1016 D5); `applied` for a resource ask, whose value now answers someone else |
| `timeout` | 10 s after dispatch | `uncertain` |
| `interrupted` | after dispatch: the plan changed, the window closed, Allow Agents went off | `uncertain` |
| `refused` | `unavailable`, `arguments`, `busy`, `declined`, `consent-timeout`; after dispatch, `person-required` (D4a) | `none` before dispatch; `uncertain` after |

- **`stale: true`** is added to `ok` or `failed` when another commit wrote a
  slot that `returns` or `fails when` reads, or re-asked a resource they
  read, between dispatch and terminal.
- **Cancellation** (`notifications/cancelled`) and **disconnect** get no
  response, as MCP specifies. While awaiting consent, the alert is
  withdrawn and the action never runs. Later, nothing is undone; the journal
  records `uncertain`.
- **Access off, window closed, plan change:** a pending confirmation is
  withdrawn (`refused`, `none`). A dispatched call ends `interrupted`, with a
  response before the socket closes.
- **Envelope.**
  - `structuredContent` is `{outcome, certainty, reason, stale, skipped,
    value, message}`; it is also the `outputSchema`, with `value` typed from
    `returns`.
  - `value` is `returns` encoded as `state` encodes values
    (`runner/src/agent.rs:1091-1128`), or `null`. A `failed` call still
    carries it, so the agent reads the app's message.
  - `content` repeats it as one text block, and `isError` is `true` for
    every outcome but `ok`. An unknown tool name is JSON-RPC `-32602`.
  - The message for `uncertain` says to read before calling again.

**Four traces** (`content` elided).

*1. An identical search already pending.* The person typed "rain", and
`library` is in flight:

```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"findNotes","arguments":{"value":"rain"}}}
{"jsonrpc":"2.0","id":7,"result":{"isError":true,"structuredContent":{"outcome":"refused","certainty":"none","reason":"busy","stale":false,"skipped":[],"value":null,"message":"library is answering another request; call again when it has answered."}}}
```

*2. A superseded write: why Fieldnotes does not bind `save`.* A tool
`saveCurrent writes = save returns saved` is dispatched. While its
`saveNote` is in flight the person presses Save, whose commit sends `saved`
again, and `enqueue` drops the owned ticket:

```json
{"jsonrpc":"2.0","id":8,"result":{"isError":true,"structuredContent":{"outcome":"superseded","certainty":"uncertain","reason":"saved was sent again by the person's Save","stale":false,"skipped":[],"value":null,"message":"Your save may have been written. Read the note before calling again."}}}
```

*3. Cancellation during confirmation.* `deleteNote {"id":"12"}` shows the
alert, then the client sends
`{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":9}}`.
The alert closes, `deleteById` never runs, request 9 gets no response, and
the journal reads `tool deleteNote cancelled none`.

*4. Consent denied.* The person presses Don't Delete:

```json
{"jsonrpc":"2.0","id":10,"result":{"isError":true,"structuredContent":{"outcome":"refused","certainty":"none","reason":"declined","stale":false,"skipped":[],"value":null,"message":"The person declined."}}}
```

### D4a — The command vocabulary

Every `HOST_COMMANDS` entry has a class.
- **Refuse:** reaching it fails the compile (D3).
- **Skip:** not run when an invocation's commit issues it, and listed in
  `skipped`.
- **Run:** as on a press.

| class | commands |
|---|---|
| refuse | `copyText`, `deliveryActivate`, `deliveryCheck`, `format`, `openURL`, `reload`, `setScheme`, `share`, `showPicker`, `showOpenFilePicker`, `showDirectoryPicker`, `showSaveFilePicker`, `saveFile`, `showModal`, `showNotification`, `closeNotification`, `playSound`, `playSounds`, `close` |
| skip | `focus`, `blur`, `scrollIntoView`, `selectText`, `setSelectionRange`, `fastSeek`, `load`, `haptic` |
| run | `postMessage`, `stopSounds`; `preventDefault`, `stopPropagation` (no-ops: a call has no event) |

- **The rule behind the classes.** Refused commands act in the person's
  name, wait for the person, or reach outside the app. Skipped ones move the
  person's view.
- **A name not in the table is refused,** and a test fails when
  `HOST_COMMANDS` gains a name without a row.
- **At run time,** a data-module request that needs the person
  (`openAuthSession`, LLP 1069.006; a permission prompt, LLP 1069.008),
  made while answering an owned request, is refused to the source. The call
  ends `refused` (`person-required`, `uncertain`), and the module's later
  value is not returned.

### D5 — Consent is the host's

**The threat model.** A tool caller is trusted to call the tools it is
offered, and nothing more. A `destroys` call needs a yes from the person
that the caller cannot give. Exact does not defend against software that
already runs as the person with full access, which could press Delete or
open the database without tools.

- **One confirmation:** an app-modal `NSAlert` outside the Exact tree.
  - It names the app, the window, the tool, its description and the
    decoded arguments, and what was shown is what runs.
  - It is bound to the invocation, its session and its plan generation.
    Availability is checked again after the yes.
- **App-modal only while asking.** While the alert is up the app takes no
  other input, and answers from the worker wait for it to close. The wait
  after dispatch is not modal.
- **No answer in 60 s** closes it: `refused` (`consent-timeout`, `none`).
  Escape declines.
- **Elicitation is not consent:** a client's yes proves nothing about a
  person. **"Always allow"** is not in v1.

### D6 — A tool reaches only what the app reaches

- **The grants do not change** (LLP 1069.008).
- **Secrets.** The store is not exposed automatically: `returns` reads root
  state, not the store. Sources that follow LLP 1018 D5 keep a bearer token
  out of their values. Nothing stops a source returning a secret, and one
  returning `store.get("token")` is tested behaviour
  (`runner/tests/it/now_screen.rs:84`, `:141`). The author owns what
  `returns`, `fails when` and messages expose, for every secret.
- **The tool table is plan data, not part of the compatibility id.** One
  format bump admits it and feeds the id once (LLP 1030 D3a).

### D7 — The local door

`app.json` gains `"tools": {"mcp": true}` (`scripts/app.schema.json`), off by
default. Tools declared without it ship the table and no door.

- **Allow Agents** is a host-owned App menu item, off by default and kept per
  app. While it is off the app does not listen. While it is on, the window
  shows it and shows a connected client.
- **The socket** is `~/Library/Application Support/exact/<id>/tools.sock`,
  in a `0700` directory, mode `0600`. If that passes `sun_path`'s 104 bytes,
  it is `/tmp/exact-<uid>/<first 16 hex of SHA-256(id)>.sock`, in a `0700`
  directory the app checks it owns.
- **What it adds.** A same-user process can already open the SQLite file.
  The socket adds running the declared tools under the app's grants and
  sign-in, while access is on. During that time any same-user process can
  call every tool not marked `destroys`, and the app's About Agents text
  says so.
- **The session rule.** A session is eligible when its window is open and its
  plan has a tool table. A connection binds when it opens, in this order:
  1. the one eligible session, if there is just one;
  2. else the window named by `exact tools <id> --window <label>`
     (`--windows` lists each window's label and title);
  3. else the session most recently key. The app records it on each
     `windowDidBecomeKey`, so the rule never reads current focus. This
     follows `frontWindow()`'s fallback (`main.swift:328`).

  The binding is fixed for the connection's life, and the window shows it.
  When the window closes, its calls end (D4) and the connection closes. A
  plan change sends `notifications/tools/list_changed`. **To demonstrate:**
  Claude Code in Terminal, in front; Fieldnotes behind it, inactive, with
  two windows. The connection binds to the one used last, and a call
  works.
- **The relay.** `exact tools <id>` is the stdio server a client is
  configured with, relaying bytes to the socket. It never launches the app
  (§8 Q1). If the app is not running or access is off, it says which on
  stderr and exits 1.
- **Transport profile.**
  - *Framing.* MCP stdio: UTF-8 JSON-RPC 2.0, one message per line, no
    embedded newline, logs on stderr only. The socket carries the same.
  - *The server.* The app answers `initialize` with its name, version and
    `tools: {listChanged: true}`.
  - *Bounds.* A message over 4 MiB is refused: incoming with `-32600`, an
    outgoing result as `failed` (`too-large`) at its certainty. At most four
    connections; a fifth is closed, and the relay says so.
  - *Cancellation* is routed by connection and request id.
- **`exact new`.** The generated `verbs` table (`game/new.mjs`, beside
  `feedback`) gains `tools: ['scripts/exact.mjs', 'tools', '<name>']`.
  Claude Code is configured with
  `claude mcp add fieldnotes -- bun /path/to/fieldnotes/exact.mjs tools`, or
  in this repository
  `claude mcp add fieldnotes -- bun scripts/exact.mjs tools com.exact.fieldnotes`.

### D8 — The remote connector (direction): §4.

### D9 — Routes as resources (direction): §4.

### D10 — Not agent mode, not an operation, not server-driven UI

- **Production path:** no agent clock, device-fact substitutes or empty
  store. LLP 1069.007 D2 stands.
- **Amend LLP 1012:318:** *a declared tool (LLP 1111) is the one production
  path that invokes an action by name, and only an action the author bound
  in a `tool` line.*
- **Outside the ten operations.** The driver reaches tools through the ones
  it has:
  - `tree` adds `tools[]` to the root's record.
  - `tap <root> {"tool":"findNotes","args":{"value":"rain"}}` enters D4 as a
    commit with an invocation id, as `Runner::act` dispatches. It replies at
    terminal, or at awaiting consent with
    `{phase: "awaiting consent", hold: "consent"}`.
  - Under agent mode the confirmation is a held device request named
    `@consent` (LLP 1069.007 D4), answered with `tap @consent yes|no`. It is
    not in `state.pending`, so `clock settle` never waits on a person, and its
    reply says `delivery: "tool"`, not `"substituted"` (`agent.rs:208-214`).
  - `state` gains `tools.last`: the last invocation's phase and outcome
    object.
- **Authored tests gain three steps.**
  - `call <tool> {IDENT "=" test-value}` returns at terminal or at the hold.
    `scripts/agent-test.mjs:222` awaits each step, so a call that waited
    for its consent would deadlock; returning at the hold lets the next step
    answer it.
  - `consent (yes|no)` answers the hold and returns at terminal.
  - `expect call <field> {"." field} == test-value` reads `tools.last`.
- **Not server-driven UI.** No UI crosses the wire.

### D11 — Fieldnotes: declarations, delete policy, tests

**Declarations:** the Summary's twelve lines, plus two prerequisites.
- **`library` re-answers after a tool's write:**
  `derive toolRevision = createdRevision + deletedRevision` (two derives over
  `created` and `deleted`, shaped as `saveRevision` is), and
  `resource library = library(query, saveRevision, serviceRevision, toolRevision)`.
  The source reads only `[query]`.
- **Tests seed by calling `addNote`** into agent mode's scratch `app:/data`
  (`NativeModule.swift:464-466`). Each test's store is fresh
  (`scripts/agent-test.mjs:107`), so the first id is `"1"`. LLP 1112 D1 has
  no store snapshot.

**Deleting the note being edited.**
- *A clean editor:* the session ends, as the UI's `remove` ends it.
- *A dirty editor:* the draft is kept as a new note. `noteId` and `savedId`
  are cleared and `draft` keeps the text, so the person's next Save inserts
  it instead of failing with "This note was deleted" (`app.ts:149`).
- *Another note:* the editor is left alone.

**Overlapping writes** can't happen between tools, which D4 serializes. A
tool's write and the person's Save use different mutations, and `serial()`
in the source orders them at the database.

**Tests** (`apps/fieldnotes/app.test.contract`):

```text
test "an agent's delete asks, and a no keeps the note"
  call addNote title="Rain" body="It rained at noon."
  expect call outcome == "ok"
  call deleteNote id="1"
  expect call phase == "awaiting consent"
  consent no
  expect call outcome == "refused"
  expect call reason == "declined"
  expect call certainty == "none"
  call findNotes value="Rain"
  expect call value.total == 1

test "a yes deletes it"
  call addNote title="Rain" body="It rained at noon."
  call deleteNote id="1"
  consent yes
  expect call outcome == "ok"
  expect call certainty == "applied"
  call findNotes value="Rain"
  expect call value.total == 0

test "deleting the note being edited keeps a dirty draft as a new note"
  call addNote title="Rain" body="It rained."
  tap "note-1"
  type "note-body" "It rained all day."
  call deleteNote id="1"
  consent yes
  expect state session.noteId == ""
  expect text "note-body" == "It rained all day."
  tap "save-note"
  call findNotes value=""
  expect call value.total == 1

test "deleting the clean note being edited clears the editor"
  call addNote title="Rain" body="It rained."
  tap "note-1"
  call deleteNote id="1"
  consent yes
  expect text "note-title" == ""

test "a call while library is still loading is busy"
  before data
  call findNotes value="Rain"
  expect call reason == "busy"
```

The last test needs `library` to still be answering its boot request
(`before data`). Where boot answers first, it waits on LLP 1103's fault
table growing a hold; until then it is a manual check.

**The proof.** A person's Claude Code session, configured as in D7, finds,
adds and deletes a note while the person watches, with Fieldnotes behind
Terminal. The alert is shown for the delete.

**The admission.** One feature on an existing host (macOS), recorded among
`rules/DEFERRED.md`'s feature admissions, not as a §Surfaces line. It names
no take; it asks Charlie for a waiver or a take (§8 Q4). An implementer and
a date are still owed.

## 4. Later doors (direction, not specified)

Each needs its own revision and consumer before it is built.

- **WebMCP** (`document.modelContext.registerTool`). Chrome and Edge run
  origin trials, and WebKit has recorded opposition. The JS target has its
  own runtime (`host/web-js/rt.js:162`, `host/web-js/src/emit.rs:428`), so D4
  is built twice, with a Rust parity case. Open questions:
  - discovery: `available when` can't be evaluated before an
    interaction-activated `app.js` builds the state (`build.mjs:273`,
    `capture.js:1`), so it is a static catalogue checked at `execute`, or
    activation before registering;
  - no `destroys`, because a page can't learn whether `consequentialHint`
    is honoured;
  - `untrustedContentHint`'s default.
- **The anonymous connector (Caltrain).** Two `tool` lines and
  `"connector": "anonymous"`, served over Streamable HTTP on the render
  server beside LLP 1112 D10's anonymous mode. Open problems:
  - departures are fixed to 2026-08-28 in the data crate (`DAY_START_MS`,
    `apps/caltrain/data/src/lib.rs:118`, used at `:189`), so a wall clock
    alone empties the board; a labelled simulation or real data must be
    chosen;
  - a public front door: the server binds loopback (`serve.rs:155`), and
    LLP 1082 is a Draft;
  - request bodies and `Origin` validation: `POST` is refused today
    (`serve.rs:513`);
  - a hard deadline is LLP 1112 D11's 503 with no MCP body;
  - the launch-record fields a call needs (LLP 1112 D9), and the child's
    result channel.

  MCP's authorization is optional, so an anonymous server is allowed.
- **Routes as resources.** A template per route, read as a URL under a state
  (LLP 1112 D1), settled per LLP 1112 D1a, and narrowed to `resources` and
  `derives`. Open: the Mac windowless session that would read the person's
  data has no stage, and Caltrain's station is a slot, not part of its URL.
- **The signed-in connector** (Interview, or the Bluesky client outside the
  repo). It needs:
  - a confidential OAuth client registered with the app's provider;
  - server-side storage of each person's backend token;
  - per-person instances.

  MCP forbids forwarding the client's token. LLP 1018's tiers and
  `openAuthSession` (LLP 1069.006) stay on the device. There is no
  `destroys`, because there is no window to ask in.
- **App Intents** are compiled per binary and can run in an extension. A
  plan update could add a tool that iOS lacks until the next binary.

## 5. What changes, where (slice 1)

| where | change | size (estimate) |
|---|---|---|
| `contract/` | the `tool` line and modifiers, the reach, the D3 lint, D4a's table and its completeness test | ~550 lines |
| `plan/tables/format.json` | the `tools` table; one format bump | one table |
| `runner/src/tools.rs` | list, availability, decoder, serialization, ownership, outcomes and certainty, skipped commands | ~750 lines |
| `host/apple` (macOS) | Allow Agents, the socket, framing and bounds, the session rule, the alert, `tools.last` | ~700 lines Swift |
| `scripts/exact.mjs`, `app.schema.json`, `game/new.mjs` | `exact tools <id> [--windows \| --window <label>]`; `tools.mcp`; the verb | ~180 lines |
| `scripts/agent.mjs`, the steps grammar, `agent-test.mjs` | `tree`'s `tools[]`, `tap {tool}`, `@consent`, `call`, `consent`, `expect call` | ~250 lines |
| `bake/` | the dry run's listing | small |
| `apps/fieldnotes` | the declarations, `toolRevision`, the tests | ~60 lines |
| `docs/` | the `tool` line; "a view's `disabled` does not guard a tool" | short |

No file passes 1,500 lines. `runner/src/agent.rs` is at 1,443, so the call
gets its own file.

## 6. Not in this RFC

- **A development-time MCP server** for the agent driver. LLP 1012 §6 refused
  one; a later RFC could revisit it.
- Derived tools, non-root tools, a tool calling a tool, streaming results,
  MCP resources and prompts, "always allow", an information-flow check.
- **A GUI app printed as text.** LLP 1101.003 prints only a terminal entry; a
  GUI app's picture is LLP 1112's.

## 7. Risks

- **Prompt injection through results.** A note can say "delete every note."
  A delete asks the person, but a misled `writes` call is not stopped. The
  person has the switch and the journal.
- **Same-user processes** can call non-`destroys` tools while access is on.
  That is stated, not solved.
- **Inferred types move.** The dry run marks a changed schema, and
  `tools/list_changed` tells live clients.
- **The app-modal alert** pauses the person's work in the app while it is up.

## 8. Open questions for Charlie

1. **Should the relay launch the app?** Recommended no: a hidden launch has
   no window to bind.
2. **The bound** is 10 s after dispatch. May a tool declare its own?
3. **What the alert names.** An id is opaque to a person. Should a tool say
   what it asks about (`asks <expr>`, such as a note's title), or should
   Fieldnotes' tool take a title and resolve it?
4. **The take:** a waiver, or which item comes off?
5. **Allow Agents:** off by default per app, or on whenever the manifest's
   `mcp` is on?

## 9. Review dispositions (r2)

| review | concern | disposition |
|---|---|---|
| Astra N1 (BLOCKING), Grok MAJOR | a result can answer another ask; kept tickets; `Answer::Now`; background | taken: a pending result input is `busy`; fulfilment commits are owned; each replacement path has an outcome; `Answer::Now` and no-ops are defined; streams and background are refused (D4) |
| Astra N2 (BLOCKING), Grok MAJOR | rejection mixed with uncertain effects; the bound vs the alert; cancellation | taken: phases, certainty on every outcome, consent outside the bound, no response on cancel, withdrawal on access-off and plan change (D4, D5) |
| Astra N3 | the example's failures; deleting the edited note; overlapping writes | taken: `fails when` on all three, the delete policy and two tests, serialized writes (D11) |
| Astra N4 | the key-window rule | taken: sole, named, last key (D7) |
| Astra N5, Grok MAJOR | the command list was a sample | taken: all 32 classified, unknown refused, a completeness test, gated tasks in the reach (D3, D4a) |
| Astra N6 | live availability vs static discovery | moved with WebMCP and the connector (§4); the local door evaluates it live (D2) |
| Astra N7 | annotation meanings | taken: `additive`, `idempotent`, `closed`; grants do not decide `openWorldHint` (D2, D3) |
| Astra N8, Grok MAJOR | a deadline borrowed from LLP 1112 | moved to direction with the connector; slice 1 has one 10 s bound (§4) |
| Astra N9 | Caltrain's fixed timetable; the front door | recorded as open problems (§4) |
| Astra N10 | the consent test deadlocks | taken: `call` returns at the hold, `consent` answers it, `expect call` reads `tools.last` (D10, D11) |
| Astra N11 | the bearer-token claim | taken: removed (D6) |
| Astra N12 | MCP authorization is optional | taken (§4) |
| Grok MAJOR | the signed-in connector's citations | taken: replaced (§4) |
| Grok MINOR | citations; `failed(x)`; the windowless session | taken (§1, D2, §4) |
| Astra suggestions | traces; a transport profile; `exact new` | taken (D4, D7) |
| Grok suggestions | `readOnlyHint` only without a `send`; `outputSchema`; `tap` as a commit; `sun_path` | taken (D3, D4, D7, D10) |

r1's dispositions are summarized in each review file's round-1
**Disposition:** line. The r1 concerns round 2 marked resolved stay
resolved: the opt-in socket, the compatibility id, the asserted `reads`, no
web `destroys`, the fixture without a snapshot, the JS runtime accounting
(now §4), and the factual inventory.
