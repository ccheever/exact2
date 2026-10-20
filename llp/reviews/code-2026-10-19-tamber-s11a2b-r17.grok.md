# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 14 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `8a9ccf942b`.
- **Method:** one brief (sha256 `aa2ad6ae21b96860bcfa58562675135ef4bf2eef936aa7144a93fca2aefd796f`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

I'll review HEAD read-only against the theme2 package-file definition: whether any design-server op can still write a package file after the in-definition fix. I'll start with the review skill and `git show HEAD`.HEAD is the package-designs commit. Next I’ll read the guard and every design-server write path against theme2’s real-path definition.The guard is in `read_only` and `in_package`. I’ll trace every design-server write—transaction, proposal accept, scrub release, undo, revert/import—to see whether any can still land on a package file.BLOCKING: a contract load that resolves a package and then fails leaves `package_roots` empty, so a transaction writes a file under that resolved root.

`link` records the package before later uses run (`contract/cli/src/sources.rs:765-773`). A following missing import or any syntax error makes `sources::load` return `Err`, and `params_of` then keeps the default params, with no roots (`contract/cli/src/tamber_params.rs:72-75`). `source_graph` still holds that root (`contract/cli/src/sources.rs:654-656`). `read_only` only consults `package_roots` for (c) (`contract/cli/src/tamber_packages.rs:145-153`).

That root is not caught by (a) or (b) when `package.json` is `{}` and the real path has no `node_modules` segment. Per-file links are not directory links, so the symlink walk does not see the design file. `in_package` returns `Ok(None)` (`contract/cli/src/tamber_packages.rs:228-249`). Proposal accept, scrub release, undo, and a restoring revert or import all commit through this same guard.

Reproduction, the existing resolved-root layout plus a load failure after `p` is resolved:

- `app.json`: `{"tamber": "vendor/p/design.tamber"}`
- `vendor/p/package.json`: `{}`
- `vendor/p/index.contract`: a component file
- `vendor/p/design.tamber`: `tamber t` / `theme` / `brand = #123456`
- `node_modules/p/package.json` and `node_modules/p/index.contract`: symlinks to those two files under `vendor/p` (the directory `node_modules/p` itself is real)
- `app.contract`:

```
use P from "p"
use Gone from "./gone.contract"
component App
  view
    P()
```

`use P from "p"` resolves root `vendor/p`. `./gone.contract` then fails the load. Real path of the design is `<app>/vendor/p/design.tamber`: no `name` in `package.json`, no `node_modules` segment, and it sits under the root just resolved. Hello, then:

```json
{"method":"transaction","client":"t","files":"<hello files>","ops":[{"op":"set-token","name":"brand","value":"#654321"}]}
```

The transaction is accepted and `vendor/p/design.tamber` becomes `#654321`. The same layout with a clean load is the case the new test refuses.
