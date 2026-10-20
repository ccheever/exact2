# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), round 2 of 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `0d8dd32eca`.
- **Method:** one brief (sha256 `600c29979b03ed53e76422f725de0be8120735048b96c5a2607d12d1300ac771`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** findings below (none graded BLOCKING).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

## Findings

**[SHOULD-FIX] Keep `restore` server-only — `scripts/tamber-server.mjs:702`**

`add-token` of a package or base name is supposed to be refused unless the server is planning a revert or import. Two client paths skip that. `propose` with `hold: true` rejects `review.revert` but still accepts `review.importer` (any truthy value), and both `hold` and `accept` treat that as `restore: true` (`scripts/tamber-server.mjs:925`, `scripts/tamber-server.mjs:1028`). A direct `transaction` or `propose` is forwarded as sent (`scripts/tamber-server.mjs:682`, `scripts/tamber-server.mjs:731`), so a client can also pass `restore: true` itself. In `contract/cli/src/tamber_packages.rs:219` that leaves the `add-token` intact, including `whens`, `kind`, and `meta`. `set-token` would have written the value only (`contract/cli/src/tamber_packages.rs:209`). A paired writer can do this. It still writes the app's design, not the package file.

**[SHOULD-FIX] Do not watch a design path the loader refuses — `contract/cli/src/tamber_packages.rs:48`**

`watched()` lists every package-origin `.tamber` plus every consulted `.tamber`. Those come from `packages::sources` (`contract/cli/src/sources/packages.rs:193`), which does not apply the inside-package check in `packages::read` (`contract/cli/src/sources/packages.rs:126`). A `package.json` `"tamber"` of `/tmp/x.tamber` or `../../x.tamber` is `tamber-unreadable` and is not loaded, but it is still returned in `hello {paths: true}` `watch`. The local wire then watches that directory and its parent with no containment check (`scripts/tamber-package-designs.mjs:21`). `path.resolve` keeps an absolute path, so a one-level file such as `/tmp/x.tamber` watches `/tmp` and `/`. A linked package whose design stays inside that package is fine to watch, and those paths are not sent to clients.

**[NIT] The 1.14 bullet omits the rename refusal — `docs/tamber-protocol.md:104`**

`rename-token` onto a package name, a base name, or a package namespace is refused (`contract/cli/src/tamber_packages.rs:235`). The protocol bullet only mentions the `add-token` refusals.

## What checks out

**Rows and reads.** With no package designs, `hello` and `describe` omit `packages`, and `resolve` and `explain` add no package fields, so those answers stay byte-identical. With packages, describe rows gain `package` and `file` (the package's home, or the app design when the app sets the token). `resolve` and `explain` add `package`, `file`, and `identity` (`@acme/reading/reading.tamber#reading.card`). `explain` adds `package_value` `{value, whens, file}` only when the app defines the token. `resolve` does not. Digests are the package text's SHA-256.

**set-token.** It rewrites to `add-token` only when the app design, as earlier ops in the batch left it, does not define the name and the name is a package token or one of the nine base names (`space.xs`–`space.xl`, `radius.sm`–`radius.full`). Those match the stage 11 B scale, not schema rows. A name the app already defines stays `set-token`. Params, icons, and roles are not rewritten unless they collide with a package or base name. A `when` on the first set is refused. A later `set-token` with a `when` in the same batch is allowed once the name is defined. The new line is value and comment only (`whens`, `kind`, and `meta` dropped). `changed` names the app file. `remove-token` only updates that name set. It leaves a real remove of the app line, so the fold falls back to the package or base.

**Who runs the transform.** `propose` and `transaction` do. `scrub-fold` does, with `restore` forced off, after `param_names` (that pass only rewrites `set-param` and `unset-param`, so the order does not matter). Scrub release writes the original ops through `transaction`, which transforms them again. Revert and import holds store `review.revert` or `review.importer`, and both the plan and the accept pass `restore: true`. Undo does not transform: the issued inverse is a line `restore`, and `undo` puts those bytes back, including dropping an override line the transform had added.

**Refusals.** An op whose `file` is the package home or the stored path is refused as read-only before any rewrite. Any other spelling is not that check. `plan` then refuses it as not a design file, so the package file is still not written. `add-token` of a package or base name points at `set-token`. A new name under a package namespace is refused. `rename-token` onto either is refused.

**`hello {paths: true}`.** Acceptable. The wire forces `paths: false` on every client `hello`, paired or local, before the design server sees it (`scripts/tamber-server.mjs:713`). `path` and `watch` are not in that reply. `describe` lists packages with paths off. The internal refresh is the only `paths: true` caller. Its `changed` event keeps `{package, file, digest}` only.

**JS announce.** The first successful read is the baseline, so it does not announce. Later, a new, changed, or removed package identity is `edited`, and the event carries the current `packages`. A package-only change is one `changed` event. A design-file change in the same read is published first, then the package event. `scrubs.compete()` runs on that announce. `params.key` is cleared and `learnParams` runs afterward, so params are read again even when the design files did not change. Watches are installed after the new directories are recorded. A held accept re-plans on the server. The walk key includes `.tamber` digests, so a package edit is visible even if the proposal's file digests did not change.

**Tests.** The Rust tests cover rows, `packages[]`, resolve and explain, the read-only and add-token refusals, `set-token` with a `when`, a base name, a batch that sets then edits, rename into a namespace, `restore`, scrub-fold, and `hello {paths: true}` `watch`. The JS tests cover the listing, a disk change, a held proposal refused after the package drops the token, a design created where none was, and a client `hello` with no `path` or `watch`. Not covered: `package_value.whens` for a token that has whens, `remove-token` of an override, undo of a transformed set, a client-supplied `restore` or `review.importer`, and a `"tamber"` path that leaves the package.
