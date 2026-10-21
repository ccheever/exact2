# Code review: LLP 1121 stage 11 A1, package designs in the compiler, round 2 substitution, 2026-10-19 (fable)

- **Family:** Anthropic. Fable 5.1 (`claude-fable-5-1`), standing in for Grok under the lead's substitution rule (12:12): Grok's rounds 1 and 2 stopped without a verdict. Read-only, a throwaway worktree at `8cc68cb25a`.
- **Method:** a brief with the same context and checks as round 2's, blind to Astra's reviews. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve (2 SHOULD-FIX, 5 NIT).
- **Disposition:** taken: `pair=` names judged (`Checked.pairs`); tests for vendored discovery, another package's token, and the unbanded "(line 4)" text; an `exports "."` without `./` falls back. Not taken: the nameless nested `package.json` walk (it continues upward, as a component's identity does, `home()`); the redundant-looking `bindable` on package designs (it is what keeps them in an app with code beside it and no app.tamber, tested); the directory watch's cost (named in the commit message). A `SOURCES` comment added.

**Verdict: approve** (two should-fixes, no blockers).

**BLOCKING** — none.

**SHOULD-FIX**

1. `contract/lower/src/theme/packages.rs:253-269` — `references` judges `Checked.names` and `.dials` but not `Checked.pairs`. tamber resolves `pair=` names to tokens (`tamber/src/check.rs:291-293`), so a package token `reading.ink = #000 pair=brand` naming the app's `brand` (or another package's token) passes silently, against Q6. Add the same own/shared test over `checked.pairs`.

2. `contract/cli/src/sources/packages.rs:19-52` — the vendored-package walk (nested `package.json` above a relatively-`use`d file) has no test; it is the only discovery path not exercised by `package_designs.rs`. Also untested though claimed: cross-package reference refused (QUEUE (2)), a package motion token reaching `motion::tokens`, an app override of `reading.card` with another package token naming it, and the `unband`ed "(line 4)" text (the duplicate test asserts only `span.line == 6`).

**NIT**

- `contract/cli/src/resolve.rs:415` — `entry.strip_prefix("./")?` makes a package whose `exports["."]` lacks the `./` prefix silently have no design and no "unmade" path for the watcher. Refuse or fall back to `index.contract`.
- `contract/cli/src/sources/packages.rs:32-50` — `break` only after a manifest parses with a `name`; a nameless/malformed nested `package.json` lets the walk continue up and attribute the file to an outer vendored package.
- `contract/cli/src/lib.rs:512-516` — every linked package without a design now puts a `rerun-if-changed=<package dir>` directory watch into build scripts (Cargo scans it recursively). Intentional per the message, but it is a behaviour change for all apps with packages and can be costly for large node_modules packages; worth a note in the commit text.
- `contract/lower/src/lib.rs:1334` — `package_designs.iter().any(|d| d.bindable)` is redundant: `bindable` is the same `code_beside(path)` the app's design carries.
- `packages::merge` is the only setter of the `SOURCES` thread-local, and `with_package_designs` (graph, source ids all 0) resets it; today every reader re-merges first, but `span()` has no guard that the current bands belong to the file being lowered. A comment or passing the ids explicitly would make it less fragile.

**What checks out**

- Band scheme: `LINE=1<<28`, 256 bands of `1<<20`, upper bound `params::LINE=1<<29`, `SYNTHETIC=1<<30`; `merge` refuses `k>=256` and designs with `>=BAND` lines, so banded lines never reach the param range. Every reader (`token_span`, `design_refusal`, `span`, `unband`, `references`) range-checks `LINE..params::LINE` first and the three ranges are disjoint. Parse errors of a package design use raw lines (correct). Token name, value and each `when`'s line/value are all banded; `Token.meta` carries no lines.
- Refusals: header/namespace, base names, axes/dials/params/components/icons/exports, `when` against app axes (via the fold), dials via `Checked.dials`, name references via `Checked.names` — whose entries for role overrides are the row spelling (`check.rs:169-172`), which `shared()` matches case-insensitively through `roles::role`. Synthetic `~i` tokens and `param.*` tokens are above the band range and skipped; a `param.`-named package token hits `tamber-param-name` with a package span.
- Override: exact-name skip in `merge` (dotted names have no alternate spellings in tamber's duplicate check), so no cross-file `tamber-duplicate`; a package token naming an overridden name still resolves to its own declared name list.
- Discovery: link-order, root-dedup for aliases, manifest name for namespace/home, `use` spelling for Origin (consistent with other sources); containment covers `..`, absolute and symlinked paths; named-but-missing → `tamber-unreadable`; unnamed-missing → skipped and listed as unmade.
- No-package regressions: `merge` returns early; `design()`, `lint_scope`, `one_scheme_tokens` (judged slice `[..app]` holds since merge appends and the fold keeps order), `motion::tokens`, `names_tokens_outside_views` all reduce to prior behaviour. `theme_decl`/`params::unknown`/`bind_axes` read the app's design only, which is correct (packages can't declare axes/params).
