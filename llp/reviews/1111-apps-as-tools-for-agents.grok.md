# Review: LLP 1105: Apps as tools for a person's agent (r1, grok)

> **Renumbered 2026-10-08.** These RFCs were drafted as LLP 1105, 1106 and 1107 in a checkout 542 commits behind origin, where those numbers had since been allocated to other documents. They were committed as LLP 1111 (apps as tools), 1112 (a URL under a state) and 1113 (settled and the two clocks); LLP 1101.003 kept its number. The reviews below are transcribed as returned and keep the numbers they were written against: read 1105 as 1111, 1106 as 1112, 1107 as 1113.

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` the same detached worktree at `2ce193976`; 2026-10-07T04:16:30Z to 2026-10-07T04:26:51Z.
- **Method:** one brief per RFC (sha256 `bf3784510b6fd282cc0674029ba5f3e5d0b4a886b0ef95e07dec2c7de98899f9`), shared verbatim with the other reviewer; round 1, blind to the other review. Requested by Charlie ("have astra max and grok 4.7 xhigh review the RFCs"). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted: the brief and documents hold no secrets.
- **Transcription:** the review as returned, unedited. The sentences before the assessment are the CLI's progress narration, kept as returned.
- **Verdict:** NOT READY.
- **Disposition:** r2 (2026-10-06) takes every concern; LLP 1105 §8 maps each one to the decision that answers it. Two suggested remedies are declined: a per-launch socket token (any same-user process can read it) and keeping the focus exception only for `reads`. r2 also follows Charlie's direction (an MCP server and a staged connector per app, inferred parameter types, routes as resources), which supersedes r1 D2's declared-types rule and r1's deferred remote door.

---

I'll review the draft RFC read-only: first the binding rules and the document, then the sibling RFCs and the code it cites.The draft is loaded. Next I’ll check its claims against the cited code and the two sibling RFCs.The design is the right one for this repository, and it is not ready to build from. Declared tools, with the screen left to the platform accessibility tree and the development driver left at ten operations, match the binding rules. Several decisions an implementer would follow are wrong or contradict each other: the macOS door’s trust boundary, the compatibility id, the `reads` check, the wait, and the Fieldnotes test.

## 1. Overall assessment

A person’s agent has no honest way into a local-first Exact app. The development agent API is correctly sealed in production (`LLP 1069.007` D2, implemented: `Session.swift:45-54`, `host/linux/src/app.rs:487-504`, `host/web-js/build.mjs:347-352`), and driving the accessibility tree is the platform’s job. Declared tools bound to actions the app already has, with host-owned consent and no new driver operation, is the change that belongs here. It is not a new surface, not a second authoring model, and not server-driven UI.

The draft asks Charlie for the take and names no implementer, which is what `rules/RULES.md` requires. The text still specifies mechanisms, and four of those mechanisms are unsafe or inconsistent with the code and the siblings. I would not hand this to an implementer until those are rewritten.

Claims I checked against this tree:

- `runner/src/agent.rs` is 1,443 lines. Its header (`agent.rs:6-7`) warns against a host mirror of the tree. `tree` emits each node’s `handlers` (`agent.rs:452-466`; the cited range stops at 457, a few lines early). `state` encodes slots, derives, and resources through `typed_json` from the plan’s type table (`agent.rs:722-832`, `1091-1128`). `Value::conforms` is `plan/src/value.rs:240`.
- Linux opens the agent channel only when `EXACT_AGENT=1` (`host/linux/src/app.rs:231`, `561-562`). Headless screenshot and smoke runs do not. `Session.swift:28` substitutes launch facts; `:64` is the `agentMode` flag. The Apple channel starts in the adapters (`ExactMac/main.swift:7`, `ExactIOS/main.swift:8`).
- A `textarea` has no `submit` (`contract/lower/src/handlers.rs:24-29`). A single-line input submits on Enter on the JS target (`host/web-js/rt.js:913`). `schema.json` has no `form` tag.
- Fieldnotes’ actions are where the draft says (`app.contract:87-165`). `search(value)` is untyped and inferred because `input=search` calls it (`app.contract:235`, `contract/types/src/component.rs:338`). Notes are SQLite at `app:/data/fieldnotes.db` (`app.ts:29`). `confirmDelete` is session state the view draws (`app.contract:51`, `159-164`), not a host sheet. The TypeScript source is placed on a worker (`app.json:14`). The Linux host links no JavaScript engine (`host/linux/Cargo.toml:5-6` and its dependencies). LLP 1027 line 451 still says Linux execution is unbuilt. Fieldnotes declares no Linux host (`app.json:15-18`).
- `clock replies` waits until `state.pending` is empty, bounded at twenty seconds on native hosts, and does not move the clock (`llp/1012.000-attached-agent-sessions.rfc.md:117-126`). `has_pending` does not count device holds (`runner/src/runner/commit.rs:1124-1135`). Apple’s agent `state` includes `navigation.url` (`AgentMac.swift:139`, `AgentIOS.swift:191`). The Linux agent reports navigation unavailable (`host/linux/src/agent.rs:225`).
- WebMCP’s `document.modelContext.registerTool`, the four annotations, `AbortSignal` unregistration as of Chrome 153, the `toolchange` event, and default-closed `exposedTo` match Chrome’s imperative API page as updated 2026-09-21. `untrustedContentHint` defaults to false there. `consequentialHint` is a hint the page sets. The page cannot read back whether the browser honors it. The origin-trial window “149 through 156”, the Edge flag, and the Firefox and Safari positions are not on that page; the draft already marks those unverified.

## 2. Strengths

- **D1 and §2** refuse derived tools for the reason `agent.rs:6-7` already gives, and they leave the accessibility tree to LLP 1080.002. The “when this is not worth it” case, an app whose backend already has an API, is the right scope cut.
- **D8 and D9** keep production tool calls out of agent mode and out of the ten operations. `tap <root> history -1` is a real precedent for a `tap` that is not a press (`scripts/agent.mjs:1058-1059`). D9’s distinction from server-driven UI matches the refusal in `rules/DEFERRED.md`.
- **D2’s root-only rule** matches how handlers already resolve: a handler names an action of the root (`contract/lower/src/handlers.rs:42-47`). Telling the author to write a tool-shaped action, instead of binding `edit`, matches Fieldnotes: `edit` sends `opened`, resets the session, and focuses the body (`app.contract:104-107`), which would drop a draft.
- **D6’s grant and token claims, narrowly read,** match the code. A call that is just an action cannot grow grants. LLP 1018 D5 keeps the bearer token out of the plan, so a `returns` expression cannot name one.
- **D7(a)** is the right shape for this repo: one on-demand artifact, nothing before first pixel, and the wasm target left until a consumer. **D7(c) and (d)** staying out of v1 is right. Fieldnotes cannot be the Linux consumer.
- **D10** names a consumer, a proof someone can watch, and a take. **§7** asks Charlie the questions that are actually his.
- The sibling wording for “a URL under a state” matches LLP 1106 D1’s definition. The draft does not invent a second meaning. The mistake is in how D8 applies it, below.

## 3. Concerns

### BLOCKING — The macOS socket is a same-user API, and the container it names does not exist

**Section:** D6, D7(b).

**Evidence:** Today’s Mac builds are unsandboxed. `DocumentPickers.swift:11` says the security-scoped call is “a no-op for today’s unsandboxed builds.” An unsandboxed app has no container, so `$CONTAINER/tmp/exact-tools.sock` is not a path. Mode `0600` stops other users. It does not stop another process of the same user. D7(b) listens for the whole time the app runs, not only while `exact tools` is connected.

`writes` has no prompt (D3). `readNote` is classified `writes` and returns the note. Any local process that can find the socket can list, read, create, and modify notes, and can run any command the action can start, inside the app’s grants, without the person having configured an MCP client. Configuring Claude Code with the shim is a per-client grant. A listening socket skips that grant. `destroys` still asks, so the hole is everything that is not `destroys`.

**Resolve:** Make the shim the only peer. Have the app accept one connection from a launch the person configured, or check the peer and refuse everyone else. State the path for an unsandboxed build. Say the threat model in one sentence: another process of this user can call every tool you did not classify as `destroys`.

### BLOCKING — Putting the tool table in the compatibility id forks a stream on every tool edit

**Section:** D6.

**Evidence:** LLP 1030 D3a (`llp/1030-delivery-unified.rfc.md:191-242`) defines the compatibility id as the digest of what a bundle depends on and cannot replace. The same decision says the id must not include anything that does not make a bundle unsafe on an older binary. Plan contents ship on the existing stream. Hashing the tool table into the id makes each added or edited tool a new cohort. Old binaries do not fetch that cohort, so the Fieldnotes tools in D10 would not reach installed apps without a new binary.

The reason given, “a reviewer sees it change in the dry run,” is the deploy classifier’s job (LLP 1030.000), not the cohort key’s.

**Resolve:** A format-version bump admits the `tools` table once, and that bump already feeds the compatibility id. The table’s contents stay plan data. The dry run prints the tool names, consequences, and bound actions.

### MAJOR — `reads` is not a fact across the data module, and `writes` never asks

**Section:** D3, the summary’s “fact, not a guess,” D10’s table.

**Evidence:** The analyzer can see Contract sends, store names, and commands. It cannot see `app.ts`. Fieldnotes’ own `library` opens the database and, when the table is missing, runs `CREATE TABLE` (`app.ts:28-36`, called from `library` at `133-135`). D10 binds `findNotes` as `reads` to `search`, whose only Contract effect is `query = value` (`app.contract:87-88`). The re-answer of `library` is exactly the case D3 defines as a read. The platform hint would say read-only while the module can create the schema. On the web, `app.ts:31-32` notes that even a no-op `execute` exports the whole database.

Separately, `writes` is unchecked and unprompted. An action that sends `deleteNote` (`app.contract:163`) labeled `writes` never shows the sheet. The dry run, as specified, shows the adjective, not the reachable sends.

**Resolve:** Say what `reads` actually certifies: no Contract `send`, no store write, no command, and name the disqualifying send. State that a resource body can still write, and that the author owns that. In the deploy table, print the sends reachable from the bound action. Keep `writes` as the author’s word only after that listing exists.

### MAJOR — The wait is not `clock replies`, and a timed-out call is not safe to retry

**Section:** D4, D8.

**Evidence:** `clock replies` waits until every in-flight request is gone, for twenty seconds on native hosts, on the agent clock (`llp/1012.000-attached-agent-sessions.rfc.md:117-126`). Device holds are outside that wait (`commit.rs:1124-1135`). D4 waits only for requests that one commit started, for 10 seconds, on the wall clock, then evaluates `returns`. Those are different waits. An implementer who calls the existing helper will also wait on the person’s unrelated requests, and will not wait on a resource that becomes pending after the commit returns.

D4 also does not say whether the runner keeps taking the person’s events during the wait. §6 says a tool and an edit interleave. The step list reads as one synchronous call. Fieldnotes’ source is on a worker so the UI thread does not block (`app.json:14`). Blocking the runner for up to 10 seconds fights that.

On timeout, effects stand. MCP’s `idempotentHint` is not in the annotation table. A client that retries a timed-out `addNote` inserts a second note. WebMCP’s `execute` also receives an abort `signal` (Chrome’s imperative API). Cancellation is unspecified, and it has the same “effects already committed” outcome.

**Resolve:** Specify the wait as its own rule: the requests and resource re-answers this commit caused, wall clock, default bound, person’s events still applied, `returns` read after those requests settle. The timeout and abort errors must say the effects may have landed. Set `idempotentHint: false` unless the author marks the tool idempotent. Pick one bound; 10 seconds and `clock replies`’ twenty seconds should not both be in the story.

### MAJOR — Web `destroys` depends on a signal the browser does not provide, and the macOS prompt is doubled

**Section:** D5, §7 Q2.

**Evidence:** Chrome’s imperative API documents `consequentialHint` as a hint the page sets so that an agent or the browser may confirm. Nothing in that API, or in the register options (`signal`, `exposedTo`), reports that the browser honors the hint. D5 says a web `destroys` tool is registered only where the browser reports that. Q2 recommends registering `reads` and `writes` only. An implementer cannot satisfy D5.

On macOS, D5 asks for MCP elicitation and the host sheet “as well as” the sheet when the window is up. That can be one prompt or two. Elicitation is a mid-call client capability. The draft never names the protocol revision, how a client advertises elicitation, or the MCP error shape (`isError: true` versus a JSON-RPC error). WebMCP’s in-page `execute` return and MCP’s `tools/call` result are different encodings. Both are unspecified.

**Resolve:** Adopt Q2 as the decision: no web `destroys` until a browser documents a confirmation the page can rely on. For the shim, one confirmation: the host sheet when a window can present it, otherwise elicitation, and no `destroys` tool when neither exists. Name the MCP revision and the result and error objects.

### MAJOR — Person-requiring commands still run, and only two are held back

**Section:** D4, D3.

**Evidence:** D4 holds back `focus()` and `showPicker` during a call. D3’s command list is `share`, `saveFile`, `showPicker`, with `focus` excepted. Fieldnotes actions call `share`, `saveFile`, and `showPicker` (`app.contract:131-135`, `157`). A `writes` tool bound to an action that shares or saves opens a system panel with no `destroys` prompt and nobody required to be the MCP user. `openAuthSession` (LLP 1069.006) is in the same class and is not mentioned. A skipped picker that still returns `{"ok": true}` looks like success.

**Resolve:** List every command a tool call refuses, including share, save, pickers, auth, and notifications. A call whose action reaches one is a tool error, and the action does not run. Keep the focus exception only for `reads`, and say so in the result when a focus was skipped.

### MAJOR — The Fieldnotes test cannot seed its notes from “a URL under a state”

**Section:** D8, related-work paragraph.

**Evidence:** LLP 1106 D1’s store snapshot is LLP 1018’s plain store. The same decision says SQLite, the device, and secrets read that snapshot or are refused by name (`llp/1106-a-url-under-a-state-as-an-image.rfc.md:152-154`). Fieldnotes’ notes are rows in `app:/data/fieldnotes.db`, not store keys. `call findNotes` against “a store snapshot of three notes, at `/`” runs the real `library` resource and sees an empty database. LLP 1103’s grammar has no `call` step. That part is new, which is fine. The fixture is not.

**Resolve:** Seed the three notes the way the module reads them, or state that the contract test uses a fixture source and the live proof is the only SQLite proof. Do not call the SQLite file a store snapshot.

### MINOR — `available when` is described as a derive and exemplified as a resource field

**Section:** D2, the opening example.

The opening example uses `available when library.ready`. `library` is a resource (`app.contract:70`). D2 says the expression “is a boolean derive.” An implementer has to guess whether slots, resource fields, and `pending()` are allowed. Say it is a boolean expression over the root’s state, and name what it may read.

### MINOR — “Secrets cannot leave” is broader than LLP 1018 D5

**Section:** D6.

D5 keeps the bearer token out of the plan, so `returns` cannot include that token. Note bodies, and any secret the module copies into a slot, can. The heading should say “a bearer token,” which is the property the code actually gives you. Tool arguments are outside that rule. If the journal records them, they are a second copy.

### MINOR — Citations and one sibling pointer

`Session.swift:28` and `:64` are not where the channel is admitted. Linux admits it on the agent branch of headless mode, not on every headless run. `state.navigation.url` is a host section. The Linux agent does not have it, which matters for the no-`returns` payload `{"ok": true}` plus the location.

§5 points at LLP 1101.003’s print-once text as “what is on screen” for a GUI app. LLP 1101.003 D9 prints only a terminal entry. A GUI app’s static picture is LLP 1106.

D3’s default of `untrustedContentHint: true` disagrees with §7 Q5, which still asks Charlie. Chrome’s default for that hint is false, so Exact’s default is a real choice and should stay a question or become the decision, not both.

## 4. Suggestions

- Record the admission as a feature with D10’s take, not as a new surface. `rules/DEFERRED.md` §Surfaces is about another presenter and another sweep. These doors sit on the web and macOS hosts that already exist.
- In the annotation table, set MCP `openWorldHint: false` for a tool that only touches the app’s own grants, and set `idempotentHint` from the author or leave it false. The MCP default for `openWorldHint` treats the tool as talking to the outside world.
- Define the driver reply so it cannot collide with `tap`’s `delivery` field. `delivery: "substituted"` is already the hold-answer reply (`agent.rs:208-214`). A confirmation the driver must answer should be a device hold, answered with the existing `tap @ticket`, which `has_pending` does not wait on. Putting it in `state.pending` makes `clock replies` and `clock settle` wait on a person.
- Say that the driver’s `tap <root> {"tool":…}` enters D4’s runner path. It is not a synthesized press at a point.
- Forward WebMCP’s execute `signal` into the wait, with the same outcome as the timeout.
- When App Intents come back, generate them with the binary. A later plan update can add a tool the compiled intents do not have. The v1 plan table should not assume iOS can hot-add tools.
- If the deferred headless instance returns, give it LLP 1106 D8’s bounds for running app code for someone who is not the person at the keyboard. “Its own store, not the person’s” is the start of that sentence.
- Map `option<T>` to JSON null the way `typed_json` already does (`agent.rs:1099`), and reject extra fields, missing fields, and non-finite numbers, which is what `Value::conforms` already does. Name that, so the schema text and the checker stay one rule. `TypeKind` in this tree is number, bool, string, unit, option, list, and record.

## 5. Open questions for the author

1. During the wait, may the person’s events commit, or does the tool call occupy the runner until `returns` is read? §6 and D4 currently answer both ways.
2. Is a same-user process that was not launched as `exact tools` in the threat model? D7(b) as written says yes.
3. Should the deploy dry run show reachable sends, or is the author’s `writes` / `destroys` word the whole review?
4. For the contract test, what is the fixture if it cannot be an LLP 1018 snapshot? A temporary database file, a fake source, or no contract test until the live proof?
5. Q2, Q5, and Q6 are the right questions for Charlie. D5 and D3 currently pre-answer Q2 and Q5. Which text wins?

## 6. Verdict

Verdict: NOT READY

## Round 2 (2026-10-07), on r2

- **Family:** xAI. `grok-4.7` via the grok CLI (`--prompt-file`), reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` a detached worktree at `2ce193976` holding the three r2 drafts and only Grok's own r1 reviews; 2026-10-07T20:14:49Z to 2026-10-07T20:28:38Z.
- **Method:** one round-2 brief per RFC (sha256 `695a861a163c54033d977201591f34549f640c32edcd5c03320c7dd6c2e58f21`), shared verbatim with the other reviewer; blind to the other family's reviews, though r2's dispositions section names concerns both raised. Requested by Charlie ("ok sounds good" to a second round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited; the sentences before the assessment are the CLI's progress narration.
- **Verdict:** NOT READY.
- **Disposition:** r3 (2026-10-07) shrinks LLP 1105 to the local MCP slice for Fieldnotes on macOS and takes every round-2 concern; §9 maps each. The race: ownership includes fulfilment commits, the kept ticket and `enqueue`'s drop each have an outcome, and `stale` follows `returns`/`fails when` inputs. The alert: consent sits outside the 10 s bound, which starts at dispatch, and decline, dismiss and no answer are `refused` with certainty `none`. The commands: every `HOST_COMMANDS` entry is classified, and an unknown one is refused. The connector's deadline and the signed-in stage's citations move to §4 as direction. The minor items are taken. Nothing is declined.

I'll review the r2 draft of LLP 1105 against your r1 notes, the sibling RFCs, and the code it cites. First I'm reading the binding rules and the review skill so the judgment stays inside this repo's process.The binding rules are in. Next I’ll read the r1 review, the r2 RFC, and the sibling drafts that define the shared terms.The r2 text is in. I’ll check its new claims against the code and the sibling decisions it now depends on.The r2 draft is the right shape for this repository, and it is not ready to build from. Declared tools, host consent, and a staged connector match Charlie's aim and the CLI bargain in LLP 1101.003: the author names what to expose, and Exact generates the schemas, the transports, and the call path. The two r1 blockers are fixed. The stage-1 race and the stage-2 failure mode are still specified against the wrong code and against LLP 1106, so an implementer of the first slice would be guessing.

## 1. Overall assessment

What you get is real plumbing, and it is not free. One `tool` line binds a root action. Doors stay off until `app.json` turns them on. That matches the softened aim. The local server is `exact tools <app>` plus an opt-in socket. The anonymous connector is the same tool table on LLP 1106's anonymous mode. The signed-in connector is a different system, and D8 describes it with two LLPs that do not provide it.

Stages 1 and 2 are the first slice (D11). The happy path for Fieldnotes is specified: a separate mutation so a save does not move the draft (`app.contract:79-80`), `saveNote("", …)` inserts (`app.ts:147-154`), and the test seeds notes by calling `addNote` into the agent scratch (`NativeModule.swift:464-466`). Caltrain's boards are synchronous Rust (`apps/caltrain/data/src/lib.rs:269-274`), so an anonymous `query()` works through the existing `Anonymous` adapter (`host/render/src/source.rs:19-35`). The race example, the delete alert's timeout, and a connector that hits a deadline are not specified at that level.

Checked against this tree: `Session.swift:44-54`, `host/linux/src/app.rs:145-151`, `:231`, `:487-504`, `:561-562`, `host/web-js/build.mjs:279` and `:346-352`, `runner/src/agent.rs:6-7`, `:452-466`, `:1091-1128` (the file ends at line 1443), `plan/src/value.rs:240`, `commit.rs:933-1001`, `scripts/agent.mjs:1205-1209`, `ExactMac/main.swift:132` and `:161`, `DocumentPickers.swift:11`, `NativeModule.swift:464-470`, `host/linux/src/agent.rs:651`, `host/render/src/serve.rs:1-14`, `apps/caltrain/app.contract:11-13` and `:54-108`, Fieldnotes at the lines D2 and D11 cite, and `contract/syntax/src/lib.rs:83-146` (`HOST_COMMANDS`). `contract/lower/src/handlers.rs:21-29` is the textarea `submit` rule. There is no `form` tag in `kernel/tables/schema.json`. `apps/messages/linux/src/main.rs:5` is a type alias; the TypeScript embed is lines 4 and 7. `scripts/agent.mjs:1205` is the start of `clock`; the `data` branch is line 1209, and it leaves the runner clock unmoved.

## 2. r1 concerns

1. **Same-user socket, no container — resolved.** D7(a) listens at `~/Library/Application Support/exact/<id>/tools.sock`, off until Allow Agents, and states that any same-user process can call every non-`destroys` tool while that is on. Declining a token file is sound on an unsandboxed Mac.
2. **Compatibility id — resolved.** D6 puts one `formatVersion` bump in the id (LLP 1030 D3a; `formatVersion` is already a cohort input) and leaves the table as plan data.
3. **`reads` is not a fact; `writes` never asks — resolved.** D3 makes `reads` the author's assertion, fails the compile on a store write or a command, lists the sends, and leaves `writes` unprompted on purpose.
4. **The wait, and retry after timeout — partly resolved.** D4 is its own 10s wall-clock wait, names `clock data`, sets `idempotentHint` false, and the `unknown` text says the effects may have landed. The bound versus the alert, and `superseded` versus `enqueue`, are still open (below).
5. **Web `destroys` and a doubled prompt — resolved.** D5 offers no web or connector `destroys`, one `NSAlert`, and elicitation is not consent.
6. **Person-requiring commands — partly resolved.** `share`, `saveFile`, `showPicker`, `showNotification`, and `playSound` fail at compile time. `openAuthSession` and a permission prompt fail at run time. `focus`, `blur`, and `scrollIntoView` are skipped and reported. The rest of `HOST_COMMANDS` is unclassified (below). Declining a `reads`-only focus exception is reasonable once `skipped` is in the result.
7. **The Fieldnotes fixture — resolved.** D10 seeds three notes with `call addNote` into the agent-mode scratch at `NativeModule.swift:464-466`. LLP 1106 D1 r2 has no store snapshot, so the old fixture is gone on both sides.
8. **`available when` — resolved.** D2 is an expression over slots, derives, resources, mutations, `pending(x)`, and `failed(x)`.
9. **Secrets — resolved.** D6 limits the guarantee to the bearer token (LLP 1018 D5) and says a secret copied into a slot or resource can be returned (`now_screen.rs:84`, `:141`).
10. **Citations, the GUI picture, `untrustedContentHint` — resolved.** The r1 line numbers are corrected, §5 points a GUI picture at LLP 1106, and D3 sets the hint to true unless `trusted`. New citation misses are the minor below.

## 3. New concerns

### MAJOR — The stage-1 race is not the function D4 cites, and the kept-ticket path has no outcome

**Section:** D4, the `findNotes("rain")` example.

**Evidence:** `forget` at `commit.rs:933-940` drops a target when a mutation is assigned (`:811`, `:816`) or a resource is answered (`settlement.rs:752`). A new in-flight request is dropped inside `enqueue` at `commit.rs:997-1001`, which does not call `forget`. Fieldnotes' `library` is a continuation (`js/src/lib.rs:778`, `:920`), so a second search takes that path. HTTP resources take the other path: if only the arguments moved, `enqueue` keeps the ticket and the source answers the newer arguments (`commit.rs:964-995`, LLP 1054.000.000 D3). `stale` only watches a slot the `returns` expression reads. `returns library` reads a resource, so the person's `query` write is invisible to `stale` and, on the keep path, invisible to `superseded`. The call can come back `ok` with the other query's notes.

A fulfillment commit re-settles the whole tree. Once `saveRevision` counts `created` (D11), `addNote`'s fulfill re-asks `library` in that commit. If the person changed `query` in between, that request is both "a re-answer its writes caused" and the person's search. D4 says the call owns the first and neither of the second.

**Resolve:** Own every request enqueued by the invocation's commit and by fulfillment of an owned request, and say what happens when that enqueue also sees the person's slots. `superseded` is an owned ticket dropped in `enqueue` (`:997-1001`) or `forget` (`:933-940`), naming the replacing commit. The kept-ticket path gets its own outcome. `stale` follows the inputs of `returns` and `fails when`, including the arguments of a resource they read.

### MAJOR — The 10s bound can report that a delete ran while the alert is still up

**Section:** D4, D5.

**Evidence:** D5 draws an app-modal `NSAlert` and dispatches only after yes. D4's `unknown` text is "the action ran; its requests had not answered; they may have landed." Nothing says the 10s starts at dispatch rather than when the door receives the call. The stage-1 proof is a person watching that alert (`D11`). A yes after 10 seconds matches `unknown`, and the sentence is then false. `NSAlert` runs a modal loop on the calling thread, which also fights "the person keeps working" for the duration of the alert.

**Resolve:** Consent is outside the bound. Decline, dismiss, and no answer are `refused`, and the action does not run. The 10s starts at dispatch. Say the alert is modal and the wait after dispatch is not.

### MAJOR — A connector deadline contradicts LLP 1106, and the launch facts are unnamed

**Section:** D8, D4.

**Evidence:** D4 bounds every call at 10s. D8 also applies LLP 1106 D11: 2s cooperative inside the child, hard kill at 2.5s, and says the hard deadline ends `unknown`. LLP 1106 D11 says a hard-killed child has no tree and the server answers 503 `no-store`. The child cannot emit an MCP result after it is killed. D11's picture path uses the same 503 row for "deadline or busy." D9's resource read and D8's boot wait say "default facts" and "data-settled." LLP 1106 D1 has four columns with different epochs. The server's clock is the named-picture column. The agent epoch is `1767225600000`. Caltrain's proof is wrong unless the process clock is the wall clock (D11's own prerequisite, `app.contract:54`). The launch record that carries the tool name, the arguments, and the child's result is not in either RFC. `tools/list` without a boot is specified. `tools/call` is a child with no IPC.

