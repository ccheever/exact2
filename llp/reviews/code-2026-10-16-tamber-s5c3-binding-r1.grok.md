# Code review: the binding core (Tamber stage 5, commit 3), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `d8523fe6013ce73f9ce060c0ec940e9d4faad9d3c017f44ef1fcdd51c6974255`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the first-round commit.

---

I'll review the binding-core commit read-only: the brief, the design, and the HEAD diff, then check the places you named for wrong answers, input tracking, the ABI, and whether the tests actually cover those claims.The review target is the single HEAD commit, so I'll read that diff and the stage-5 brief against the design, without touching the worktree.The commit is small. I'll read the binding core against how tuples, colours, and the Apple ABI already work, starting with the places the brief says can answer wrong.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is already diagnosed. I'll confirm the two failure paths and the line citations against HEAD, then write the verdict.Nothing in this commit blocks the binding core. The place tuple, a successful colour, and the Apple request shape match a row. Two P2s are worth fixing before this runs on every observer.

## P2 — `inputs` rebuilds the design digest on every call

`Tamber::inputs` (`runner/src/runner/tamber.rs:95`) hashes `runner.design()` every time. `Runner::design` (`runner/src/runner/live.rs:75-79`) clones `self.design`, and while that is `None` — the value set at boot (`runner/src/runner.rs:837`) and kept until the first successful `apply_live` — it calls `design::tokens` then `design::digest` (`runner/src/design.rs:85-112`). That clones every token name, value, and slice, then SHA-256s them (`design.rs:46-50`). `format!("{:?}", preferences.contrast)` allocates on top (`tamber.rs:98`).

A release app never live-edits, so this is the steady state. The host compares `inputs` for every observer after every batch, on the UI thread. A theme of a few hundred tokens pays a full table copy and a SHA-256 per observer per batch. Cache the digest on the runner (the field already exists after a batch) and hash contrast by its discriminant.

## P2 — a token that fails to parse is reported as missing

`color` (`runner/src/runner/tamber.rs:81`) is `set.ok().map(|_| patch.text_color)`. Any `set_plan_style` error becomes `None`, the same answer as a name the plan lacks, and `missing` is not called because `has` was true. The ABI then writes `null` (`host/apple/src/tamber.rs:34-36`), which `exact.h:578-582` defines as "the plan lacks the name", so the binding takes its fallback.

`set_tokens` (`runner/src/runner/theme.rs:59-93`) stores a non-role edit before it parses it, skips a row whose `set_plan_style` fails, and still returns `Ok`. The painted row keeps its last good colour.

Live-edit `ink` from `light-dark(#111,#eee)` to `nope` (or to a wide colour `wide::parse_wide` will not intern, `kernel/src/style/wide.rs:169-172`). The Exact row stays on the old colour. `color("ink", place)` returns `None`, the ABI sends `"ink":null`, and the native binding paints its fallback. There is no journal line.

## P3 — an unknown or retired view is the window tuple

`tuple` (`runner/src/runner/tamber.rs:38-49`) treats `kernel.node` returning `None` as the window. View ids are not reused, so this does not alias a live node. `Place::Node(99999)`, or a destroyed id that had been under a pin, returns the unpinned colours with no error. The same silence is on the ABI: `request["view"].as_u64().unwrap_or(0)` (`host/apple/src/tamber.rs:17-19`) turns a missing view, a non-number, or a string id into `Place::Root`, and a value above `u32::MAX` truncates. Non-string entries in `names` are dropped (`tamber.rs:21-23`) rather than returned as `null`.

## P3 — the tests miss the claims in their comments

`inputs_move_on_every_input_and_only_then` (`contract/cli/tests/it/tamber_binding.rs:87-127`) checks the "holds" half with a second identical call. The live-batch step rewrites the `dim` slice while `place` is the unpinned outer node, so the hash moves when that place's `ink` does not. `setMode`, scheme, contrast, and font size are not checked against `color`.

`a_place_resolves_its_own_tuple` (`tamber_binding.rs:54-70`) says a role override reads as the session's. The assert is the theme hex of `CanvasText`. `bind_roles` as a no-op still passes. The later `#202020` check is the one that goes through a role edit, and it also goes through the theme table.

No test covers nested pins, a scheme-only boundary, a retired view, `exact_tamber` before boot, bad JSON, a non-string name, or a real view id through the export. The two-place test, the state pin, the once-only journal line, and the two-session answers do test what they assert. The Apple test (`host/apple/src/abi_roles_tests.rs:171-191`) checks per-runtime `CanvasText`, `"nonesuch":null`, and a stable second read.

`set_token` (`runner/src/runner/theme.rs:14-20`) writes the live table that `color` reads and does not update `self.design`, which is all `inputs` hashes. `r.set_token("ink", "#ff0000", &[])` changes `color("ink")` and leaves `inputs` equal for the whole session until a successful `apply_live`. The host path is `apply_live`, which sets `self.design` after `set_tokens` returns (`live.rs:117-121`). If `apply` fails after `edit_token`, the live table is ahead of both the kernel and the digest, so the next `color` can show a value the row never committed, and `inputs` does not move for a non-role token.

The tuple encoding matches the bridge (`mode` stored as tuple+1, nearest pin holds the full composed tuple). An empty or refused pin clears the row and the walk continues. A scheme-only boundary does not set `Mode`. On `Err`, the thread tuple is restored before the return. Theme, role, and profile scopes restore on drop. `take_named` clears the names bit. A name the plan lacks is `None` once, and an unbooted runtime returns `{"error":"tamber: not booted"}`.
