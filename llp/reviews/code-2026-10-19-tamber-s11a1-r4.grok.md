# Code review: LLP 1121 stage 11 A1, package designs in the compiler, re-check 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents` (the lead's 13:05 recipe), a throwaway worktree at `331f471132`.
- **Method:** round 2's brief (sha256 `dd2c7c05dad016ba3b31aeb823a571364aa7e7f54440caae4c62e58c9e01d723`), blind to the other reviewers. Rounds 1 and 2's Grok runs stopped without a verdict (before the recipe); Fable stood in for round 2 (its own record).
- **Transcription:** the final message from its verdict on, unedited but for the worktree path prefix.
- **Verdict:** approve (1 SHOULD-FIX, 2 NIT).
- **Disposition:** the SHOULD-FIX (dial words) was already fixed in the next commit (Checked.reads); taken: a real path that can't be resolved counts as outside. Not taken: driver refusals at line 0 (they name the file).

No blocking defect. The namespace reading is right: a clash is two designs that would define tokens under one last segment, and the error names both packages. Linked packages that do not both ship a design define nothing that can collide.

## SHOULD-FIX

**Dial scan false-positives on CSS function names and quoted font words** — `contract/lower/src/theme/packages.rs:287-291`.

The fold records token and `pair=` names, and it resolves a dial only when an identifier is looked up (`tamber/src/check.rs` `Scope::named` checks dials and does not put them in `refs`). The extra scan then treats every `scan::words` word as a dial read. `words` (`contract/lower/src/theme/scan.rs:61-63`) keeps letters, digits, `-_.` and splits on everything else, so function names and quoted stack members are words: `light-dark`, `color`, `color-mix`, `contrast-color`, `rgb`, `oklch`, `calc`, `min`, `max`, `clamp`, `round`, and `"Inter"`.

Those names are legal dials. `reserved()` (`tamber/src/check.rs:359`) is CSS colour keywords plus `none`/`from`/`in`/…, not function names, and `color`/`rgb` parse only with a body (`color/src/lib.rs`). An app dial named `color` refuses a package token written `color(display-p3 …)` that the fold accepted. Same for `light-dark(reading.ink, …)` and a family stack `"Inter", system-ui`.

There is no matching false negative. A dial is an ident (`tamber/src/lex.rs:191`), and every ident is one word, so a direct dial read is still caught (`accent` is tested). A dial reached only through the package’s own token is correctly left alone. `var()` never gets here: the fold refuses it (`tamber/src/fold.rs:317`).

`shared()` is the right exemption for the fold’s names, not for this scan. A role or base name that is also a dial really is the dial (`named` tries dials first), so refusing it is correct. An app token that is a role stays allowed: `refs` stores the row spelling (`-exact-label`, `Canvas`) and `roles::role` accepts both (`kernel/src/style/roles.rs:18-33`). An app token named the namespace word (`reading`) or `reading.extra` is not own and not shared, so it is refused. `reading.extra` is tested.

## A2 seam (not an A1 defect)

Nothing in the tree ships a package design, so today’s server and the 43 unchanged apps still build. The split appears as soon as a linked package does:

- `design_with` folds only the snapshot file (`contract/cli/src/tamber_serve.rs:721-738`). An app token that names a package token is `tamber-unknown-token` in describe/check, and the build accepts it.
- `synthetic` and `uses` call `compile_path_all` / `compile_path_mapped` (`tamber_serve.rs:394`, `:1037`, `:1053`), which load package designs. A package-side refusal fails the server with “the app does not build” even though that file is not in the snapshot.
- `relocate` only remaps `params::LINE` (`contract/cli/src/tamber_params.rs:68-71`). A merged design folded there would attribute a banded line to the app file. `contract tamber check` without params builds a temp app of the design text alone (`contract/cli/src/tamber.rs` `admission`), so it disagrees with `contract build` the same way.

## NIT

- Driver refusals for “outside” and a namespace clash use `Span::default()` (`contract/cli/src/sources/packages.rs:63-70`): the file is set, the line is 0. Header mismatch is line 1.
- `canonicalize().map_or(true, …)` (`sources/packages.rs:126-129`) treats an unresolvable path as inside. A broken symlink still fails `read_to_string`. A readable file whose real path cannot be computed would be accepted.

## What checks out

