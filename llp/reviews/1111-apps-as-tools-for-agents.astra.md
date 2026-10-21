# Review: LLP 1105: Apps as tools for a person's agent (r1, astra)

> **Renumbered 2026-10-08.** These RFCs were drafted as LLP 1105, 1106 and 1107 in a checkout 542 commits behind origin, where those numbers had since been allocated to other documents. They were committed as LLP 1111 (apps as tools), 1112 (a URL under a state) and 1113 (settled and the two clocks); LLP 1101.003 kept its number. The reviews below are transcribed as returned and keep the numbers they were written against: read 1105 as 1111, 1106 as 1112, 1107 as 1113.

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only (the local `codex` shim also passed `--dangerously-bypass-hook-trust`), `-C` a detached worktree at `2ce193976` with the three drafts copied in uncommitted; 2026-10-07T04:16:30Z to 2026-10-07T04:34:15Z.
- **Method:** one brief per RFC (sha256 `bf3784510b6fd282cc0674029ba5f3e5d0b4a886b0ef95e07dec2c7de98899f9`), shared verbatim with the other reviewer; round 1, blind to the other review. Requested by Charlie ("have astra max and grok 4.7 xhigh review the RFCs"). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted: the brief and documents hold no secrets.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r2 (2026-10-06) takes every concern; LLP 1105 §8 maps each one to the decision that answers it. Declined: nothing; one remedy is reshaped: the cheap path names the screen's action for reads, and a tool-shaped action is required only where a write shares the editor's state (D2). r2 also follows Charlie's direction (an MCP server and a staged connector per app, inferred parameter types, routes as resources), which supersedes r1 D2's declared-types rule and r1's deferred remote door.

---

**1. Overall assessment**

The problem is worth solving here: Fieldnotes has local data and existing application logic that a person’s agent should be able to use. Explicit exports, existing grants, and execution inside the running app fit Exact’s authoring model and do not constitute server-driven UI. However, this draft is **not ready for implementation**. Its central guarantees about read-only behavior, secrets, and consent exceed what the proposed mechanisms establish; asynchronous calls can return another interaction’s results; and the web implementation and session model are materially underspecified. This is a read-only source review at `2ce193976`, including the governing rules and both siblings. I made no edits and ran no builds or tests.

**2. Strengths**

- **D1:** Explicit, named exports are a sound choice. A stable operation with authored meaning is more useful than automatically exporting every visible control. The rejection of derived tools is well motivated.
- **D6–D7:** Reusing the app’s identity, grants, data sources, and running process avoids inventing another backend or independently opening its database from a sidecar.
- **D8–D9:** Keeping production tools separate from development inspection, clock control, and device substitution is correct.
- **D10 and §5:** Fieldnotes is a concrete consumer. Deferring remote access, headless execution, App Intents, streaming, and MCP resources/prompts keeps the initial scope defensible.

The following current-behavior claims are supported by the code:

| Claim | Source verification |
|---|---|
| Production builds suppress the GUI development agent channel. | [Apple’s environment filtering](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/apple/Sources/ExactKit/Session.swift:44), [Linux’s production check](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/linux/src/app.rs:145), and [web production gating](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/web-js/build.mjs:346). |
| `tree` lists handlers; `state` has a typed JSON encoder. | [runner/src/agent.rs:452](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/runner/src/agent.rs:452) lists event kinds; [agent.rs:1091](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/runner/src/agent.rs:1091) encodes values by plan type. |
| Fieldnotes has the cited actions and inferred `search` parameter. | [app.contract:87](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.contract:87); `edit`, `importPicked`, and `remove` occur at lines 104, 140, and 163. |
| There is no Contract `form`; `textarea submit` is refused. | [The tag vocabulary](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/contract/lower/src/tags.rs:53), [the explicit refusal](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/contract/lower/src/handlers.rs:21), and [input Enter handling](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:413). |
| Fieldnotes uses local SQLite and a TypeScript worker. | [app.ts:9](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.ts:9), database opening at line 28, and [app.json:14](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.json:14). |
| Linux lacks the TypeScript execution needed by Fieldnotes; Linux and terminal headless execution exist. | [Linux dependencies](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/linux/Cargo.toml:22), [Linux headless boot](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/linux/src/app.rs:536), and [terminal mode selection](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/terminal/src/cli.rs:176). |

