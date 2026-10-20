# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 13 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `17f78e2dac`.
- **Method:** one brief (sha256 `813efcc2fa168ab853c19f5352cb0c82a051084fe39f2c77673ff3b84b185346`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING.** A transaction can overwrite a file whose real path sits under a package root the app's load resolved.

`in_package` only treats a symlink inside `node_modules` as a package root when the file's real path is under that symlink's target, and it never descends into the link. A named `package.json` is only noticed on an ancestor strictly below the app. A resolved root that is neither of those is still a package file under (c), and the write is not refused.

**Reproduction.** `node_modules` is a real directory. The scope entry is a link, and the package link is one level further, so the walk stops on the scope directory.

```text
app/node_modules/@acme → /outside/scope
/outside/scope/reading → app/vendor/reading

app/vendor/reading/package.json    {"version":"1.0.0"}     # no "name"
app/vendor/reading/index.contract  component Card
                                     view
                                       text "x"
app/vendor/reading/reading.tamber   tamber reading
                                     theme
                                       reading.card = #112233
app/app.contract                   use Card from "@acme/reading"
                                   component App
                                     view
                                       Card()
app/app.json                       {"tamber":"vendor/reading/reading.tamber"}
```

`reading` resolves. `resolve.rs` does not require `name`: the manifest file is enough, so the root is the real path of `app/vendor/reading`. `reading.tamber` is under that root (direct dependency). It is not (a) (no string `name`) and not (b) (the real path has no `node_modules` segment). It is not the queued case: `node_modules` itself is not a link, and the load does resolve this root.

The server starts. That design path is a plain file inside the app. `set-token` of `reading.card` is a transaction (the same path as proposal accept, scrub release, and a revert/import hold):

```json
{"method":"transaction","client":"t","files":{"vendor/reading/reading.tamber":"<digest>"},"ops":[{"op":"set-token","name":"reading.card","value":"#999999"}]}
```

`read_only` canonicalizes the file and calls `in_package`. The walk sees `@acme` → `/outside/scope`, that target is not a prefix of `vendor/reading/reading.tamber`, and it does not list `scope/reading`. The ancestor scan finds no `name`. `package_design` returns nothing without a `name`, so the design-path check misses too. The rename writes `#999999` into `app/vendor/reading/reading.tamber`.

The same hole exists one link deeper: `node_modules/outer` → a directory outside the app, and that package's own `node_modules/reading` → `app/vendor/reading`, with `outer`'s source doing `use "reading"`. That root is resolved transitively, and the walk never enters the outer link.