**Resolve:** The connector's bound is D11, not 10s. The cooperative deadline returns MCP `unknown` from the child. The hard kill is the server's 503, with no MCP body. Name the launch-record fields for a call and for a resource read: server clock, anonymous mode, no `answers`, location `/` unless the read's path is the location, and the outcome on the child's stdout or a result file.

### MAJOR — The compile-time refusal list is a sample of the host commands

**Section:** D4.

**Evidence:** `HOST_COMMANDS` is `contract/syntax/src/lib.rs:83-146`. D4 refuses `share`, `saveFile`, `showPicker`, `showNotification`, and `playSound`. The same file also has `showOpenFilePicker`, `showDirectoryPicker`, `showSaveFilePicker`, `closeNotification`, `playSounds`, `stopSounds`, `openURL`, `copyText`, `close`, `showModal`, `reload`, `deliveryActivate`, `deliveryCheck`, `haptic`, `fastSeek`, and `load`. A `writes` tool may call those. D3's lint catches commands only on a `reads` tool. `selectText` and `setSelectionRange` are the same class as `focus` and are not in `skipped`. D4's "reaching" does not say it uses D3's reach (props, `then`, and LLP 1089 calls). A runtime permission refusal "lets the source see the refusal as data" and also ends the call `refused`, so the module's value and the tool result can disagree.