I also confirmed the 1,443-line count for `runner/src/agent.rs`. The platform description is substantially accurate about WebMCP’s registration API, registration abort behavior, annotations, and declarative forms, and MCP’s list/call model. These are documentation checks, not browser interoperability tests. [Chrome imperative API](https://developer.chrome.com/docs/ai/webmcp/imperative-api), [declarative API](https://developer.chrome.com/docs/ai/webmcp/declarative-api), [MCP tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).

**3. Concerns**

**C1 — BLOCKING — D3: The proposed analysis cannot establish read-only behavior.**

Resources and mutations use the same data-source interface. Both can receive mutable `Store` access and initiate external work: [runner/src/runner/source.rs:323](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/runner/src/runner/source.rs:323). A resource is not an enforced read capability.

The proposed first consumer demonstrates this: `library` calls `withDatabase`, whose initialization can execute `CREATE TABLE`: [app.ts:133](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.ts:133), [app.ts:28](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.ts:28). More generally, changing local state can activate gated tasks; following only action calls and `then` is insufficient.

There is also a semantic mismatch: unchecked `writes` does not justify `destructiveHint: false`, and consequential actions include more than deletion. WebMCP explicitly includes purchases and money transfers. [WebMCP annotation definitions](https://webmachinelearning.github.io/webmcp/#dictdef-toolannotations).

**Resolution:** Either make these author assertions with a limited structural lint and remove the proof claim, or specify an enforceable effect boundary covering sources and triggered work. Align the categories with the web’s actual meanings. Do not quietly add a general effect system to satisfy this small feature.

**C2 — BLOCKING — D5, D6, §6: Consent is promised more strongly than it is established.**

The draft names no actual browser API that reports whether `consequentialHint` is honored. The current specification describes an advisory signal used by agents to decide when to require confirmation; it does not establish the detection mechanism D5 requires. [WebMCP §6.4.5](https://webmachinelearning.github.io/webmcp/#consequential-annotation-for-tool-executions).

On macOS, D5 first requires a host sheet, then makes that sheet conditional on the window being onscreen. MCP elicitation capability only establishes that a client implements elicitation; its affirmative response does not independently prove human approval. Furthermore, an `NSAlert` outside the Exact tree is outside this RPC interface, but not inherently beyond an agent with general OS computer-use access.

D6 also conflates author opt-in with caller authorization. A `0600` socket authorizes the same UID, not a particular chosen agent.

**Resolution:** State the threat model explicitly. For v1, adopt Q2’s recommendation to omit web destructive tools and adjust D10’s Chrome demonstration. Require native host approval whenever a destructive call can execute, bind it to immutable arguments and the selected session, and recheck availability afterward. Specify whether all same-UID clients are trusted. Remove §6’s unconditional claim that the hints prevent unasked destruction.

**C3 — BLOCKING — D4, §6: Serializing events does not isolate asynchronous calls or their results.**

Consider:

1. Agent A calls `findNotes("rain")`.
2. The person or agent B changes the query to `"sun"`.
3. A evaluates `returns library` after waiting.

Nothing specified prevents A from receiving the `"sun"` result. Fieldnotes’ query and library are shared root state: [app.contract:58](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.contract:58). The runner replaces requests by target, drops superseded replies, and explicitly notes that an already-sent POST is not undone: [commit.rs:933](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/runner/src/runner/commit.rs:933).

The claimed reusable wait is also wrong. The shipped operation is `clock data`, not `clock replies`: [scripts/agent.mjs:1205](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/scripts/agent.mjs:1205). It waits on global pending work and continuations, rather than requests owned by one invocation: [host/web-js/agent.js:283](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/web-js/agent.js:283).

**Resolution:** Define invocation ownership, completion, supersession, and the exact point at which results are captured. Cover queued sends, subsequent `then` work, existing requests, streams, user interference, disconnects, and cancellation. A timeout after possible persistence must communicate an uncertain outcome and must not invite automatic resubmission. LLP 1101.003’s whole-process completion rule cannot simply be reused inside a continuously running GUI.

**C4 — MAJOR — Summary, D6: “Secrets cannot leave … by construction” is false.**

There is a direct counterexample in the repository: a source reads `store.get("token")`, returns it as a string resource, and the test asserts that the resource contains that value. See [runner/tests/it/now_screen.rs:84](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/runner/tests/it/now_screen.rs:84) and its assertion at line 141. The typed encoder emits strings without a secrecy distinction.

LLP 1018’s application pattern is not a language-level information-flow guarantee.

**Resolution:** Replace the absolute claim with the actual boundary: tool declarations and source implementations must select safe outputs. Include errors and journal messages in that responsibility. If enforceable non-disclosure is required, specify its mechanism and cost separately; it does not already exist.

**C5 — MAJOR — D2, D7(b): The root belongs to a session, not uniquely to an app.**

The Mac host creates a separate `ExactSession` for each document window: [host/apple/Sources/ExactMac/main.swift:132](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/apple/Sources/ExactMac/main.swift:132), with `makeSession` at line 161. Thus an app-ID socket can have multiple roots, different availability, different drafts, and different locations.

“Root only” prevents row-instance collisions but does not identify which window receives a call. Running in one process also does not itself guarantee one logical database writer.

**Resolution:** Specify session selection and lifetime. Bind discovery, calls, results, and approval to that session and its plan generation. Define behavior when its window closes, another becomes frontmost, or its plan changes. Refusing ambiguous sessions is an acceptable small v1.

**C6 — MAJOR — §1, D7(a), §4: The web target is not a thin projection into the Rust runner.**

The claim that `runner/src/agent.rs` implements reads once “for every host” is incorrect for the shipped JS target. It has its own commit/action runtime: [host/web-js/rt.js:162](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/web-js/rt.js:162), action compilation in [emit.rs:428](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/web-js/src/emit.rs:428), and separate inspection code.

Consequently, `execute → runner` omits the JS implementation of call tracking, result evaluation, error handling, and command policy. Its existing inspection adapter is excluded from production: [build.mjs:279](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/web-js/build.mjs:279).

Discovery also needs an activation decision: interaction-activated pages do not load `app.js` merely because first paint occurred: [capture.js:1](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/web-js/capture.js:1).

**Resolution:** Add the JS compiler/runtime work explicitly, including registration before a person interacts, data-module readiness, production-safe encoding, and parity cases with Rust. Revise the scope estimate after doing that accounting.

**C7 — MAJOR — §2, D2, D10: Calling an action does not preserve all UI safeguards.**

Many Fieldnotes safeguards are on controls, not inside actions. For example, the save button’s disabled condition is at [app.contract:261](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.contract:261), while `save` itself sends unconditionally at line 115. Calling the action bypasses those view conditions.

The proposed consumer has a more specific hazard: `addNote` returns the existing `saved` mutation, while `editingId` and the editor’s reference fields depend on `saved.version == session.version`: [app.contract:66](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.contract:66). An implementation using the current session version could switch the editor’s effective note to the agent-created note.

**Resolution:** Specify the actual consumer actions and their draft-isolation rules, including deleting the currently edited note. Move shared business preconditions into shared action logic where necessary. Qualify §2’s claim: action guards are preserved; view guards are not automatically inherited.

**C8 — MAJOR — D2, D4, D7: Result and failure semantics need a complete wire contract.**

Arbitrary Contract expressions can return scalars, lists, or nullable values. MCP structured results are objects, so “encode like `state`” does not define the MCP envelope or output schema. MCP also distinguishes tool failures from protocol failures. [MCP schema](https://modelcontextprotocol.io/specification/2025-11-25/schema), [tool results and errors](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).

The proposed failure rule misses the consumer’s normal failures. Fieldnotes catches save/delete errors and returns ordinary records containing `failed: true`: [app.ts:143](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.ts:143), [app.ts:201](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.ts:201). These are not runner resource failures. A call without `returns` could consequently report `{"ok":true}` despite failure.

Finally, [`Value::conforms`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/plan/src/value.rs:240) returns a boolean, not the field-path diagnostic D4 promises.

**Resolution:** Define the envelope, nullable/omitted arguments, unknown fields, validation diagnostics, domain failures, commit refusals, and timeout outcomes. Pin the MCP version and specify initialization, duplex elicitation, framing, cancellation, and bounded messages. These are externally observable decisions, not merely compiler syntax choices.

**C9 — MAJOR — D3–D4: Silently skipping commands breaks the promised action semantics.**

Suppressing focus is reasonable. Silently suppressing a picker can leave an action partly executed, awaiting an event that will never occur, while returning success. Fieldnotes already separates picker initiation and completion: [app.contract:130](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/apps/fieldnotes/app.contract:130).

“During a tool call” is also ambiguous while human input and asynchronous continuations continue. A global suppression flag could suppress the person’s own commands.

**Resolution:** Define command disposition per originating invocation. Explicitly refuse unsupported interactive workflows or deliver their normal cancellation outcome. Cover other user-interaction-dependent operations, rather than asserting that everything else follows the press path.

**C10 — MAJOR — D8, D10; sibling consistency: The proposed fixture cannot be supplied by the named state model.**

LLP 1106 D1 explicitly supplies a *plain-store* snapshot and fetch-answer substitution. Fieldnotes’ notes live in SQLite. The existing native snapshot collects secret-store entries and retained key/value answers, not SQLite files: [host/apple/src/store.rs:292](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/apple/src/store.rs:292).

Thus “a store snapshot of three notes” is not an implementation-ready fixture. Adding `call` to authored tests also requires parser, runner, and result-assertion changes absent from §4.

**Resolution:** Seed a named scratch database through existing app operations or an explicitly described fixture setup. State that LLP 1106’s launch vocabulary governs tests; production calls attach to existing sessions. Compile test `call` through the actual tool boundary and include tests for concurrency, refusal, timeout, consent denial, and draft preservation.

**C11 — MAJOR — D8, D10: The admission and the proposed “take” remain unresolved.**

The exclusions in D10 are sensible boundaries, but keeping unimplemented alternatives out does not remove anything from the current doing-list. The rule requires an actual take or Charlie’s waiver: [rules/RULES.md:47](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/rules/RULES.md:47).

Using `tree` and a `tap` form is consistent with the operation-count rule’s wording. However, the existing API expressly says agents never invoke actions by name: [LLP 1012:318](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/llp/1012-agent-api-v1.spec.md:318). This needs a narrow amendment admitting declared production tools, not an implicit reinterpretation that exposes arbitrary actions.

**Resolution:** Record the actual trade or waiver, implementer, and implementation date before admission. Amend the existing boundary explicitly. The human-requested draft itself is legitimate; it is not yet authorization to build every mechanism described.

**C12 — MINOR — §1, D1–D2, §8: Correct the remaining factual inventory.**

- **Inferred parameters do have schemas.** Types are inferred before lowering, and lowering writes them into action parameters: [component.rs:208](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/contract/types/src/component.rs:208), [lower/src/lib.rs:346](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/contract/lower/src/lib.rs:346). Explicit annotations can be a policy choice, but D2’s technical rationale is wrong.
- **Android host code exists.** See [host/linux/src/android.rs:1](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/linux/src/android.rs:1) and its conditional export at `host/linux/src/lib.rs:41`. Android remaining deferred does not establish absence of implementation.
- **Not every host exposes a real accessibility tree.** Linux returns an explicit unavailable result: [host/linux/src/agent.rs:651](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/linux/src/agent.rs:651).
- **Agent execution is not universally development-only.** Qualify the summary to the gated GUI driver; the existing terminal headless command path is an exception.
- **App Intents do not universally execute in the main app process.** An app-only execution design is possible, but Apple also supports separate extension processes. [Apple’s documentation](https://developer.apple.com/documentation/appintents/creating-your-first-app-intent).
- **Browser status understates the situation.** Edge has an origin trial, and WebKit’s recorded position is opposition, not merely an unspecified commitment. [Implementation status](https://github.com/webmachinelearning/webmcp/blob/main/implementation-status.md), [WebKit position](https://github.com/WebKit/standards-positions/issues/670).

**Resolution:** Update §1 and §8 to distinguish verified implementation, governing policy, platform documentation, and proposed behavior.

**4. Suggestions**

- Write one complete invocation lifecycle and two worked examples: overlapping searches, and a save that persists but times out before its result arrives. Use those to settle D4.
- Keep the first implementation narrowly bounded: explicit session selection, conservative concurrency handling, host-confirmed native deletion, and web tools whose required consent can actually be supported.
- Explain the distinction from LLP 1101.003’s rejected action exports: these are deliberately declared public operations, rather than automatically exposing every action.
- Clarify compatibility identity. LLP 1030 D3a hashes requirements an installed binary cannot replace. Explain which tool-table fields constitute that ceiling and why a description edit should—or should not—create a new cohort.

**5. Open questions for the author**

1. Are consequence declarations trusted author assertions, or intended enforceable guarantees?
2. Does native v1 trust every same-UID process, and what qualifies as human approval?
3. What should happen when a person supersedes state used by an outstanding tool call?
4. Which live window/session does an app-ID connection select?
5. How will Fieldnotes’ tool results be isolated from its editor’s mutation slots?
6. What existing work is the actual take, or is a waiver being requested?

Verdict: NOT READY
## Round 2 (2026-10-07), on r2

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `2ce193976` holding the three r2 drafts and only Astra's own r1 reviews; 2026-10-07T20:14:49Z to 2026-10-07T20:26:27Z.
- **Method:** one round-2 brief per RFC (sha256 `695a861a163c54033d977201591f34549f640c32edcd5c03320c7dd6c2e58f21`), shared verbatim with the other reviewer; blind to the other family's reviews, though r2's dispositions section names concerns both raised. Requested by Charlie ("ok sounds good" to a second round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r3 (2026-10-07) shrinks LLP 1105 to the local MCP slice for Fieldnotes on macOS and takes every round-2 concern; §9 maps each. N1: a pending result input is refused `busy`, fulfilment commits are owned, each replacement path (`forget`, `enqueue`'s drop, the kept ticket) has an outcome, streams and background work are refused. N2: phases, a certainty on every outcome, consent outside the bound, no response on cancel. N3: `fails when` on all three tools and a delete policy with tests. N4: the session rule is sole, named, then last key. N5: all 32 commands classified. N10: `call` returns at the consent hold. N11: the bearer-token claim is removed. N6, N8, N9 and N12 move with WebMCP and the connectors to §4 as direction. Nothing is declined.

**1. Overall assessment**

R2 is substantially better and now matches Charlie’s aim: the author declares a small public surface, while Exact supplies schemas, execution, transports, and connector infrastructure. Inferred parameters, reuse of existing actions, and staged connectors are appropriate choices.

It is **not yet specified enough to build the first slice without guessing**. Invocation ownership still leaves cases that can return an unrelated or unfinished result. The outcome table does not consistently distinguish rejection before execution from failure after effects may have occurred. The concrete Fieldnotes example and consent-testing path also remain incomplete.

I reviewed [LLP 1105 r2](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/llp/1105-apps-as-tools-for-agents.rfc.md), the r1 review, governing instructions, cited code, relevant existing LLPs, both siblings, and primary protocol documentation. This was a source review at `2ce193976`; I made no edits and ran no builds or tests.

The siblings now agree on several important boundaries: launch inputs contain no store snapshot; resource reads use whole-instance data settlement; calls in an existing GUI require a different completion rule; and explicitly declared tools differ from exporting every action. The remaining inconsistencies concern deadlines, discovery, and execution lifetime.

**2. R1 concerns**

| Concern | Status and evidence |
|---|---|
| **C1 — Read-only proof** | **Partly resolved.** D3 explicitly makes consequence an author assertion and permits opaque source effects; the annotation mapping still overstates what follows from those assertions—N7 below. |
| **C2 — Consent and caller trust** | **Resolved at the policy level.** D5 binds native confirmation to displayed arguments, rechecks availability, rejects elicitation as consent, and excludes web/connector `destroys`; D7 explicitly trusts same-user callers. |
| **C3 — Async isolation** | **Partly resolved.** D4 adds ownership, capture and uncertainty, but existing-request reuse, retained tickets and terminal outcomes remain incomplete—N1–N2. |
| **C4 — Secret non-disclosure** | **Partly resolved.** D6 acknowledges unrestricted resource outputs, but still says a bearer token cannot leave; the original counterexample still applies—N11. |
| **C5 — Session identity** | **Partly resolved.** D7 pins a connection to a session and generation, but choosing only the current key window breaks ordinary background-app use—N4. |
| **C6 — Separate JS runtime** | **Resolved in the implementation accounting.** D7(b) and §4 now explicitly include JS compilation, encoding, ownership, activation and parity; this matches `host/web-js/rt.js:162` and `src/emit.rs:428`. The new availability/discovery contradiction is N6. |
| **C7 — UI guards and draft isolation** | **Partly resolved.** §2 correctly distinguishes view guards from action guards, and `created` avoids the editor’s `saved` slot; deletion of the currently edited note remains unspecified—N3. |
| **C8 — Wire results and failures** | **Partly resolved.** D4 pins MCP, defines an object envelope and decoder, and adds `fails when`; phase-sensitive errors and cancellation remain incomplete, and the example omits two domain-failure mappings—N2–N3. |
| **C9 — Command suppression** | **Partly resolved.** Suppression is invocation-scoped and skips are reported, but the refusal list omits existing interactive commands—N5. |
| **C10 — Fixtures and authored tests** | **Partly resolved.** Seeding through `addNote` fixes the SQLite/snapshot mismatch and agrees with 1106 D1; the consent hold cannot yet be driven through the specified synchronous call interface—N10. |
| **C11 — Admission and take** | **Partly resolved.** D10 supplies the narrow LLP 1012 amendment; D11 correctly admits that exclusions are not a take. The waiver, implementer and implementation date remain outstanding. |
| **C12 — Factual inventory** | **Resolved for the enumerated corrections.** Inference is supported by `contract/types/src/component.rs:208` and `contract/lower/src/lib.rs:346`; Android and absent Linux AX are accurately described. **My r1 claim that Linux lacks TypeScript support was wrong:** `apps/messages/linux/src/main.rs:5` and `js/build.rs:187` support r2’s correction. |

**3. New and remaining concerns in rewritten decisions**

**N1 — BLOCKING — D4: Owning newly started work does not establish that the returned value answers this call.**

D4:275 owns requests the invocation starts; D4:283 captures at its last settlement. Consider a person starting `search("rain")`, followed by a tool calling the same search while that resource remains pending. No new resource request is necessary. The runner can reuse the standing value while retaining the existing request, explicitly distinguishing that value’s arguments from the current ask: [settlement.rs:421](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/runner/src/runner/settlement.rs:421), especially the reuse condition at line 463. The tool owns no new request, yet `library` may still answer an earlier query.

There is also an existing path that **keeps the ticket but replaces its arguments**, parsing the eventual reply for the newer arguments: [commit.rs:963](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/runner/src/runner/commit.rs:963). LLP 1054.000.000 D3 explicitly requires this. Watching only forgotten/replaced requests misses it.

Finally, “the last owned request” does not define completion for synchronous `Answer::Now`, no-op actions, or an invocation whose work becomes LLP 1097 background storage. Background work has a separate, module-wide ticket outside ordinary enqueueing: `runner/src/runner/background.rs:1`.

**Resolve:** Define completion against result-dependency generations, including `fails when`; choose whether existing pending dependencies are joined or refused; define synchronous completion and retained-ticket retargeting. Explicitly support or refuse source streams/background work in the first slice.

**N2 — BLOCKING — D4–D5: Terminal outcomes conflate rejection with uncertain effects.**

The new table gives uncertainty wording only to `unknown`. However:

- A write reported as `superseded` may already have persisted. The cited [forget path explicitly says an already-sent POST is not undone](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/runner/src/runner/commit.rs:933).
- A runtime `openAuthSession` or permission refusal can follow an earlier successful effect in the same invocation.
- Cancellation while awaiting consent occurs **before** the action runs, contradicting the universal `unknown` message that “the action ran.”
- `idempotentHint: false` describes repeat effects; it does not establish a protocol rule forbidding retries. [MCP annotations](https://modelcontextprotocol.io/specification/2025-11-25/schema#toolannotations)

The RFC also needs to say what cancellation, disconnect, and turning Allow Agents off do to pending confirmations and undispatched work. Sending an `unknown` response is not sufficient notification after cancellation: MCP clients are advised to ignore subsequent responses. [MCP cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation)

**Resolve:** Specify a lifecycle with validation, awaiting consent, dispatched, completing and terminal phases. Give every failure an explicit effect-certainty result. Invalidate confirmations when their invocation or generation ends, and choose what happens to outstanding and queued work without implying rollback.

**N3 — MAJOR — Summary, D2, D11: The revised Fieldnotes example still fails its stated guarantees.**

Only `addNote` has `fails when`. Yet `deleteNote` catches failures and returns ordinary `Status{failed:true}` at [app.ts:201](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/apps/fieldnotes/app.ts:201); `library` returns `ready:false` on failure at line 140. Both tools therefore report D4’s `ok` for normal application failures.

The new `deleteById` also leaves the editor’s session unchanged. Deleting its current note leaves an editor pointing at a deleted identity; the next save fails at [app.ts:149](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/apps/fieldnotes/app.ts:149). The existing UI delete explicitly resets the session at `app.contract:163`. Preserving an orphaned draft might be intentional, but that policy is not stated.

**Resolve:** Supply complete declarations for all three tools, including failure predicates. Choose and test what deletion means for a clean or dirty editor holding that note. State whether overlapping tool writes queue or may supersede one another.

**N4 — MAJOR — D7(a): “Key window or refuse” obstructs the primary consumer.**

The person will commonly start Claude Code while Terminal is foreground and Fieldnotes is running behind it. AppKit’s `keyWindow` may be nil when the app is inactive. [Apple documentation](https://developer.apple.com/documentation/appkit/nsapplication/keywindow)

The existing Mac host already anticipates this: [frontWindow():328](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/apple/Sources/ExactMac/main.swift:328) falls back from key to main to the newest window. D7:393 instead refuses the connection.

**Resolve:** Define selection independently of current keyboard focus—an explicit session choice, the sole eligible session, or a recorded last-selected session. Keep the resulting binding fixed, and demonstrate connecting while the app is inactive.

**N5 — MAJOR — D3–D4: The command policy is incomplete for existing commands.**

D4’s compile-time refusal names `showPicker` but omits `showOpenFilePicker`, `showDirectoryPicker`, and `showSaveFilePicker`. These are separate commands that display system UI, not merely data-module requests: [runner/src/commands.rs:25](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/runner/src/commands.rs:25).

It also refuses `playSound` but leaves `playSounds` unclassified. Other existing commands include `showModal`, `openURL` and bare `close()`, which bypasses `beforeunload`: [HOST_COMMANDS:83](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/contract/syntax/src/lib.rs:83).

Separately, D3:235 appears to reject every command reached by a `reads` action, while D4 permits skipped focus/blur/scroll commands.

**Resolve:** Classify the existing command vocabulary completely, with an explicit default for unclassified commands. Reconcile the lint with the skip exceptions. Either include resource-triggered and gated-task work in the advertised reach listing or stop describing it as “everything it can reach.”

**N6 — MAJOR — D2, D7(b), D8: Dynamic availability conflicts with static discovery.**

D2:214 requires `available when` to be evaluated over live state and suppress unavailable tools from discovery. D8:429 promises `tools/list` from the deployed plan **without booting anything**. A predicate depending on a resource, mutation or `pending(x)` cannot generally be evaluated that way.

The WebMCP registrar has the same problem: it publishes before `app.js` activates, although that module constructs the state: [build.mjs:273](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/web-js/build.mjs:273).

**Resolve:** Choose a discovery policy per door: activate before listing, publish a static catalogue and check availability on execution, or restrict predicates for stateless connectors. Specify list-change behavior and readiness before the first call.

**N7 — MAJOR — D3: The MCP annotation mapping uses the wrong meanings.**

`openWorldHint:false` does not mean “restricted by grants.” An API-backed search remains open-world even when it reaches one granted origin. Likewise, `destructiveHint:false` means updates are additive, not merely that the operation lacks a `destroys` label. [MCP’s definitions](https://modelcontextprotocol.io/specification/2025-11-25/schema#toolannotations)

The RFC supplies neither an additive-only definition of `writes` nor a closed-domain restriction that justifies these mappings.

**Resolve:** Keep conservative protocol defaults unless the author asserts the corresponding property, or define the Exact categories precisely enough to imply those annotations. Grants should not determine `openWorldHint`.

**N8 — MAJOR — D8–D9: The borrowed deadline contract does not work for every execution environment.**

D9 places a Mac resource read in a windowless session **inside the running app**, then says it runs within LLP 1106 D11’s deadlines. That sibling’s hard deadline works by killing a child process after 2.5 seconds. There is no disposable child here, and Rust sources need not expose an interrupt: [DataSource::interrupt:461](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/runner/src/runner/source.rs:461).

For anonymous calls, D8 imports a two-second total budget while D4 describes a ten-second invocation wait. The intended override and outcomes before versus after dispatch should be explicit.

**Resolve:** Provide a per-door deadline/lifetime table covering boot, consent, execution, cleanup and the returned error. Specify how local resource sessions stop without terminating the person’s app, or defer that local resource mode.

**N9 — MAJOR — Summary, D11: Caltrain’s new wall-clock prerequisite is insufficient.**

The data crate also fixes every departure to **2026-08-28**, independently of the Contract clock: [DAY_START_MS:118](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/apps/caltrain/data/src/lib.rs:118), used at line 189. `board` filters those departures against `now_ms` at line 332.

Consequently, replacing only the Contract’s demo clock with today’s wall clock filters out the entire schedule. Moreover, this source is explicitly a seeded corridor model, not a live timetable.

**Resolve:** Choose whether stage 2 demonstrates a clearly labelled simulation or real upcoming trains. Include the corresponding data-source work in the prerequisite and estimate; the promised public result is not presently a two-line change.

Also record the public front door as a **stage-2** prerequisite or name the demonstration’s tunnel: the current server binds loopback at `host/render/src/serve.rs:155`, and LLP 1082 remains Draft.

**N10 — MAJOR — D10: The consent test cannot be expressed through the specified reply sequence.**

D10 says `tap {tool}` returns the call’s outcome, while a destructive call waits for `tap @ticket`. An authored test executes and awaits each input before proceeding: [agent-test.mjs:222](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/scripts/agent-test.mjs:222).

Thus `call deleteNote …` cannot finish until a later step approves it, but the later step cannot run until the call finishes. The hold is deliberately absent from `state.pending`, and neither the ticket’s discovery location nor an intermediate reply is specified.

**Resolve:** Show a complete executable test transcript for approval and denial. Define how the test obtains the hold, continues while the invocation remains pending, and later reads that same invocation’s terminal outcome.

**N11 — MAJOR — D6: The remaining bearer-token guarantee is still false.**

“A bearer token cannot leave” remains an absolute information-flow claim. LLP 1018 D5 describes an application pattern: its source deliberately omits the token from returned values. It does not create a protected value type.

The counterexample is unchanged: [now_screen.rs:93](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/runner/tests/it/now_screen.rs:93) returns `store.get("token")` as a resource, and `runner/src/agent.rs:1098` serializes strings normally. Calling that string a bearer token changes nothing.

**Resolve:** Say that the store is not automatically exposed, and that sources following LLP 1018 keep tokens out of returned values. Apply the author’s output-selection responsibility to **all** secrets, including bearer tokens.

**N12 — MINOR — §1, D8: MCP does not universally require remote authentication.**

The statements that a remote MCP server “is an OAuth 2.1 resource server” and that MCP requires this omit the protocol’s optional-authentication qualification. Anonymous servers are permitted. [MCP authorization requirements](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)

**Resolve:** Scope those statements to the signed-in connector. Keep the separate backend-token rule, which is correctly stated.

**4. Suggestions**

- Preserve the small author-facing declaration. A conservative first slice may serialize tool calls and explicitly refuse unsupported dependency situations; it need not introduce a general effect system.
- Specify four complete traces before implementation: an already-pending identical search, a superseded write, cancellation during confirmation, and consent denial through an authored test. Include exact result JSON.
- Add a compact transport profile covering connection/message bounds, cancellation routing and HTTP handling. The render server currently rejects POST at `serve.rs:513` and reads only request headers at `serve.rs:637`; Streamable HTTP needs deliberate body handling and Origin validation. [MCP transport requirements](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
- Make an app created with `exact new` part of acceptance. Its generated wrapper has a fixed verb table and rejects unknown verbs at `game/new.mjs:500`; §4 should include the wrapper change and the actual client configuration/deploy instructions.
- Record the implementer, date and take or waiver before admission. The revised acknowledgement is honest, but is not itself an admission.

**5. Open questions for the author**

1. May a first-slice call join an existing pending result dependency, or must it refuse that situation?
2. What exactly continues after cancellation or supersession, and how does the result describe effects that may already have happened?
3. What should Fieldnotes do when an approved deletion targets the note currently being edited?
4. Should stage 2 include local windowless resource sessions, or only anonymous connector resources?
5. Is the Caltrain proof a simulated schedule or real departures, and what public front door makes its connector reachable?

Verdict: NOT READY
## Round 3 (2026-10-07), on r3

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `2ce193976` holding the three r3 drafts and only Astra's own r1 and r2 reviews; 2026-10-07T20:54:22Z to 2026-10-07T21:27:33Z.
- **Method:** one round-3 brief per RFC (sha256 `30fb735a98174484bff4f42c617a36b2621d97240d59294a9764b933fc1ec05c`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("sounds good, try that" to the narrowed r3 and a third round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** pending; the loop stops here after three rounds; not revised against this review.

**1. Overall assessment**

**The scope reduction works, but the first slice is not yet implementation-ready.** The remaining problems concern completion, cancellation, consent concurrency, window selection, and executable acceptance tests—not the deferred connectors.

Section 4 clearly labels the later doors as unspecified and records the substantial open problems. I have not counted their incompleteness against the verdict. The siblings now correctly distinguish whole-tree settlement from invocation-local completion: [1106 D1a](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/llp/1106-a-url-under-a-state-as-an-image.rfc.md:157) explicitly names 1105’s difference. The launch-record defaults do not govern a connection to an already-running Fieldnotes session.

The principal current-behavior claims check out: production GUI gating, action-parameter inference, typed JSON encoding, per-window sessions, Fieldnotes’ failures-as-data, and the three request-replacement paths. Specific corrections appear below.

This was a read-only review at `2ce193976`, including both earlier reviews, binding instructions, the siblings, cited implementation, and primary MCP documentation. I made no edits and ran no builds or tests.

**2. Round-2 concerns**

| Concern | Status and evidence |
|---|---|
| **N1 — Invocation completion** | **Partly resolved.** Pending-resource refusal and kept-ticket handling address the earlier examples; deferred sends and separately scheduled continuations still escape the completion predicate. R3-1 below. |
| **N2 — Outcomes and cancellation** | **Partly resolved.** Phases, consent withdrawal, and pre-dispatch certainty are specified; post-dispatch cleanup and resource-supersession certainty remain inconsistent. R3-2. |
| **N3 — Fieldnotes declarations and deletion** | **Partly resolved.** All three failure predicates and the clean/dirty deletion policy are supplied; the acceptance sequences do not reliably exercise them. R3-4. |
| **N4 — Inactive-app session selection** | **Partly resolved.** Remembering the last key session fixes the Terminal-in-front case; explicit window selection can still be silently ignored. R3-6. |
| **N5 — Command classification** | **Resolved.** D4a covers every entry in `contract/syntax/src/lib.rs:83`; there are **31**, not 32. |
| **N6 — Static versus live discovery** | **Moved.** Section 4 names the WebMCP/connector discovery decision; local availability is evaluated live in D2. |
| **N7 — Annotation meanings** | **Resolved at the assertion level.** `additive`, `idempotent`, and `closed` provide corresponding author assertions; grants no longer determine open-world status. D2–D3. |
| **N8 — Borrowed deadlines** | **Moved.** Windowless reads and connector deadlines are direction; local calls have their own dispatch-relative bound. |
| **N9 — Caltrain and public hosting** | **Moved.** Section 4 honestly records the fixed timetable, simulation-versus-real-data choice, and missing public front door. |
| **N10 — Consent-test deadlock** | **Resolved.** D10 returns at the hold and supplies a separate `consent` step. The remaining test sequencing problem is R3-4. |
| **N11 — Bearer-token guarantee** | **Resolved.** D6 explicitly acknowledges unrestricted source outputs, consistent with `runner/tests/it/now_screen.rs:93`. |
| **N12 — Mandatory remote authentication** | **Moved, with the correction made.** Section 4 permits anonymous MCP, consistent with the [authorization specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization). |
| **Carried C11 — Admission, implementer, date** | **Not resolved.** D11 acknowledges all three remain outstanding; acknowledgement does not satisfy `rules/RULES.md:47` and `:58`. |

**3. New concerns**

**R3-1 — BLOCKING — D4: “Every owned request has settled” still permits premature success.**

D4 owns *enqueued requests* and declares an action that enqueues nothing complete at its commit. That does not encompass all work the current runner can retain.

- Before a TypeScript source is ready, a send goes into `unsent`, without calling the source or creating a request ticket: [commit.rs:643](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/runner/src/runner/commit.rs:643). D7 admits a session once its window is open and its plan has tools. An early Fieldnotes `addNote` can therefore enqueue no request and return `ok/applied` with `created == none`, before saving anything.
- A mutation’s `then` runs in a **separate commit**, after the answer’s commit: [commit.rs:530](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/runner/src/runner/commit.rs:530). Fulfilment removes the request before arming that continuation (`commit.rs:1217–1224`). There can be zero owned requests while owned continuation work remains.
- A queued send can wait without being asked: [queue.rs:109](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/runner/src/runner/queue.rs:109). Its eventual `next` is another commit.

The parenthetical “its `then` … its queued sends” does not reconcile these paths with the explicit request-only completion rule. Notably, 1101.003 D2 explicitly accounts for `then_due` and queued sends.

**Resolution:** Define an invocation’s outstanding work to include deferred sends, queue entries, and scheduled continuations, with ownership transferred across their commits. Complete only after that work is exhausted, and define outcomes for action/continuation commit errors. A smaller first slice may instead refuse calls before data readiness and explicitly reject unsupported continuation/queue patterns.

**R3-2 — MAJOR — D4: Terminal response and execution lifetime remain conflated.**

After cancellation, D4 says nothing is undone and the journal records uncertainty. After timeout it returns a terminal outcome. Neither specifies whether undispatched continuations and queued sends continue, whether late replies still commit, or when the session becomes available to another call.

Those choices have observable consequences. Releasing the session immediately allows a new `addNote` while the previous source operation remains active; retaining it needs a defined release condition. The existing runner deliberately distinguishes forgetting a reply from undoing an effect: [commit.rs:933](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/runner/src/runner/commit.rs:933), and [source.rs:444](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/runner/src/runner/source.rs:444).

There is also a direct contradiction in the certainty table. `applied` means owned requests settled, but superseding a resource receives `applied` even when its ticket is merely retargeted and remains pending (`commit.rs:980–994`). An invocation can additionally own both a send and that resource; certainty cannot depend solely on which ticket caused termination.

**Resolution:** Specify post-terminal handling of each work category and the admission-lock lifetime. Derive certainty from the entire invocation. A superseded, unsettled resource does not satisfy the current definition of `applied`; use `uncertain` or revise the definition explicitly.

**R3-3 — MAJOR — D4–D5, D11: Serialization excludes consent and does not match the app-modal policy.**

D4 refuses competing calls only between **dispatch and terminal**, leaving `awaiting consent` outside the exclusion. Consequently, another call can arrive while the first confirmation is pending. D5 specifies one app-modal confirmation and D10 exposes one `@consent` hold, without defining arbitration.

There is a second interaction across windows: a call in window A may already be dispatched when window B asks for consent. D5 pauses worker answers while the alert is open, potentially for 60 seconds, while A’s 10-second wall-clock deadline continues.

Finally, D11’s assertion that overlapping tool writes cannot happen is broader than D4’s **per-session** serialization. Windows have separate sessions ([main.swift:159](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/apple/Sources/ExactMac/main.swift:159)); Fieldnotes creates a module instance through its data factory ([apple/src/lib.rs:10](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/apps/fieldnotes/apple/src/lib.rs:10)). The module-local `serial()` is not an app-wide lock.

**Resolution:** Reserve invocation admission before showing consent. Specify app-wide confirmation arbitration and its effect on other sessions’ deadlines. Either serialize tools app-wide for this slice or explicitly limit the serialization guarantee and cover cross-session calls.

**R3-4 — MAJOR — D10–D11: The acceptance tests assume settlement that D4 deliberately excludes.**

The added `toolRevision` makes `library` refresh after `created` or `deleted` changes. However, those tools return their mutations, so D4 explicitly does **not** wait for the library refresh.

The tests immediately:

- tap `note-1` after `addNote`, although the refreshed row may not exist yet;
- type after opening that note, although `openNote` may still be pending;
- call `findNotes` after a write, although its pending-library check may return `busy`.

Current Fieldnotes disables note buttons while busy and rejects editor changes while `opened` is pending: [app.contract:95](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/apps/fieldnotes/app.contract:95), `:245`, `:258–259`. Authored input steps do not automatically settle data; clock steps are explicit: [agent-test.mjs:184](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/scripts/agent-test.mjs:184), `:241–270`.

Thus these are races, not reliable acceptance transcripts.

**Resolution:** Add explicit `clock data`/appropriate settlement steps before dependent UI operations and reads. Define how `call` and `consent` pump their owned work under the frozen agent clock. Preserve D4’s production completion boundary rather than silently making tests wait for the whole app.

**R3-5 — MAJOR — D4: The generated output schema does not describe the supplied refusal example.**

D4 types `value` from `returns`. For `findNotes`, that is the non-optional `Library` record, declared at [app.contract:7](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/apps/fieldnotes/app.contract:7). Yet trace 1 returns `value: null`.

The state encoder only naturally emits null for unit or absent options; it does not make every record nullable ([agent.rs:1087](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/runner/src/agent.rs:1087)). MCP requires structured results to conform to an advertised output schema. [MCP output schemas](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#output-schema)

**Resolution:** Specify the complete envelope schema, including required fields and nullability. Permit `T | null` where the result was not captured, or use outcome-dependent schema branches. Validate every example against that schema, including refusal, supersession, timeout, and oversized-result failure.

**R3-6 — MAJOR — D7: Explicit window selection can be ignored, and its transport is unspecified.**

The selection order chooses the sole eligible session **before** checking `--window`. If the requested window has closed and another remains, the relay silently connects to that other window.

Also, the relay is specified as forwarding unchanged MCP bytes over a single app socket. Neither `--window <label>` nor `--windows` has a defined message to the app. The app cannot derive the relay’s argv from those bytes. The existing development carrier solves its different case with an explicit `session` field and refuses an unknown label: [Agent.swift:138](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/apple/Sources/ExactKit/Agent.swift:138).

The generated wrapper also passes an app **name**, while D7 documents an **id**. Today’s `resolveApp` resolves names/directories, not reverse-DNS IDs ([app.mjs:476](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/scripts/app.mjs:476)).

**Resolution:** Honor an explicit selector first and refuse if absent. Specify the connection-selection/listing exchange, including zero eligible sessions. State whether the CLI accepts names, IDs, or both, and make the wrapper and examples follow that rule.

**R3-7 — MINOR — §1, D11, §4: Correct the remaining inventory and sibling references.**

- `HOST_COMMANDS` has **31** entries; D4a correctly lists all of them.
- The Summary’s additions occupy **13 lines**, not twelve.
- D11 cites `NativeModule.swift`’s native-module scratch roots for Fieldnotes’ storage. The relevant storage implementation is [picker.rs:20](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/apple/src/picker.rs:20), which uses the named agent scratch directory. The fresh-store claim itself is correct.
- Section 4 calls the remote hard deadline “LLP 1106 D11’s 503.” R3 D11 specifies local child termination and exit 124; server status selection remains direction, explicitly including an unresolved 500-versus-503 choice ([1106:281](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/llp/1106-a-url-under-a-state-as-an-image.rfc.md:281), `:391–396`).

**Resolution:** Correct the counts and storage citation; describe 503 as a proposed future choice rather than an existing sibling decision. This is a consistency correction, not a demand to specify the remote slice.

**4. Suggestions**

- Keep the first slice small by explicitly refusing unsupported execution patterns. Fieldnotes does not require general queue or continuation support to prove the feature.
- Add focused traces for a call before data activation, timeout followed by another write, and two connections arriving during confirmation.
- Emit `notifications/tools/list_changed` when live availability changes, as well as when the plan changes; D2 otherwise changes discovery without notifying clients.
- Test failed deletion with an edited note. The proposed action changes the editor session before the database result arrives; make that failure behavior intentional.
- Record the implementer, implementation date, and take or waiver before admission.

**5. Open questions for the author**

1. Should slice 1 refuse calls until the data module is ready and reject queued/`then` actions, or support ownership of those work items now?
2. After timeout or cancellation, what continues, and when may another tool call enter?
3. Should the first slice serialize invocations app-wide, including consent, given the app-modal alert?
4. Who implements it, when, and what take or waiver admits it?

Verdict: NOT READY