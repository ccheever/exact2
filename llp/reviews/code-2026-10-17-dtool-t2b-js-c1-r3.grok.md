# Code review: LLP 1122 T2b, the JS target, step 1 (the page's plan digest and its map), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `24efee359`.
- **Method:** one brief (sha256 `2a6bb3c4e28331a659cc2bada1335cb58b99169f9cb2580b852e5a62568a0c22`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken (the last round). A reload certifies its design only when the shape held: then the natives on the resident producer take it as a batch. A shape change (an axis, a role, or a reordered value table with the same digest) leaves `designServed` null, so a build left on its old plan stays read-only. The T2 test's poll beside the drive is stopped and awaited in `finally`, before the tool goes.

---

I'll review this commit read-only: the JS plan-digest path, prior round notes, and whether every served build keeps the map the page actually runs.The commit is the JS page’s plan digest and the loop’s map. I’ll read the diff, the earlier review rounds, and the build/server code those notes depend on.Round 2 left one `designServed` case. I’ll read the current loop against the wasm meaning and the map contract.The round-2 `designServed` change is only half right. A shape-breaking publish still certifies a design that natives never receive, so a build left on the old plan can edit a file it has not taken.

## Medium

`host/web-js/dev.mjs:376`

`served` is set from the new build before `shaped` is known. The batch and the snapshot are sent only when the shape held (`dev.mjs:382-386`). `design::digest` is token name, value, and slices (`runner/src/design.rs:136-148`). `design::shape` is axes, roles, and slice counts (`runner/src/design.rs:66-83`). Those can diverge.

Add a mode axis, or change a token's role, without changing any token value. The design digest stays the same, the file hash changes, and `structure()` changes, so the build is not kept and the page reloads. `shaped` is false, so no batch is broadcast and `snapshot` is null. A Mac on the resident producer keeps its plan and its old digest. `designMoved` (`scripts/tamber-inspect.mjs:98`) then sees that digest equal `served.digest` and the new file equal `served.source`, and offers the token rung. That build has not taken the new axis or role. The wasm loop clears its live certificate on a plan publish (`host/web/dev.mjs:547`), so the same selection stays read-only.

Set `served` only after `shaped` is computed, and only when it is true: `{ digest: next.digest ?? null, source: built.designSource }`. When the shape did not hold, set `served` to null. The reloaded JS page stays editable through the new plan's map, which records the file just compiled. An accent-value change still certifies, because that keeps the shape, sends the batch, and is the case round 2 fixed.

The rest of this step holds. `exact.plan` is the SHA-256 of the same `app.plan` bytes `map.json` names (`host/web-js/src/main.rs:319-328`, `contract/cli/src/map.rs:19-20`), and the dev build embeds that hash and copies both files (`host/web-js/build.mjs:172`, `400`, `692-696`) with no `--plan` and no `--production`. `keepPageMap` runs for the first build whether it failed, for a publish, and for a publish that lands while a later edit was requested; a kept or value-only edit does not replace the plan, and a scrub does not write one. With no digest, `sourceMap` still returns only the resident plan. The page map is read from `dist` after the compiler, and a resident map still has to match its card. The new test reads `planDigest` while the page is open and checks it against `dist/app.plan` and the on-disk map.

Verdict: request changes — one Medium, a shape-breaking publish certifies a design the native builds did not take.

