# Code review: native abort, landing: the rebase onto main and door 23, round landing re-check 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `ea22acccaf`.
- **Method:** one brief (sha256 `79d2811cca7987e7a12d123a3be0802eb04fcadb0cb70db9227e715f7eb01484`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

Op 23 is the abort door everywhere it is dispatched, and op 22 stays module-work state. `js/src/door.rs` has both arms (`22` sets `module_outstanding`, `23` records the ticket). The prelude sends `host(23)` only for a claimed non-stream fetch and still withdraws an unsent one with op 16. The wasm page and worker realms handle `op === 23` before the store fallthrough (`host/web/module-glue.js`, `host/web/module-worker.js`); `glue.js` only wakes `exact_collect_aborts` and cancels on the batch op `"abort"`. Render has no prelude-door table of its own; it uses the native door and cancels through `settle_aborts`. Nothing else on those hosts owns op 23.

`TYPESCRIPT_RUNTIME` is 5, copied onto Hermes cohort requirements, and `docs/reference.md` names that version. An older door's unknown op is the caught path in `abortRequest`; an older prelude never sends 23 and would leave the request running, which is what the bump keeps off a new native TypeScript bundle.

The rebase kept both behaviors: `watchAbort` still calls `work.landed()` for an adopted fetch and then `abortRequest`; `take_aborted_answer` sits before `module_work_refusal` and `tool_refusal(allow_ordered)`, and `has_request_refusals` still includes module aborts, auth, module work, and tools; `note_retired(&p)` precedes `let_go_request(p)` at all three commit sites. `finish` remains in `host/web/src/host/live.rs`, and `complete` in `host/web/src/host/complete.rs` is main's batch end plus the abort collect and `batch.abort`.
