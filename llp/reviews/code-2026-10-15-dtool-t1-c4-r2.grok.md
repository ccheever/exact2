# Code review: LLP 1122 T1 commit 4 (editing and undo), round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `540fe0e2a` (commit `d3e6ad575` and its round-1 fixes).
- **Method:** one brief (sha256 `065c7291ec4c92997f8571343a4d06b75051b16744c7c6e4597d4f71feb3b1ca`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium, 1 low).
- **Disposition:** all taken, as in the astra record.

---

Round 1’s branch selection, ties, carried review, digest map, undo peek, and byte-level tests are fixed. Three problems remain. Redo can still write this session’s operation into another app on port 7740, Cancel leaves “only this mode” armed, and a platform-role value can be overwritten without Pin.

## High

**Redo is still not bound to the server it was minted on.** `apps/design-tool/app.ts:450` (`redoLast`), `app.ts:52` (`call`), `app.ts:484` and `app.ts:490` (`waitChanges`); `scripts/tamber-server.mjs:388`; `contract/cli/src/tamber_serve.rs:553`.

The comment at `app.ts:307` says a new epoch empties both stacks so a redo cannot write this app’s edit into another app on port 7740. That empty happens only inside a successful events reply, and only when `place.epoch` is already set. A failed poll returns at `app.ts:484` with the stacks untouched, and the retry waits 3 seconds (`app.contract:199`). `redoLast` does not look at epoch or `app`. It calls `hello`, and `call` on HTTP 404 drops the token and retries against whoever now holds 7740 (`app.ts:59`). Rust `hello` returns no epoch (`tamber_serve.rs:562`); the wire adds `app` and nothing else (`tamber-server.mjs:388`). The transaction then uses that server’s current digests and the operation copied off the stack before the await. A poll that clears the arrays during that await does not stop the write, and the new inverse is pushed onto the new session’s undo stack.

Undo an edit in app A, stop its server, start app B on 7740, and press Redo before the next events reply lands. Redo’s hello takes B’s token and writes A’s `set-dial` of `hue` (or any shared token name) into B. Undo is safe in that window: B has never issued the inverse id, so the call is refused and the entry stays. Apply usually conflicts, because it still sends A’s digests; it writes only when B’s file digest happens to match.

Put `epoch` on hello (the wire already has it). In `redoLast`, `undoLast`, and `writeEdit`, if `epoch` or `app` differs from the run the stacks and the review were minted under, clear both stacks, bump `session`, and refuse. Do that on a token 404 as well, so the gap is not “until the next successful poll.”

## Medium

**Cancel leaves “only this mode” selected, and the heading keeps the old branch.** `apps/design-tool/app.contract:286` (`cancelEdit`), `app.contract:253` (`cellText`), `app.contract:255` (`onlyThisMode`); `apps/design-tool/app.ts:403`.

`onlyThisMode` sets `scopeOwn` and `editWhen = branch.own`. `cancelEdit` clears `editKind` only. The next `cellText` still does `editWhen = scopeOwn ? branch.own : branch.when`. The heading is built in `loadBranch` from the winning `when` (`app.ts:403`) and never switches to `own`, and the button has no pressed state (`app.contract:422`).

On the fixture, open Dark · black+compact of `surface.card` (the cell shows the default), press “Only this mode”, then Cancel, type a color, and Apply. The write is `when dark-style=black density=compact`, the branch the button would have added, while the heading still says `surface.card · default`. The same mismatch is on screen for the whole time the proposal is up, before Cancel. `selectCell` does clear `scopeOwn` (`app.contract:243`); Cancel is the path that does not.

Clear `scopeOwn` in `cancelEdit`. While it is set, name `branch.own` in the heading so the branch on screen is the branch Apply will write.

**A value that is only a platform role is not kept as an expression.** `apps/design-tool/app.ts:240` (`inputsOf`) and `app.ts:404`; `tamber/src/check.rs:146` and `check.rs:970`; `apps/design-tool/stub/dials.tamber:37`.

D6 says editing an expression offers its inputs, and a literal overwrite is an explicit pin. `expression` is true only when `inputsOf` finds something in `explain`’s `names` or `dials`. Those `names` are token refs (`check.rs:146`). A platform role returns `Named::Role` and records no ref (`check.rs:970`), so both lists are empty. `text.system = -exact-secondary-label` therefore opens the raw field (`app.contract:421`). The round-1 alias case is fixed: a role that is a token in the file is reported as its row spelling, and `inputsOf` matches `LABEL` to `-exact-label`. This fixture value is the case that list does not contain. `app.test.contract:174` selects the cell and checks only the heading.

Apply `#ffffff` on that cell. The role binding is replaced with a literal, with no Pin step. Explain should report role references (row spelling). Treat them as expression inputs: label a platform role, and do not send it to `selectCell`. Pin stays the way to overwrite it.

## Low

**A client timeout can consume the inverse and leave the stacks backwards.** `apps/design-tool/app.ts:52` and `app.ts:438`; `scripts/tamber-server.mjs:73` and `tamber-server.mjs:208`.

`call` gives up after 15 seconds and returns `NO_SERVER` with `reply: {}`. `conflict` is false, so `undoLast` keeps the entry and pushes nothing onto redo. The wire’s own wait is 120 seconds, and `done` still announces the undo after the HTTP client has gone. The inverse is removed only on success (`tamber_serve.rs:760`).

Undo a large design and let the tool time out while the server finishes. The file is restored, Undo still shows 1, Redo shows 0. The next press gets “no inverse” and, because that refusal is not a conflict, the button stays. On a timeout, re-read the digests (or the following change event) and, when the postimage is already gone, drop the entry and push redo.

## What round 1 got right

Light cells skip a `when` that names an under-dark axis (`app.ts:229`), matching `select(..., base: true)`, and a tie is an error (`app.ts:396`). “Only this mode” names every axis, and a light column with an under-dark axis offers none (`app.ts:232`), which is the disposition that was chosen. A typed value survives the button in either order, because `change` runs before the press and `scopeOwn` keeps it. Apply writes the proposal on screen: the review rides on that value (`app.ts:417`), Apply requires `proposal.key == draftKey` (`app.contract:290`), and the whole digest map is sent (`app.ts:427`). Undo and redo peek; undo is set aside only on `conflict`, and any other failure keeps the entry. A null inverse leaves redo alone (`app.ts:431`). Dial inputs call `showDial`. The branch panel is shown only for the selected cell (`app.contract:405`). Refusals include each diagnostic (`app.ts:23`). The chip stays “Rebuilds” until `class === "live"` and hello says `live: true` (`app.ts:417`). The proxy test awaits `before()` before forwarding, compares one hunk and then the whole file, edits the unformatted `text.primary` line, and checks the Dim `when`, the full-tuple `when`, redo, and a refused pin (`tamber-dial-inert`, then Cancel). None of that covers a server swap or `text.system`.

Verdict: request changes — redo can still write this session’s edit into another app on port 7740.
