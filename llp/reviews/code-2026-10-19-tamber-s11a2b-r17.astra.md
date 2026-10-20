# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 14 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `8a9ccf942b`.
- **Method:** one brief (sha256 `aa2ad6ae21b96860bcfa58562675135ef4bf2eef936aa7144a93fca2aefd796f`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — resolved package roots are lost when a later import fails.**

[params_of](contract/cli/src/tamber_params.rs:55) populates `package_roots` only on successful load. Its error branch returns empty roots, and [read_only](contract/cli/src/tamber_packages.rs:145) accepts that empty list.

Concrete reproduction: use the first fixture in [the new regression test](contract/cli/tests/it/tamber_serve_packages.rs:598), but make `app.contract`:

```text
use P from "p"
use Gone from "./gone.contract"
component App
  view
    P()
```

Leave `gone.contract` absent, then invoke the fixture’s existing `write(&dir)` transaction.

The resolver successfully records real root `vendor/p` before the second import fails. `params_of` discards that root. The filesystem fallback misses it because its manifest is `{}` and the node_modules links target individual files. The transaction consequently reaches `commit`, changing `vendor/p/design.tamber` from `#123456` to `#654321`.

This is inside **(c)**: the target is under a root the app’s load resolved. Preserve resolved roots on load failure, or refuse when they are unavailable.

Source-traced reproduction; not executed because it writes files.
