# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 19 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `27ed7b24b4`.
- **Method:** one brief (sha256 `44b36caec32e085de47cf9c017a3ab217add64e76c034df61b9916e5b5747521`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

I'll review HEAD read-only and judge only whether a design-server write can still land on a package file under the theme2 definition.The commit is the package-design wire change. I'll read the guard and every write path that is supposed to call it.BLOCKING: a contract-only transaction writes `.exact`'s journal and temporary files inside a package directory. `read_only` never runs, so neither (a) nor (c) is applied.

`read_only` is called only from the design-op loop in `ops()` (`contract/cli/src/tamber_packages.rs`). A transaction whose ops are only `set-state`, `unset-state`, or `set-row` is split into contract ops (`tamber_serve.rs` `transaction`); the design-op list is empty, the loop does not run, and `.exact` is not judged. `commit` still fsyncs `tamber-*.tmp` and `tamber.journal` under `.exact`.

That directory is a package file by the 21:32 definition when `.exact/package.json` names a package (a), or when the load resolved `.exact` as a package root (c). The same bypass applies to accepting a held revert whose ops are only those contract ops (`tamber_history.rs`).

Reproduction:

- App with the usual `app.tamber` and an `app.contract` that has `style Post` and a component using it (as in the set-state test).
- `.exact/package.json` is `{"name":"p"}`. The server starts.
- Transaction: `{"method":"transaction","client":"one","files":{"app.contract":"<digest>"},"ops":[{"op":"set-state","style":"Post","state":"active","row":"background-color","value":"accent.pressed"}]}`.
- The write succeeds. `.exact/tamber-<session>-<n>.tmp` and `.exact/tamber.journal` are created under the directory whose `package.json` names `p`.

The same layout as `nothing_is_written_when_the_servers_own_state_is_a_resolved_packages_root` (`.exact/package.json` is `{}`, `node_modules/p` links at that manifest and `index.contract`, `app.contract` has `use P from "p"`) with this `set-state` instead of `set-token` writes those same files under the resolved root. A `set-token` is refused; this op is not. Undo of the result does call `read_only`, so the inverse is refused after the package files have already been written.
