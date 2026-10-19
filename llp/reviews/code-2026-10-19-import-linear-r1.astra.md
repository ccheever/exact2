# Code review: Tamber importers' super-linear paths, the exact part, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c10631c116`.
- **Method:** one brief (sha256 `a6640f04ec2d4040839df9a7bb1860a55d89ee8a09a14566a95c7a8131894acc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (the type memo cloned a long type into every alias's entry: GiB from a small input), one nit (stepped's claim needs no Restore)
- **Disposition:** both taken in the next commit: the memo keeps the typed definition's index; the claim says no Restore

I found **one blocking regression** in HEAD `c10631c1164f`.

1. **Blocking — the type memo can retain gigabytes from a small input.** [kinds.rs:54](/tmp/rv-implin-1/tamber/src/import/dtcg/kinds.rs:54) clones the entire resolved `$type` into every visited path’s cache entry. Type strings have no length bound. One token with a 512 KiB unknown type and 4,096 direct aliases fits below the request’s 1 MiB limit but retains roughly **2 GiB** of type strings. The old two-hop walks kept those copies transient. These allocations are uncharged, exceed the [worker’s memory allowance](/tmp/rv-implin-1/scripts/tamber-isolate.mjs:16), and can replace the ordinary unmapped report with a failed import. Cache the terminal definition index or a shared/borrowed type string; adding a new refusal would also violate this commit’s exact-output requirement. This is a source-derived counterexample, not an executed probe.

2. **Nit — qualify the composition claim with “no `Restore` operations.”** [edit.rs:465](/tmp/rv-implin-1/tamber/src/edit.rs:465) claims chunked stepping equals `apply_all` generally. If the first group fails with “no dial” and a later group contains `Restore`, whole-list `apply_all` returns the Restore refusal first; chunked stepping returns the earlier error. Current re-import generates only `SetDial`/`SetToken`, so this does **not** change its behavior. Narrow the documentation.

Otherwise, the equivalence checks hold by inspection:

- Type answers match for untyped cycles, missing targets, alias errors, typed midpoints, and reused suffixes. Caches are per tuple. Nothing inside the memo borrow calls `raw`, `leaf`, or `type_of` recursively.
- With the generated operations and `params::NONE`, stepping and checking preserve diagnostic order. A cached prefix error remains the same refusal; rejected groups update neither `made` nor `ops`; empty groups retain the old counting behavior.
- Name indexes preserve last-wins values, first-wins overrides/declarations, exact-before-ASCII-insensitive lookup, and the old `zip` length.
- Both `applied` replacements preserve checking before the unchanged-text branch.

The timing bounds have about **20× and 8.3×** headroom against the supplied measurements. They are plausible coarse regression guards, but wall-clock assertions remain load-sensitive; I cannot establish flakiness from inspection. All changed source files meet the cap; the largest is `export.rs` at **1,491 lines**.

No files were edited, and no builds, tests, or probes were run.
