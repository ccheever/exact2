# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 13 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `17f78e2dac`.
- **Method:** one brief (sha256 `813efcc2fa168ab853c19f5352cb0c82a051084fe39f2c77673ff3b84b185346`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — two in-scope write paths.**

Both reproductions use:

- `app/app.json`: `{"tamber":"vendor/p/design.tamber"}`
- `app/vendor/p/design.tamber`: `tamber t\n\ntheme\n  brand = #123456\n`

Start `contract tamber serve --stdio app`, send `hello`, then send this transaction with its returned `ok.files`:

```json
{"id":2,"method":"transaction","client":"review","files":HELLO_FILES,"ops":[{"op":"set-token","name":"brand","value":"#654321"}]}
```

1. **Unreadable named manifest bypasses (a).** [tamber_packages.rs:221](contract/cli/src/tamber_packages.rs:221)

   Give `vendor/p/package.json` contents `{"name":"p"}` and mode `000`; run as a non-root user. Keep its directories readable/writable and use a minimal `app.contract`:
   `component App\n  view\n    text "x"\n`.

   The directory walk succeeds, but reading the manifest returns `PermissionDenied`. `.ok()` discards that error, classifies the target as unprotected, and the transaction replaces the package’s design. The named manifest makes this **(a)** regardless of whether the app imports it.

2. **Resolved roots without package designs bypass (c).** [tamber_packages.rs:148](contract/cli/src/tamber_packages.rs:148)

   Use readable `vendor/p/package.json` containing `{}`, plus `vendor/p/index.contract` containing `component P\n  view\n    text "x"\n`. Create a regular `node_modules/p` directory containing only these links:

   ```text
   package.json   -> ../../vendor/p/package.json
   index.contract -> ../../vendor/p/index.contract
   ```

   Set `app.contract` to `use P from "p"\ncomponent App\n  view\n    P()\n`.

   The resolver records real `vendor/p` as a package root. The guard checks the individual symlink targets, ancestor manifest names, and package-design identities; it never checks that resolved root. Because the manifest lacks `name`, no package design enters `app.packages`. The transaction therefore replaces `vendor/p/design.tamber`, a package file under **(c)**.

These are code-traced reproductions; I did not execute writes or modify files.
