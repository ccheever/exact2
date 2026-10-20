# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 15 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `a46bf4455a`.
- **Method:** one brief (sha256 `27482f1edc3ae24531a84337f113f96762c5b93a33ff38d05681f684992fdd97`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING.** An unreadable `app.contract` skips the fail-closed package check, so a transaction writes a file under a package root the load would have resolved.

`params_of` treats every failure to read the contract as “no contract”: `failed` stays false and `package_roots` stays empty (`contract/cli/src/tamber_params.rs:41-43`). Only `sources::load` returning `Err` sets `failed` and fills roots (`tamber_params.rs:74-82`). `read_only` then applies “any `package.json` below the app” only when `app.failed` is set (`tamber_packages.rs:145-157`). A contract that exists but cannot be read never reaches that branch. `source_graph` would also refuse it (`lib.rs:421-423`), and the guard never asks.

That is the same case as the test where the load dies before `use P`: `vendor/p/package.json` is `{}`, so clause (a) does not apply, and the real path has no `node_modules` segment, so (b) does not either. The per-file links do not save it. `in_package` canonicalizes `node_modules/p/package.json` to the manifest file, and `vendor/p/design.tamber` does not start with that path. With `failed == false`, the write is allowed.

Reproduction:

1. App design is `vendor/p/design.tamber`. `vendor/p/package.json` is `{}`, with `index.contract` beside it. `node_modules/p/package.json` and `node_modules/p/index.contract` are symlinks to those two files (the layout in `a_resolved_package_root_and_an_unreadable_manifest`).
2. `app.contract` contains `use P from "p"`, then `chmod 000` it.
3. `contract tamber serve` still starts (`design_path` only needs `app.json` and the design file).
4. `transaction` `set-token` on that design. `read_only` returns ok. The commit renames onto `vendor/p/design.tamber`.

`chmod 644` and the same use resolves root `vendor/p`, and (c) refuses the write. Unreadable, the server writes it.
