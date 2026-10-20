# Code review: stage 4 commits 3 and 4: the live table, round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree, read-only by instruction.
- **Method:** one brief (sha256 `230ec6b094bdf6799c769e0b4c8300a3ee18f6ecb7f17535381f1745d0114afa`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (snapshots refused with a role override; snapshot fetches racing; accent companion rules; partial accepts dropping guards).
- **Disposition:** fixed in the first-round commit.

---

I'll review the HEAD commit as a read-only defect pass against the live-table design, starting with the review criteria and the commit itself.The commit touches the live-table path end to end. I'll read the design, then the structure decision, apply path, and hosts.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I’ll confirm the live-table paths against the commit and then report findings by severity.The role-override snapshot refusal is blocking. A plan that keeps a role override (Canvas and the others) cannot recover onto the current design, and on Linux it never applies a live edit at all.

## Findings

[P1] A snapshot that includes a role override is refused whole, so live recovery deadlocks — `runner/src/runner/theme.rs:43`

`design::tokens` (`runner/src/design.rs:58`) emits every theme row, and `snapshot` sends that list with no base. `set_tokens` returns before `edit_token` if any named row has a role, and `apply_live` (`runner/src/runner/live.rs:81`) leaves `self.design` unchanged on that refusal. A value-only colour batch still applies, because it carries only the tokens that changed and a role value stays in the structure. The recovery snapshot always carries the unchanged role row too. The build answers `refused`, stays on the old digest, and the next batch is stale. Linux only polls `/__dev/design` (`host/linux/src/fetch.rs:152`, `host/linux/src/presenter/delivery.rs:44`), so an app with a role override never takes a live edit there. Web and Apple keep applying incremental batches until a connect notice, a missed frame, or an oversized-batch notice asks for that snapshot, and then they stick the same way.

[P1] Overlapping snapshot fetches apply in completion order and can roll the design back — `host/apple/Sources/ExactKit/PlanURL.swift:855`

`design` starts `syncDesign` on every stale answer and does not cancel an in-flight one. `beginResolution` (`PlanURL.swift:412`) cancels fetches only when a generation starts, and a value-only edit does not start one. Each `BoundedFetch` has its own session, and the callback applies any HTTP 200 snapshot. A snapshot has no base, so `apply_live` takes it whenever the digest differs. A connect notice and the next edit, or two stale frames, can both be in flight. If the newer body returns first, the slower older body writes the previous colours and digest afterward. The next server batch is then stale against that rolled-back digest, and nothing polls. The JS-target page has the same interleaving: `live` in `host/web-js/dev.mjs:205` fetches `/__dev/design` without waiting for the previous call. The wasm dev page chains frames on one promise (`host/web/dev.js:263`), so this race is the Apple session and the JS-target page.

[P2] A live edit leaves an accent companion rule on the colour baked at build — `host/web-js/modes.js:195`

`rules` writes a second rule, `{selector}[data-x-accent="{prop}"]{--exact-accent:{colour}}`, when a token names the accent and its own value is a colour (`host/web-js/src/modes.rs:415`). `exact.design` only `setProperty`s the token property on the tuple rule. `tupleRules` skips selectors that contain `data-x-accent`, so the companion stays at the build-time colour. The development test bakes `[data-x-modes="ink=plain"][data-x-accent="--x-accent-mix"]{--exact-accent:#000000ff}`. After a live edit of `accent.mix`, elements that pin that accent keep the old colour, and `Design` still advances.

[P3] A partial accept plans the replacement without the proposal's requires — `scripts/tamber-server.mjs:525`

A full accept writes `requires: p.guards`, and the design server rejects a missing or mismatched digest (`contract/cli/src/tamber_serve.rs:788`). A partial `items` list, and a non-revert conflict, call `hold` with no `requires` argument. The replacement is stored with empty guards, and the accept that actually writes it does not check the paths the original proposal guarded. A revert replacement still spreads `p.guards` into `files`, so this is the ordinary partial path.

## Assessment

The value-only decision holds on the paths checked. `Plan::structure` keeps role values, profile colours (`"(--"`), slice ranges, axes, code, and data, and drops a string from the pool only when no other string field still uses it. The builder interns design values last. `design::batch` returns nothing when token names differ, which covers a token or synthetic added or removed. The wasm loop also requires unchanged contract text and an unchanged surfaces stamp, and only `.tamber` paths in the change set. The TypeScript producer requires identical module bytes. The JS loop requires the same structure, the same token names, and an unchanged logic revision. A normal colour edit, including one tuple of a varying token, stays value-only.

Digest, stale, and notice handling match the live-table section for a plan with no role row. An empty notice does not move `self.design`. A release `compat` whose trust is `production` refuses. A new runner boots with `self.design` unset, and `set_role_overrides` clears that plan's edits and the theme-table cache. `set_tokens` checks every name, then edits every token, then walks rows, so a row that reads several tokens sees the whole batch. A bloom-bit collision sets an extra row again from the updated table. Wide-table refusal reports `full` and still advances the digest, which is what this stage specifies.

On the JS target, `every(dev_reload)` restores the previous flag when the guard drops, and a release emit passes `false`. The development test checks that release CSS has no `--x-brand` and that `dev-design.json` carries `--x-brand` and the second `surface.card` tuple as `light-dark(#ffffffff, #15202bff)`. Tuple keys are mixed radix, first axis most significant, on both sides. `modes.js` does not read `structure`; a structure change is a reload that clears the snapshot.

The Swift signatures match the C header and the Apple export: `exact_design_rows(rt, len)` against the wasm export `exact_design_rows(len)`. URLSession callbacks are on the main queue and enter the runtime through `Bridge.on`. Each snapshot fetch uses a 10s request timeout and a 30s resource timeout. The event stream uses a 24-hour resource timeout. This review did not compile the Swift.

Writes call `ask` with `wait: 0`, so a transaction, undo, or accept stays pending until the design server answers and does not 504. Reads still use `answerMs`. `hello` includes `live`.

The batch test (`contract/cli/tests/it/tamber.rs:1051`) and the Linux live test (`host/linux/tests/it/live.rs`) apply snapshots only for plans with no role row. The structure test shows a Canvas change changes the structure and never applies a snapshot that contains that row. The server test checks a stale `requires` on propose (`scripts/tamber-server.test.mjs:411`) and does not accept a guarded proposal after the required file moves.