**Resolve:** Classify every `HOST_COMMANDS` entry: refuse, skip and report, or run. Refusal uses D3's reach. On a runtime refusal the tool result is `refused` and the module's later value is not returned.

### MAJOR — The signed-in connector cites a client store and a browser session as a server OAuth stack

**Section:** D8, stage 4.

**Evidence:** LLP 1018 D2 is the on-device secret tier (Keychain, a `0600` file, `localStorage`). The plain tier is unbuilt (`llp/1018-durable-client-state.rfc.md:137`). There is no per-person server store. LLP 1069.006's host opens the system browser and returns a callback URL. The source builds PKCE, exchanges the code by `fetch`, and stores the token with `secret.keep`. The connector cannot be "an OAuth client of the app's own provider, the one LLP 1069.006 signs the app in with" without a new confidential client and a token exchange the app's source currently owns. MCP's rule against forwarding the client's token is stated. The place the backend token lives is not.

**Resolve:** Keep stage 4 as a stage, and replace those two citations. Name the OAuth client the connector registers as, where each person's backend token is stored, and that LLP 1018 and `openAuthSession` stay on the device. The anonymous stage does not need this.

### MINOR — A few citations and one expression do not match the grammar

**Section:** §1, D2, D4.

`handlers.rs:21-29` is the textarea rule, not "no `form`." `messages/.../main.rs:5` is a type alias. `agent.mjs:1205` is four lines above the `clock data` branch, and that wait does not run on the runner clock. `src/emit.rs:428` is `host/web-js/src/emit.rs`, the action emitter. `failed(x)` is a resource only (`docs/contract-grammar.md:565`). A mutation reports failure in its value, which the Fieldnotes example already does with `s.failed`. D2 lists `failed(x)` as if it took a mutation too. `app.ts:143-152` stops before the `INSERT` at line 154.

