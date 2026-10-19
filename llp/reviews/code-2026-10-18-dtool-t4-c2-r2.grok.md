# Code review: LLP 1122 T4 step 2, the state rung, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `025317cbf9`.
- **Method:** one brief (sha256 `63a9ad47f085842a8c6c3d1613e21e1f9cab8611df6b3795617b7843f3c43766`), shared by both reviewers, blind to the other review; commits 0c44ecbba5 ab3c809bed. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 1 medium, 1 low).
- **Disposition:** taken in `5d876785e2` (the text: found by the gate preview before this round reported it, read from the span's start; the test's span now names the attribute only) and `d736c267b4`: the state editor starts from the value as written or empty, never the host's resolved paint; a state draft is bound to the copy and the files as they were when it was opened, so any write since, or a read-only copy, leaves it stale until opened again (a fresh report does not revive it); the held node's part files count toward `readOnly`; a package is a file the server does not count among the app's own Contract sources (`ownContracts`, from `tamber_contract.rs`'s rules), vendored and linked ones included. Not taken: `:hover`, `:focus-visible`, `:disabled` and `:checked` parts carry no place in the map (the compiler's; the recorded limit, in the protocol doc and §10); the Apply → rebuild → replay line is the T4 gate's, which found the replay bug fixed in `d058feff01`.

This still blocks. The state rung is offered from a real map, and Apply on it writes the host's resolved paint back into the style.

## Correctness

**[P1] The state field still starts from the resolved paint — `scripts/tamber-inspect.mjs:205`**

`stateRung` treats the map span as `name=value` and keeps only a regex match inside it:

```204:207:scripts/tamber-inspect.mjs
  const line = lines[b.at.line - 1] ?? '';
  const attr = line.slice(Math.max(0, b.at.col - 1), b.at.end_col ? b.at.end_col - 1 : undefined);
  const text = /=\s*"([^"]*)"\s*$/.exec(attr)?.[1] ?? /=\s*(\S+)\s*$/.exec(attr)?.[1] ?? null;
```

The compiler's span is the attribute name alone. `literal_line` stores the ident's span (`contract/syntax/src/parser.rs:654`), and `:active` lowering keeps that span (`contract/lower/src/states.rs:526`). On

```
style PostButton
  :active
    background-color = "accent.fill"
```

the slice is `background-color`. There is no `=`, so the rung is returned with no `text`. The style name from the `style` line above is right; the authored value is dropped.

The editor then substitutes the host value. `stateOf` turns a missing `text` into `""` (`apps/design-tool/inspector.ts:54`), and both buttons do `stateText != "" ? stateText : r.value` (`apps/design-tool/app.contract:1233` and `:1258`). Click "Edit PostButton :active background-color" and Apply without typing. The write is `background-color="#0f73ffff"` (or whatever paint the host reported). The token button beside it is intact; the bordered default is the one that replaces the token and drops other modes. A literal `#0000ff` is the same path: the web host reports `rgba(0,0,255,1)`.

A name-only span on that source returns a rung whose `text` is absent. The join test never sees it: `at()` sets `end_col` to the whole `background-color="primary_600"` string (`scripts/tamber-inspect.test.mjs:178`), and the contract test posts an `editable` that already contains `text`.

**[P2] A held node's style file is not in the moved-file check — `scripts/tamber-inspect.mjs:261`**

`readOnly` is computed from the selected node's file, its call-site chain, and its own bindings (`:261`). The held offer then reads the held node's binding file (`:273` → `stateRung`). Styles are program-wide (`contract/lower/src/class.rs`), so that file need not be either of those.

Select a label in `post.contract` inside a pressable whose `style Press` lives in `ui/button.contract`. Once the build sends `held.site` for that pressable, insert a line at the top of `ui/button.contract` before the plan reloads. The label's files are unchanged, so `readOnly` stays empty. `stateRung` reads the new bytes at the old line and can offer `set-state` for whichever `style` is now above it.

## Smaller

**[P3] A vendored package is still offered — `scripts/tamber-inspect.mjs:197`**

Outside the app, and anything under `node_modules`, now returns a reason. The server also refuses a directory under the app whose `package.json` has a `name` (`contract/cli/src/tamber_contract.rs:67`). `packages/reading` is that shape. A `:active` style there still gets a rung, and Apply comes back "`…` is no Contract source of the app's own".

## Round 1, and the fix holds

- Held rows are joined at `held.site` through `editable`, so the token sits beside the state rung when the row carries one. With no site, the rows are shown and not offered. That matches stage 9 E not having landed.
- The style name is the `style` line above the row, so a compiler rename (`Card__post` in the map, `style Card` in the file) addresses `set-state` correctly.
- A package path outside the app, or under `node_modules`, is a reason.
- The draft is bound to `inspected.key` and Apply is disabled while that copy is stale (`app.contract:680`, `:1507`). Opening the state editor clears `selToken`.
- `1e-1` is sent as a number (`app.ts:237`).
- The hold sentence says the hold returns where the node still corresponds (`app.contract:1506`). Hover, focus-visible, disabled, and checked stay without a rung, and the protocol says so (`docs/tamber-protocol.md:1022`). That is the recorded limit, and the compiler still emits `state` and `at` only for `:active`.
- The state button is the bordered default; "Edit the token … instead" is the other one. That matches the 16:18 correction.

`~/x2voice/SIGNOFFS.md`, `to-dtool2.md`, and `x2p4-s9e-plan.md` are not on this machine. The ruling above is the one in the brief and in the round-1 disposition. I did not run the suites. The name-span case was checked by calling `stateRung` with a read of the source in memory.