**Bands.** `LINE = 1<<28`, `BAND = 1<<20`, at most 256 designs, at most `BAND-1` lines. The highest stored line is `params::LINE - 1`. `token_span` tries the package band before the param band and `SYNTHETIC` (`theme.rs:719-742`); `design_refusal` does not treat a package line as a param or a synthetic (`theme.rs:748-767`). `name`, `value`, `when.line`, and `when.value.line` are all shifted (`packages.rs:190-195`). Axis and ambiguous-`when` diagnostics use `when.line`; value errors use the value’s line; the duplicate message’s `(line N)` and the ambiguous-`when` `line N` go through `unband` (`packages.rs:204-215`). Parse errors and block refusals use the package file’s own lines. `laid()` does not rewrite those lines. The duplicate test locks line 6 and `(line 4)`.

**`SOURCES`.** It is a thread-local reset at the start of every `merge`, including an empty list (`packages.rs:97`). In one compile, `motion::tokens` merges first and keeps only duration/easing name and css (`motion.rs:77-95`); it never calls `span`. `declare_theme` merges again and resolves every span before it returns. `lint_scope`, `one_scheme_tokens`, and the source-graph icon fold (`with_package_designs`) merge later, on passes that do not map lines. The icon fold’s designs use `source_id` 0 and drop diagnostics (`.ok()` / `unwrap_or_default`). A later compile on the same thread merges before it maps. Threads do not share the cell.

**Merge refusals.** Header mismatch, a token not under `{ns}.`, a token that is a base name, and axes, dials, components, params, icons, and exports are refused, and that whole package is left out of the fold. An app token of the same name drops the package token, `when` lines included (`packages.rs:186-188`), which the when-test compiles. `own` is “declared in that package’s text”, so an app override of a package token is still that package’s to name, and an app-added `ns.extra` is not.

**Driver.** Link order is use order, then vendored packages in source order, deduped by canonical root, first wins. Namespace is `package.json`’s last segment, not the `use` alias (tested). `..` is folded before the containment check, so a `"tamber"` path that escapes is refused even when named. A symlink of the file, or of a directory on the way, whose real path leaves the canonical root fails `starts_with`. A missing default file is skipped and recorded; a missing named file is `tamber-unreadable`. Cargo watches the parent directory of a missing `.tamber` when that directory exists (`contract/cli/src/lib.rs:509-516`), not the missing path, so it does not rebuild every time. `stamp_of` is `None` while the file is absent (`host/web/src/dev.rs:89-105`), and the JS loop treats a never-present consulted path as not gone (`host/web-js/dev.mjs:121`). Adding the file changes the stamp and the package-root watch. A relative `use` inside a linked package finds a vendored `package.json` (tested). `exact.json` packages are ordinary `Origin::Package`; an entry without `./` still places `<ns>.tamber` beside that file, and resolution already requires a `./` path inside the package. An app-relative `./vendor/…` stays `Origin::App` (`resolve.rs:169`) and is not given a design; the driver only adds a package inside one a relative `use` reached.

**Other readers.** `one_scheme_tokens` folds package tokens in, then judges only the app prefix (`theme.rs:1149-1168`). `theme_decl` stays on the app file, which matches that. Package `params` blocks are refused, so `params::unknown` / `design_tokens` staying on `file.design` is right. `edits.rs` `mix_edit` probes an isolated design; a mix over a token (app or package) does not fold there, so it does not invent a “declare the mix” edit. `folding.warnings` are still only printed by the single-file check and the server, not by the lowerer. `bindable` on a package design is the app’s flag (`names_tokens_outside_views` ORs it); retention still walks `words` on kept values.

## Tests

The 13 tests in `contract/cli/tests/it/package_designs.rs` cover the rules that landed: styles and full-name override, D7 drop and bindable keep, header and token namespace (error file), every forbidden block, `when` against the app’s axes and override dropping `when`s, in-file duplicate line and unbanded `(line 4)`, second package’s bad name, two designs under one namespace naming both, app token / role / `light-dark` / hex / `reading.extra`, `appFace` once declared, `space.md` when the app sets it, a package named `space` not declaring `space.md`, a `"tamber"` path outside, `use` alias vs `package.json` name, no app design, direct dial, `pair=` of an app token, another package’s token, and a vendored package plus its consulted `package.json`.

Untested, and worth a test where noted: the dial-vs-function false positive above; a `when` or bad-value span (the shift is the same code as the duplicate line, but no test reads it); symlink containment; a missing named `"tamber"` file versus a missing default file; an in-package `"tamber"` path and a design beside a nested `"."` entry; a second package’s band actually mapping to that file; the allow when only one of two same-named packages has a design; a package naming an app role-override (`label` / `-exact-label`); a whole alias of a profile colour.
