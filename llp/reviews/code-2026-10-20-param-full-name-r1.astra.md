# Code review: the Tamber server writes a library component's param by its full name (LLP 1139 D8), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `828e32f657`.
- **Method:** one brief per round (sha256 `cd95eb2371446aa4a8ec08e859a1fb2b8c64b28ff449dcf3bfe43a3789054813`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no findings; noted that an existing short line keeps its spelling
- **Disposition:** taken in round 2's brief as a question

No findings in HEAD.

- **Library membership matches for every resolved param.** [params_of](/tmp/iosfix-rv-pf1/contract/cli/src/tamber_params.rs:71) includes every loaded component in `file.library`; `Described.library` uses that same membership. Re-exports retain their declaring-file identities. Design-declared components are outside this set in both description and compilation.
- **Accepting short input and persisting full names is the right contract.** `describe.short` remains useful. New base lines, `when` operations, subsequent operations within a transaction, scrub frames, verification and release all pass through the normalization. Rename changes param values, not keys; imports generate no param operations; set-row/add-param/promote edit Contract declarations and undergo compilation.
- **The regression test is sound by inspection.** Without the fix, its [full-name assertion](/tmp/iosfix-rv-pf1/contract/cli/tests/it/tamber_package_params.rs:790) fails, and the resulting design fails compilation. Diff equality is meaningful: the preceding successful-write assertion and planner guarantee a nonempty transaction diff, so this cannot pass as `null == null`.

One explicitly preserved limitation: [existing-line precedence](/tmp/iosfix-rv-pf1/contract/cli/src/tamber_ops.rs:187) retains an already broken short line—even when one transaction unsets and recreates it. Separate unset and set transactions repair it. This predates HEAD and follows the stated spelling-preservation requirement.

Read-only review; no files modified or tests run.
