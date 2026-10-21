# Docs review: LLP 1121 stage 11 A3, package designs as built, round 1 of 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at the commit under review.
- **Method:** one brief (sha256 `db2a9c1ea1ffaccc736dcc6cc1cb8e7fa45dadce03bc21097ad5edc345896892`), shared by both reviewers, blind to the other review; commit 8d65da90a6 (first: 38e31ec3c8). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING: the identity claim; 3 SHOULD-FIX, 3 NIT).
- **Disposition:** taken in 8d65da90a6: identity described as built (a token keeps its name; its source is <package>/<file>; the server's <package>/<file>#<token> is A2's), the collision vs the override rule, the guide's restrictions (theme only, declared by this file, pair and dials, base names the app defines), the shared last segment, the entry and "tamber" field, tamber gen vs contract types, index.contract and the outside/missing path, icons QUEUE.

## BLOCKING

**Token identity is not `<package>/<file>#<token>`** — `llp/1121-tamber.rfc.md:847`

The fold keys a package token by its name (`reading.measure`). `DesignSource.home` is stored as `<package.json name>/<path inside the package>` (`contract/cli/src/sources/packages.rs:171`) and nothing appends `#<token>`. The only `home#name` join is a component param on the app’s own design (`contract/lower/src/params.rs:89`). A package `component` is `tamber-package-block` and never reaches that join. D12 in this RFC says identity is the name.

## SHOULD-FIX

**The guide’s parenthetical defines “entry” as the name segment** — `docs/contract-for-agents.md:3541`

`reading` is the file name and the `tamber` header, from the last segment of `package.json` `"name"`. The file sits beside the entry: the file `exports` maps `.` to, or `index.contract` when `exports` is absent (`contract/cli/src/resolve.rs:414`). An export of `./src/index.contract` is read as `src/reading.tamber`.

**Base names are an allow-list, and a package may not declare them** — `llp/1121-tamber.rfc.md:845`, `docs/contract-for-agents.md:3547`

The names are `space.xs|sm|md|lg|xl` and `radius.sm|md|lg|full` (`contract/lower/src/theme/packages.rs:27`). They skip the package-reference check. They are not values: `reading.gap = space.md` is `tamber-unknown-token` unless the app declares `space.md`. A package named `space` or `radius` is also refused (`tamber-package-name`) for declaring one of those names, which is still `<name>.…`. Roles resolve with no declaration. The guide lists `space.md` and `radius.md` with roles.

**The guide never says a shared last segment fails the app** — `docs/contract-for-agents.md:3542`

`@acme/reading` and `@other/reading` are both namespace `reading`. When both design files exist, the build returns `tamber-package-name` and names both packages (`contract/cli/src/sources/packages.rs:153`). One file is allowed. The guide tells the author to take that segment and stops there.

**The guide bans axes and dials; four other blocks are refused too** — `docs/contract-for-agents.md:3546`

`icons`, `params`, `component`, and `exports` are `tamber-package-block`, and that package’s tokens stay out of the fold (`contract/lower/src/theme/packages.rs:141`). The RFC lists them (`llp/1121-tamber.rfc.md:849`).

## NIT

**Missing default file versus a bad `"tamber"` path** — `llp/1121-tamber.rfc.md:840`

No `<name>.tamber` means the package has no design. A `"tamber"` path that is missing, or that leaves the package after `..` is folded and the path is canonicalized, is `tamber-unreadable`. With no `exports`, the default file is beside `index.contract`.

**Icons are not stage 12** — `llp/1121-tamber.rfc.md:850`

Components are stage 12 in the code and in `QUEUE.md`. Icon tokens are queued with no stage.

**A bad reference is reported on the token’s name line** — `docs/contract-for-agents.md:3548`

`tamber-unknown-token` uses the token name’s line and column when the bad name is on a `when` line or in `pair=` (`contract/lower/src/theme/packages.rs:249`). A header mismatch is line 1, column 1. A namespace clash and an outside `"tamber"` path name the file at line 0.

## What checks out

The file rule matches the code: `<name>.tamber` beside the `.` export, or the in-package path `package.json` `"tamber"` names. Discovery is the packages the app’s sources link, including a transitive `use`, plus a package vendored inside one of those and reached by a relative `use`. The namespace is the last segment of the package’s own `package.json` name, not the `use` specifier. The header is `tamber <name>`, and every token must be `<name>.…` (`tamber-package-name`). Two packages that would share a namespace are refused only when both design files are read, and the message names both. The app’s token replaces the package’s whole, `when` lines included. A package token may name tokens that package declares (an app override of one still counts; an app-only `reading.extra` does not), roles, and those base names, including `pair=`, and it may not name a dial its own value reads (`Checked.reads`, not a dial reached through another token). Cross-package names are refused. Fold errors are mapped back onto the package file. The guide paragraph is the right length and uses a real file, header, token, and override.
