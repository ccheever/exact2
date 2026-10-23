# Code review: stage 13 B, the pen importer, round 3's re-check 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `198b26b2aa`.
- **Method:** one brief (sha256 `095b02dee7f5f53f798a84142e8824a0544ca2822201d5c122dcd33783bc68d8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [dtcg.rs:1155](tamber/src/import/dtcg.rs:1155): **Prefix amplification remains uncharged.** A request under 400 KiB containing a 256 KiB prefix and 2,500 colour variables would retain about **1,250 MiB** across `names` and `taken`. The new charge counts only source paths—25,000 units. The watchdog, rather than the import budget, must stop it.
- [pen.rs:260](tamber/src/import/dtcg/pen.rs:260): **The theme skip also skips descendant IDs.** `ref.descendants["theme"]` contains property overrides, not theme values. With `n=0.5`, `gap:"$n"` and that descendant’s `opacity:"$n"` incorrectly produce `0.5px`. Restrict the skip to actual theme properties. [Pencil schema](https://docs.pencil.dev/for-developers/the-pen-format#typescript-schema)
- [pen.rs:226](tamber/src/import/dtcg/pen.rs:226): **`position` is accepted outside gradient stops.** Adding `position:"$n"` elsewhere beside `gap:"$n"` changes a length into a number, although that position binds nothing. The prior review explicitly required context qualification. [Pencil schema](https://docs.pencil.dev/for-developers/the-pen-format#typescript-schema)

**Should-fix**

- [import_pen.rs:613](tamber/tests/it/import_pen.rs:613): The collision regression’s `!0?`, `!1?`, etc. normalize to distinct `s0`, `s1` names, never colliding with `x`. Use distinct punctuation-only names.

**Nits:** None.

The original unused-name collision path is fixed by inspection. The permitted test command failed before execution because the sandbox denied Cargo’s temporary-file creation. No files changed.
