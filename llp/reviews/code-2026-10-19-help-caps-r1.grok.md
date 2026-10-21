# Code review: caps runs the helps' flag parity before every push (scripts/help-parity.mjs), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `4f4583b361`.
- **Method:** one brief (sha256 `723922e6eac3496b2ce131ed49ec7f50cc0eba6ead7f3517bea8f2b6c70a43d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; should-fix (slices; a source error returning early), one nit
- **Disposition:** taken in ed11a1803c, with Astra's; the counted line says problems, not disagreements

Nothing blocking. The flag-set and length checks match `help.test`, and on this checkout `require` of the ESM module works. The holes are a missing `web`/`test`/`agent` entry, and a slice that moves.

## Findings

1. **should-fix** — A missing `web`, `test`, or `agent` help throws away every help problem already found (`scripts/help-parity.mjs:67`, `scripts/caps.mjs:272`).

`helpText` returns `null` when the verb is absent. The short-help loop calls `helpText(verb).length` before it checks `HELP[verb]`. For `verify` or `hatch`, a missing entry is reported as `help:verb` and the run continues. For `web`, `test`, or `agent`, that `.length` throws. `helps` catches the throw and replaces the whole result with `help:parity` and `Cannot read properties of null (reading 'length')`. The `help:verb` line, and any stale, unsaid, or over-long helps already collected, never reach the report. The push stays red, and the message does not say which verb to add.

2. **should-fix** — A `between` or readiness slice that moves is reported as flag drift, and the repair text makes the next run green (`scripts/help-parity.mjs:25`, `scripts/help-parity.mjs:27`, `scripts/help-parity.mjs:61`).

`scriptFlags` throws only when `pairTtlOf` is gone. `between` and the readiness `slice` do not check their markers. If the start marker is gone, `indexOf` is `-1` and `slice(-1, end)` is empty, so `parsed` returns `[]`. Every flag that help names becomes `help:stale`, and the detail says to drop it from `HELP`. Dropping it makes the following run agree with the empty slice, while the script still parses the flags. `--ready` and `--timeout` are read only from that readiness slice, so a moved `if (verb === 'status'` marker is enough to send a builder down that path. If the end marker is gone, `slice(start, -1)` runs to the end of the file and later verbs' flags are charged to this one as `help:unsaid`, with advice to add them to the wrong help. `new`'s end marker is the first `\n  }`, so a same-indent close earlier in that handler hides every flag after it. The test that says a missing slice is reported only renames `pairTtlOf`.

3. **should-fix** — A source-read failure returns before the checks that only read `HELP` (`scripts/help-parity.mjs:55`).

The `catch` around `scriptFlags` records `help:sources` and returns. The 36-line check, the 1800-byte check, the index check, and the form check never run. A missing `pairTtlOf` plus a 40-line help is one caps run, then another after the slice is restored. Those four checks do not need the scripts.

4. **nit** — The index failure always says both limits were passed (`scripts/help-parity.mjs:73`).

The condition is lines over 36 or bytes over 2800. The message always says "over 36 and 2800". Both numbers are printed, so the true one is visible, and the sentence still names the wrong limit.

## What matches

`helpParity` and `help.test` agree on pass or fail for the checks this commit names. Flag sets are the same comparison: `knownFlags` keys against `scriptFlags`, split into unsaid and stale. A help over 36 lines, `web`/`test`/`agent` short text over 1800 (JavaScript `.length`, the same quantity `help.test` calls bytes), the index over 36 lines or 2800 bytes (`Buffer.byteLength`, same `indexText` arguments, `new` and `add` omitted), and a form naming a flag absent from `knownFlags` and from the flag rows, including the `contract` skip, all use the same bounds. `parsed` and the verb-to-files table were moved unchanged. `help.test` still calls `scriptFlags(ROOT)` and keeps the same `deepEqual` and 36-line loop.

These `help.test` assertions stay only there: the `More` line, `Ports`, every flag token present in `--help all`, the web and agent prose, the fixed index forms, `status` not printing `undefined`, and the `lock` index with no forms. A wording change can still go green in caps and red in `help.test` after the push. That matches the commit message. `help:verb` and `help:sources` are new reports for cases where `help.test` already throws (`knownFlags` on a missing entry, or `pairTtlOf` missing).

## caps wiring

`require` of `scripts/help-parity.mjs` from an ESM file under Bun returns the named exports, `helpParity` included. `import.meta.main` in `help.mjs` stays false on that path, so loading it does not print help and exit. A throw is caught and recorded as `help:parity`, then `helps` returns, so the run is red and the other budget problems already collected are still printed. `runCaps` stays synchronous, which is what `windows.test` calls.

`caps.test` builds throwaway repos that have no `scripts/help-parity.mjs`. `existsSync` fails and `helps` returns without a counted line, so those cases stay as they were.

`HELP` is a static import of `./help.mjs`, resolved next to whichever `help-parity.mjs` was loaded. `scriptFlags` reads `root`. For caps those are the same tree: `helps` does `require(resolve(root, 'scripts/help-parity.mjs'))` and then `helpParity(root)`, and `main` passes the one root it chose (git toplevel, or the cwd when only the cwd has `rules/RULES.md`). The split shows up when the module is imported from this checkout and called on another directory. The copy-and-edit test depends on that: it edits copied scripts and compares them with this checkout's `HELP`.

## Messages and tests

Unsaid, stale, over-long, form, and missing-verb messages name the verb, the flags, and `HELP['…']` in `scripts/help.mjs`, with a concrete edit. The counted line calls every problem "disagreeing", including a help that is only too long. The problem text under it is specific.

The fixture copies every file `scriptFlags` reads. Bun's `cpSync` creates the nested destination directories, so the copy works. The unsaid case, both `agent` and `test` going stale when `--bind` disappears, and a renamed `pairTtlOf` are real assertions. The caps assertion checks that a real `caps.mjs` run prints `helps: … 0 disagreeing`. It does not check the exit code, and it does not feed caps a broken tree. Length, index, form, and a missing `HELP` entry are untested. The stale assertion ignores the `--bound` unsaid problems that the same edit also produces.

`help.test`'s parity assertions are the same comparisons as before. A missing `pairTtlOf` now fails as a thrown `Error` from `scriptFlags` rather than `assert.ok`. The test still fails.