### MINOR — The Mac windowless resource session is specified and has no stage

**Section:** D9, D11.

D9 reads a route in a windowless session of the running Mac app, sharing the person's files, while the socket is bound to the key window (D7 a). D11's stage 2 builds route resources on the Caltrain connector, one anonymous process per read. The Mac session is a second implementation with SQLite shared with the open window, and no stage owns it.

## 4. Suggestions

- Publish `readOnlyHint: true` only when the listed reach has no `send`. D3 allows a `send` inside `reads` because `library` can run `CREATE TABLE` (`app.ts:28-36`). The dry run shows that. An MCP client sees only the hint.
- Type `outputSchema.value` from `returns`, inside the `{value, stale, skipped}` envelope D4 requires so a list still fits MCP's object rule.
- Say `tap {"tool":…}` dispatches the action the way `Runner::act` does (`llp/1012-agent-api-v1.spec.md:318`). It is a commit with an invocation id.
- Put the connector's `/.exact/mcp` child on the same launch-record file LLP 1106 D9 already has, extended with the call.
- Give `exact tools` one sentence for "not running" and "Allow Agents is off," and a shorter socket path when `sun_path` would exceed 104 bytes.
- Assign the Mac windowless resource session to a stage, or leave it out of v1 and let local route reads wait.

