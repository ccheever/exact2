# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), round 1 of 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `0400fa8b66`.
- **Method:** one brief (sha256 `14bba41674dd9998b989925c2d739e63bd90646184e136e56355a81ac096a0a3`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

## BLOCKING

**Scrub planning skips the override transform — `contract/cli/src/tamber_scrub.rs:44`**

`scrub_fold` parses ops and plans them (`:76`–`:90`) without `tamber_packages::ops`. `propose` and `transaction` both go through that transform (`tamber_serve.rs:1129`); `scrub-release` does too, because it writes a `transaction` (`scripts/tamber-scrub.mjs:235`).

- `set-token` of a package or base name the app has not set yet is `no token` on the patch path (`scripts/tamber-scrub.mjs:128` returns that error and does not fall through to `propose`). The same op as a transaction becomes an `add-token` on the app file and commits.
- `add-token` of a package name, a base name, or a new name in a package namespace is a new line, so the fold’s class is rebuild. The scrub tells the client it applies on release (`scripts/tamber-scrub.mjs:132`). Release then hits the refusal in `tamber_packages.rs:151`. `verify: true` (`tamber-scrub.mjs:168`) plans the same untransformed ops.

A later `set-token` of an override the app already has is an ordinary edit of the app’s line, so that case does not need the transform.

## SHOULD-FIX

**`resolve` also returns `package_value` — `contract/cli/src/tamber_serve.rs:980`**

`package_fields` is shared by `resolve` (`:956`) and `explain` (`:1018`). The ruling and `docs/tamber-protocol.md` put `{value, whens, file}` on explain of an app-set package token only. `package_value` (`tamber_packages.rs:89`) already returns `None` unless the app’s design defines the name.

**`hello {paths: true}` is forwarded to any reader — `scripts/tamber-server.mjs:713`**

Client `hello` is `ask(request)` with the body unchanged, and the reply is spread. `paths: true` makes the design server add each package’s absolute `path` (`tamber_serve.rs:667`, `tamber_packages.rs:30`). `hello` is `read` scope, so a paired remote client can ask. The local watcher does not need this: `refresh` calls the child itself (`tamber-server.mjs:410`). `path` is the real file (`sources/packages.rs:170`), so a linked package outside the app is disclosed. `app` on hello already gives the app directory; this adds locations outside it. Describe never sets `paths`. The change event strips `path` (`tamber-package-designs.mjs:22`). Not acceptable on the HTTP hello.

**Revert and import still emit `add-token` for these names — `tamber/src/history.rs:398`, `tamber/src/import/mod.rs:143`**

Both feeds are held and then proposed, so they do hit `ops()`. Restoring a removed base or package override is `AddToken` (`history.rs:398`); an import of a name the file lacks is too (`import/mod.rs:143`). `ops()` refuses that and points at `set-token` (`tamber_packages.rs:151`), so the whole proposal fails. In-session `undo` is unaffected: the issued inverse is a `Restore` of the app file (`tamber_serve.rs:1316`) and `undo` does not run the transform (`:1189`).

## NIT

**Namespace ownership is only checked on `add-token` — `contract/cli/src/tamber_packages.rs:173`**

`rename-token` falls through `_`. Renaming an app token to `reading.extra`, or onto a package or base name, is not refused. The ruling states the namespace rule on `add-token`.

## What checks out

Reads for an app with no package designs are unchanged: `packaged` and `mark` return before adding keys (`tamber_packages.rs:26`, `:73`). `hello`/`describe` `packages` is `[{package, file, digest}]` with `file` = `home` (`@acme/reading/reading.tamber`) and `digest` = SHA-256 of the package text, the same digest as design files (`tamber_serve.rs:154`). Describe rows gain `package` and `file` only. `resolve`/`explain` add `package`, `file`, and `identity` (`<home>#<token>`). For an app-set token, `file` is the app design and `package_value.whens` is the package line the override drops.

`set-token` becomes an app `add-token` only when the file is the app design, `when` is empty, the app text does not define the name, and a package or `BASE_SCALE` does (`tamber_packages.rs:131`). That writes the given value only (`kind`/`meta`/`whens` cleared). It does not fire for a name the app already defines (including an existing override), for `set-param`, for icons, or for role rows. `BASE_SCALE` is the nine schema `scale` names (`space.xs`–`space.xl`, `radius.sm`–`radius.full`), not other schema rows. A `when` before that first set is refused (`:138`). After the app defines it, `set-token` stays `set-token`. `remove-token` is not rewritten: it deletes the app line (back to the package or the scale) and is `no token` when the app has no line. Base-name writes change for every app, not only apps with packages; the stage 11 B test was updated to `set-token`. That is the ruling, not the read-byte claim.

An op whose `file` is `home` or the absolute `path` is refused with the D11 read-only message (`:116`). Any other spelling is not that message; `plan` then rejects it as not a design file (`tamber/src/session.rs:274`), so it still does not write the package. `"tamber": "../…"` never becomes a listed path: `sources/packages.rs:126` requires the file to stay inside the package root after `..` folding and `canonicalize`. The watcher adding `dirname(path)` (`tamber-package-designs.mjs:17`) can therefore watch a linked package outside the app, which is what a disk change has to do, and cannot be pointed at an arbitrary directory.

The JS announce matches the ruling. The first successful read is the baseline while `known` is empty, so it records digests and does not emit. Later, added and deleted package files are both in `edited` (identity form, not disk path). `publish` runs first; a package-only change does not emit there. The package callback then `scrubs.compete()`, clears `params.key`, and sends one `changed` with `edited` and `packages` (no `path`). `learnParams` runs immediately after, so it refreshes. A held proposal stores the client ops; `accept` sends them through `transaction` again, and `app_params` reloads because the package file is on the walk (`sources.rs:647`, `tamber_walk.rs:132`). The JS test covers the drop-then-refuse case. A still-valid plan commits; the written text is the text that was reviewed.

Tests cover rows, `packages[]`, identity, `package_value.value`, the home-form read-only refusal, the three `add-token` refusals, `set-token` with a `when`, a base-name override, the change event, and the held-proposal replan. They do not cover the path-form refusal, `package_value.whens`, `resolve` lacking `package_value`, `remove-token` restoring the package, `undo`, scrub, `paths: true` on a client hello, or revert/import of these names.
