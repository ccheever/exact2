# Code review: LLP 1121 stage 11 A2 part 1, the server and contract tamber fold packages, round 2 of 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `ebd1a39072`.
- **Method:** one brief (sha256 `615c1c3e84b25de4b784a038dcdfc458085419a910d4c970111b295864cdc526`), blind to the other review; the commit as amended after the previous round. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no BLOCKING; one residual, the same as Astra's BLOCKING).
- **Disposition:** taken in 6b7feeb93e: a write's planned app design folded through fold_with (an override removed that hid a package token naming the app's is refused, tested), an inline-theme app with packages told to use contract build, tamber gen for a packages-only app, the bands' limit.

No BLOCKING findings.

**What checks out**

1. **Read paths agree with the build, and apps with no package designs keep the old fold.** `design_with` goes through `fold_with`: pre-fold refusals (`join_package_designs`), the joined design, then `package_references`, with diagnostics relocated. An empty package list leaves the design unchanged, `joined` empty, and `Params::is_empty` equal to the old `list.is_empty()` check, so check, plans, and the role-axis fold behave as before. Proposals use that same plan path. History compare and revert only parse structure. Export's `design()` still checks the app file alone; that DTCG gap is the queued item. `add_to` starts from an empty `joined`, so package tokens already merged into the design are kept once, in app order then package order.

2. **Relocate.** The message is unbanded first, and only a number in `LINE .. params::LINE` (`1<<28 .. 1<<29`) is rewritten, so a param line is left alone. The band is the package's index, including a package skipped for a pre-fold refusal. The file is `app.files[source_id]` (set when that path is pushed), with `{package}/{file}` only if that slot is missing.

3. **Writes stay on the app file.** `parse_ops` only reads the op JSON. `set-token`, `remove-token`, and `unset-token-when` resolve the name with `token_index` on the parsed app design (`tamber/src/edit.rs:1026-1043`). A package-only name is `no token \`…\`` at line 1, and the package file is untouched. `add-token` treats "already declared" as the app file's tokens only (`edit.rs:700-705`) and appends the new line to the app theme block; the following laid fold then drops the package token of that name, so the new line is the token. `rename-to` a package token name changes the folded value and the rename proof refuses it. The package path is not in the snapshot, so an op `file` pointing at it is rejected. Digests, the snapshot, and hello stay the app design, sibling `.tamber` files, and the app's `.contract` files. The params cache key includes the walk's `.tamber` digests, so a package edit reloads `AppParams`.

4. **Admission and gen.** With any package design, including a packages-only app (synthetic `tamber app`), check, eval, and ir admit through `compile_path` on the real app. Gen still compiles that app first, so a pre-fold or reference refusal fails gen, then emits the joined tokens. `admitted` sees those names, so `reading.card` and `reading-card` still collide as `readingCard`. An app override is one binding. Gen of an app with no design file still fails, as it does for params alone.

5. **Dev watch.** Unchanged, and true: package design files are in the source graph, written into `.gen/dev-sources.json`, and `watchPackages` / `contractSources` watch them even though the app watcher skips `node_modules`.

6. **Tests.** The six new tests cover an alias resolve and describe, check's `reading.tamber:5`, `set-token` on `reading.card` refused with the package file unchanged, a package token naming an app token, a packages-only check, and an unbanded `(line 4)` with the band number absent.

Round 1 is in the tree: `Params.joined` and `is_empty`, `fold_with`'s pre-fold and reference refusals, message unbanding, packages-only check, and the export note in `QUEUE.md`.

**Residual, not graded.** The plan's final check does not re-run `package_references`. Removing an app override that was hiding a package token the crate accepts and the reference rule rejects still writes, and the next build fails — the same outcome as when the server folded the app file alone. A scrub keeps the `AppParams` from open, and the package file is not in the client digests, so a package edit mid-gesture can make the fresh role check disagree with the frame. The new tests do not cover `add-token`, gen collisions, scrubs, export, or a profile colour.
