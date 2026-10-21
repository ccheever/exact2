# Code review: the Tamber server writes a library component's param by its full name (LLP 1139 D8), round 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `828e32f657`.
- **Method:** one brief per round (sha256 `26cdeb279ea9452fa440fced4fee9d0c8cafb60acc2b9303fa75606a38d52935`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** SHOULD-FIX: repair a line already spelled short when a write touches it, and an unset+set in one transaction
- **Disposition:** taken: `respell` on set-param, the line renamed in place; an earlier op's spelling preferred

**SHOULD-FIX — repair existing short library entries when rewriting them.** [contract/cli/src/tamber_ops.rs:187](/tmp/iosfix-rv-pf1/contract/cli/src/tamber_ops.rs:187)

`line.or(earlier)` overrides the new full-name fallback. Starting with `Card.inset = 16px`, even `set-param` using `@acme/lib/index.contract#Card.inset` retains the short spelling. An `unset-param` followed by `set-param` in one transaction also recreates it: normalization reads the original snapshot for both operations. The write checks accept this, while compilation still rejects it with `tamber-param-short-library`.

This belongs in this commit because these are precisely the broken files the original bug produced. Canonicalize a touched library entry in place, preserving its `when` branches and comments, and test both direct rewriting and unset/recreate. Simply changing the operation’s name to `p.full` would leave the old entry behind.

No additional findings:

- **Membership matches for resolved params.** `AppParams.library` includes every loaded component whose identity is in `file.library`, even without parts; `Described.library` uses that same membership test. Re-exports retain their declaring identity. Design-declared components are outside both sets.
- **Other paths:** new base/`when` writes and earlier operations use the full spelling; scrub frames, resting verification and release share normalization. `rename-token` preserves parameter names. Set-row/add-param/promote edit Contract declarations; import acceptance uses the transaction path. These introduce no separate bypass from a valid design.
- **Keep advertising `short`.** Accepting an unambiguous short input and persisting the full library identity is a sound contract.
- **The new test is sound for new entries.** [tamber_package_params.rs:787](/tmp/iosfix-rv-pf1/contract/cli/tests/it/tamber_package_params.rs:787) checks a successful write, full spelling and compilation; without the fix, the spelling assertion fails. Diff equality is meaningful because the successful transaction supplies an actual diff, preventing a vacuous `null == null` pass. It lacks coverage for the repair above.

Static review only; no files modified or tests run.