## 5. Open questions for the author

1. On the kept-ticket path, should the call end `superseded`, `stale`, or `ok` with the newer arguments?
2. For a fulfillment commit that re-asks `library` after the person has changed `query`, whose request is that?
3. Is a connector deadline an MCP `unknown` or LLP 1106's HTTP 503?
4. Which stage, if any, builds the Mac windowless resource session?
5. For stage 4, is Interview or the Bluesky client the consumer that already has a confidential OAuth client a server can use?

Verdict: NOT READY

## Round 3 (2026-10-07), on r3

- **Family:** xAI. `grok-4.7` via the grok CLI (`--prompt-file`), reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` a detached worktree at `2ce193976` holding the three r3 drafts and only Grok's own r1 and r2 reviews; 2026-10-07T20:54:22Z to 2026-10-07T21:21:11Z.
- **Method:** one round-3 brief per RFC (sha256 `30fb735a98174484bff4f42c617a36b2621d97240d59294a9764b933fc1ec05c`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("sounds good, try that" to the narrowed r3 and a third round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited; the sentences before the assessment are the CLI's progress narration.
- **Verdict:** NOT READY.
- **Disposition:** pending; the loop stops here after three rounds; not revised against this review.

I'll review the r3 draft of LLP 1105 against the code and the sibling RFCs, read-only. First I'm loading the binding rules, the review skill, and the documents.The r2 review and the r3 draft are in. Next I’ll check the first-slice claims against the code and the sibling RFCs.The r3 cut is the right one, and it is not yet something an implementer can build. Slice 1 is a local MCP door for Fieldnotes on macOS: one `tool` line, an opt-in socket, host consent, and a call lifecycle. That matches the binding rules and the CLI bargain. Consent, the command table, and the threat model are specified against the code. The Fieldnotes proof is not. D4 returns a write while the list refresh it caused is still in flight, and the tests then tap and read that list with no `clock` step. Two certainty rows describe a state the runner does not have at the moment the call returns.

The direction section is marked unspecified and mostly honest about what it leaves open. One cross-reference in it is stale. That does not decide the verdict.

## 2. Round-2 concerns

1. **Stage-1 race — partly.** D4 now refuses a pending result input, owns fulfilment commits, and gives `enqueue`'s drop (`commit.rs:997-1001`), `forget` (`:933-940`), and the kept ticket (`:964-995`) an outcome. `stale` follows resources that `returns` or `fails when` read. The resource row of `superseded` is still wrong (below), and the Fieldnotes tests never wait for the refresh D4 refuses to own.
2. **10s bound versus the alert — resolved.** Consent is outside the bound, the 10s starts at dispatch, and decline, Escape, and 60s of silence are `refused` with certainty `none` (D4, D5). The alert is modal only while it is up.
3. **Connector deadline — moved.** Slice 1 has one 10s bound. The direction line that still cites LLP 1106 D11's 503 is a new citation error.
4. **`HOST_COMMANDS` was a sample — resolved.** D4a classifies every name in `contract/syntax/src/lib.rs:83-146`, refuses an unknown name, and fails a test when the array grows. The prose count "32" is a new miss; the array and the table both have 31.
5. **Signed-in connector citations — moved.** §4 asks for a confidential client, server-side token storage, and per-person instances, and it leaves LLP 1018 and `openAuthSession` on the device.
6. **Citations and `failed(x)` — resolved.** D2 limits `failed(x)` to a resource (`docs/contract-grammar.md:565`). The new misses are below.
7. **Mac windowless resource session — moved.** §4 lists it as having no stage.

## 3. New concerns

### BLOCKING — The Fieldnotes tests do not pass under D4 and the harness that runs them

**Section:** D4, D11.

**Evidence:** `addNote` returns `created` and does not read `library`, so D4 does not own the `library` re-ask that `toolRevision` enqueues in the fulfilment commit. The call returns at that commit, with the re-ask in flight. `findNotes` reads `library` (`fails when not library.ready`), and a call whose `returns` or `fails when` reads a pending resource is `refused` / `busy`. The note row is also absent or disabled: `busy` includes `pending(library)` (`app.contract:82`), and the row button is `disabled=(dirty or busy)` (`:245`).

The harness will not paper over this. Between steps the clock stands still, and a reply lands at a `clock` step (`scripts/agent-test.mjs:184-186`). None of the five tests has a `clock` step. `tap "note-1"` therefore runs before the new row exists. `type "note-body"` runs `changeBody`, which does nothing while `pending(opened)` (`app.contract:98-99`), so the dirty-draft test never makes a dirty draft. The three `findNotes` checks run while `library` is still the unowned re-ask, and D4 answers them `busy`.

Fieldnotes' `library` is a storage continuation, so it also never takes the kept-ticket path. `enqueue` keeps a ticket only when `request.storage` is absent (`commit.rs:976-979`). SQLite requests are dropped and replaced (`:997-1001`).

**Resolution:** Pick one rule and make the tests match it. Either a write waits for the resource re-answers its fulfilment commit enqueued (the continuations LLP 1106 D1a already attributes to an 1105 call), and then `findNotes` sees a settled `library`. Or the tests insert `clock data` after every write, before `tap`, `type`, and `findNotes`, and D4's `busy` rule applies only while the boot request is unanswered. The tests as written satisfy neither.

### MAJOR — A superseded resource is described as already answered for someone else

**Section:** D4, outcomes table.

**Evidence:** The row says certainty `applied` for a resource ask, "whose value now answers someone else." At the moment `enqueue` retargets or drops the ticket, the standing value still answers the older arguments while the new request is in flight (`settlement.rs:421-424`, the passage D4 cites for `busy`). Certainty's own definition says `applied` means the owned requests settled. A dropped request has not settled. Step 5 still evaluates `returns` at settlement. Only trace 2 shows `value: null`, and that trace is a send.

**Resolution:** On `superseded`, `value` is null and certainty is `uncertain`. Delete the claim that the value already answers the replacement.

### MAJOR — A settled `fails when` is reported as uncertain

**Section:** D4, outcomes table.

**Evidence:** `saveNote` and `deleteNote` answer with `failed: true` inside a settled value (`app.ts:157-158`, and the shared `service` catch at `:205`). That request settled. The table gives every `failed` outcome certainty `uncertain`, including `fails when` true, and the uncertain message says to read before calling again. A validation failure ("Write something before saving") and a dropped in-flight send become the same report. D11 adds `fails when` so the agent can read the app's own failure.

**Resolution:** Split the row. Owned requests settled and `fails when` true: certainty `applied`, and `message` is the app's message. A stream, LLP 1097 background work, or a request that failed in flight: certainty `uncertain`.

### MAJOR — LLP 1106's named difference and D4 describe different waits

**Section:** Related, D4. Sibling: LLP 1106 D1a.

**Evidence:** LLP 1106's header says LLP 1105 uses D1a with the differences D1a names (`1106-a-url-under-a-state-as-an-image.rfc.md:9`). The difference paragraph says an 1105 call tracks the requests and continuations its invocation started (`:159-160`). LLP 1105 says a tool call does not use D1a, and D4 waits for a resource re-ask only when `returns` or `fails when` reads that resource. LLP 1101.003 D2 shows the shape that works: it runs the procedure and lists its differences (`1101.003-the-command-line.rfc.md:122-142`). The library refresh after `addNote` is a continuation the write started, so the two documents already disagree on the blocking test.

**Resolution:** One sentence in both RFCs. If the call does not run D1a, LLP 1106's difference paragraph should say that, and D4 should state the narrower ownership as a named difference. If the call tracks those continuations, D4 should wait for them.

### MAJOR — The tests' scratch is not the directory D11 cites

**Section:** D11.

**Evidence:** `NativeModule.swift:464-466` is `NativeViews.roots`, the UI module's pid temp directory (`NSTemporaryDirectory()/exact-agent-<pid>-<rt>`), passed into a native view at `:437-438`. Fieldnotes' notes are the data module's SQLite file. On Apple that module is configured from `host/apple/src/picker.rs:20-55`: under the agent, `~/Library/Caches/exact/<id>/agent/<EXACT_AGENT_STORAGE>/data`. `agent-test.mjs:132` sets `EXACT_AGENT_STORAGE_FRESH`, and `empty_fresh_tree` (`picker.rs:88-106`) empties that tree. Each test's store name is unique (`agent-test.mjs:131`), so the first `INTEGER PRIMARY KEY` is `"1"` (`app.ts:154-155`). The conclusion about the id is right. The citation points at a directory the tests do not write.

**Resolution:** Cite `picker.rs` `app_dirs` and `empty_fresh_tree`. Leave `NativeViews.roots` out of the fixture.

### MINOR — Counts, lines, and one journal word

**Section:** §1, D4, D7, §4.

- `HOST_COMMANDS` has 31 names, not 32. The table is complete.
- `deleteNote`'s `failed: true` is the `service` catch at `app.ts:205`. Line 201 is the start of the delete branch. `saveNote`'s `failed: true` at `:158` is right.
- `host/terminal/src/cli.rs:176` is `interactive = args.is_empty() && is_terminal()`. The terminal crate has no agent gate at all.
- D7's session rule records the last key window and never reads current focus. The sentence that this follows `frontWindow()` (`main.swift:328`) points at a different rule: key, else main, else newest.
- Cancellation while the alert is up journals `tool deleteNote cancelled none` in trace 3. The prose says the journal records `uncertain`. `cancelled` is not an outcome.
- §4 says a connector's hard deadline is LLP 1106 D11's 503 with no MCP body. LLP 1106 D11 now kills `exact render` and exits 124 (`1106-…rfc.md:283-289`). 500 versus 503 is still an open problem in that RFC's §5.
- D7 says the relay reports "not running" and "Allow Agents is off" as different errors. While access is off the app does not listen, so both look like a missing socket. The signal that distinguishes them is unstated.
- `serial()` orders SQL (`app.ts:19-24`). It does not order the `session` slot. A Save already in flight still calls `saveNote` with the old id and can hit "This note was deleted" (`app.ts:149`) after `deleteById` has kept the draft.

## 4. Suggestions

- Make the pure-`reads` row set `destructiveHint: false`. An em dash leaves MCP's default, which is `true`, and that default is defined as meaningless once `readOnlyHint` is true. Setting it false keeps the schema from depending on that sentence.
- Put the consent hold on the existing device-hold path. Holds are already entries in `state.pending` with a `device` object (`agent.rs:843-865`). `has_pending` skips them (`commit.rs:1127-1134`), which is why `clock settle` does not wait on a person. D10's "not in `state.pending`" invents a second channel to get a result the current one already gives.
- Name the alert's buttons. The tests answer `yes` and `no`. The proof's "Don't Delete" is a narration, not a label rule.
- Say where Allow Agents is kept (a per-app default is enough) and that the alert orders the app forward, so the proof's "Fieldnotes behind Terminal" still shows it.
- The summary's declaration block is thirteen lines. The read is one line. The two write tools and their mutations are the other twelve.

## 5. Open questions for the author

1. Should `addNote`'s call wait until the `library` re-ask it caused has settled, or should the tests `clock data` before they touch the list?
2. For a superseded resource, is the result `value: null` with certainty `uncertain`?
3. When `saveNote` settles with `failed: true`, is certainty `applied`?
4. Does LLP 1105 use D1a with a named difference, or is D4 a separate wait that LLP 1106's difference paragraph should stop summarizing?
5. Q1 and Q5 still ask Charlie about launching the app and the Allow Agents default, while D7 already says the relay never launches and the switch is off. Which text should an implementer follow if Charlie does not answer before the slice is built?

Verdict: NOT READY
