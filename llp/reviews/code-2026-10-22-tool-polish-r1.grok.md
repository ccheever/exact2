# Code review: the design tool's stage 13 E polish (the dry commit's key, the Files fold), round 1 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `dd09005949`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; one SHOULD-FIX (the colon guard) and a NIT (the paced comment), both taken.

I'll review the polishing commit at HEAD only, read-only, against the dry-run key, the client-name guard, and the Files fold.The review is the polishing commit only. I'll read that diff and the surrounding commit-scope and files-fold code, without changing anything.The diff is small. Next I’ll trace when `fileChanges`, `commitsTold`, and `session` move, and how the client name and the Files toggle are built.Nothing blocks. Commit’s enabled state stays current for every case you listed, and the fold hides only the digest rows.

Commit is enabled from the last dry run (`scope.open`, `refused`, or `unavailable` at `app.contract:2275`). That run is repeated only when `committable` changes (`app.ts:1016`, `app.contract:903`): a `changed` event or a missed/new run bumps `fileChanges` (`app.ts:554`, `app.ts:1065`), a `committed` event bumps `commitsTold` (`app.ts:548`), and another run bumps `session` (`app.ts:294`, `app.ts:1049`). A proposal, import, or revert only writes when it is accepted, and that accept, an undo, a redo, and a scrub’s Keep all go through `write()`, which publishes `changed`. Writes from before the tool opened are on the server already; the first dry run after the name is set reads them, and the first poll’s `fileChanges` bump reads them again. A missed answer skips per-event counting and still bumps `fileChanges` once, so one dry run sees the server’s current session. A git commit outside the tool emits neither event and does not change file bytes. The button was already on, and a moved HEAD is a refusal (`scripts/tamber-commit.mjs:160`), which leaves it on. Pressing Commit dry-runs again inside `commitSession`.

The empty-name guard is right for `design-tool:` and for every paired client. Pairing names are `remote:<name>#n` with `<name>` limited to letters, digits, spaces, dots, dashes, and underscores (`scripts/tamber-pairing.mjs:274-276`), so they cannot end in `:`.

The file chooser stays above the tokens (`app.contract:1880`). Behind the toggle are only path and digest rows, after Contrast and before Proposals (`app.contract:2171`). The button’s accessible name says show or hide and the count.

1. **SHOULD-FIX** `apps/design-tool/app.ts:1211` — `client.endsWith(':')` treats any trailing colon as “no one”. A local name is free text (`app.contract:1057`), so `Ada:` is the client `design-tool:Ada:`. The dry run never runs, and Commit stays disabled with no error. The check that matches the comment is `client === 'design-tool:'`.

2. **NIT** `apps/design-tool/app.contract:354` — The `paced` comment still says the commit’s dry run is read on `paced`. It is read on `committable` now.
