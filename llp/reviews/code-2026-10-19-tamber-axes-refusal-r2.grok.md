# Code review: Tamber axes bound before the fold (the JS loop's when-token edit), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f8ad6c5658`.
- **Method:** one brief (sha256 `90b12ae349c6f68c655a290da8501ddf9fa7c2e1e0c2d38af8b7252f028f35c2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking
- **Disposition:** its note that the test checks presence, not order, is Astra's P3, taken in f8e66c6187

Nothing is blocking. The non-fatal `setMode` refusal keeps the design's own error, and the new check does not turn a build that already succeeded into a failure.

The refused command is compiled and emitted on the same path as any other command. `set_mode_args` only returns an error for two arguments whose axis is a string literal, and whose value, when it is also a string, is not one of that axis's values. Those literals go through `command_args` and `compile_or_none` as ordinary strings, then `asm.command` records `Opcode::Command` with the same operand layout a valid `setMode` uses. That recording does not add another diagnostic. Action bodies are lowered before the first `l.errors.is_empty()` check, so the pushed `tamber-mode` is already in the vec that check returns. `finish()` does not run, and the command is not part of a plan.

`l.errors` holds refusals only. Warnings are a separate channel: `Folding.warnings`, and the `warned` list `lower_all` fills from `warnings::with_text_styles`. Nothing between the two checks appends to `l.errors`. Child-state init and `bake_texts` return `Err` through `?`, and they do that only after the first check has already seen an empty vec. A build that used to reach `finish()` still reaches it.

The second check does not truncate. It is unreachable while that gap stays empty, and `compile_path_all` still keeps twenty diagnostics. The first check is the one that truncates to `MAX_REFUSALS`.

For `compile_path`, the design refusal is first. `declare_theme` pushes the fold's diagnostics, sorted by line, and returns before actions are lowered. On the duplicate-axis fixture the second `dark-style` is the earliest diagnostic, so `swap_remove(0)` is `tamber-duplicate`. `setMode("grey")` is pushed later, against the pre-fold axis list, whose first `dark-style` is `black | dim`.

The contract test covers the reported case: `compile_path_all` contains both `tamber-duplicate` and `tamber-mode`, and the under-light edit still has no `tamber-mode`. It does not assert that the design id is the vec's first element, which is what `compile_path` shows. The JS test waits until the log contains both `build failed` and `[tamber-`, which is how `CompileError` is printed (`{span} [{id}] {message}`), and then checks `tamber-under-light`.
