# Code review: the Tamber server writes a library component's param by its full name (LLP 1139 D8), round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `828e32f657`.
- **Method:** one brief per round (sha256 `cd95eb2371446aa4a8ec08e859a1fb2b8c64b28ff449dcf3bfe43a3789054813`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no findings
- **Disposition:** none needed

No findings.

`param_names` writes a new library param line by `p.full`, and that is the same set the build refuses as `tamber-param-short-library`.

`Described.library` is `file.library.contains` on the component identity (`contract/lower/src/anatomy.rs:745`). `app.library` keeps every contract component in that set, parts or none (`contract/cli/src/tamber_params.rs:74`). Param tokens use that same identity (`contract/lower/src/params.rs:72`). So `identity == p.identity && d.library` (`contract/cli/src/tamber_ops.rs:183`) is the success arm of `param_lines` (`contract/lower/src/anatomy.rs:487`).

Design-declared components stay outside both sets. Their identity is `{package}/{file}.tamber#Name` from `design_tokens` (`contract/lower/src/params.rs:96`). `file.library` is only contract entry declarations and re-exports (`contract/cli/src/sources/library.rs:67`). A short line for one of those still builds, which is what `set_and_unset_param_write_the_apps_line_alone` expects for `Map.pin`. Re-exports are inside both sets: `use X from "./x.contract"` (and a package subpath, alias resolved to the declared name) is recorded as `{home}#{declared name}`, and the loader stamps the component with that same `home()`. A loaded re-export is in `app.library` with `library: true`, so a new line is written in full. An export the app never loads has no param token.

No other writer emits a new short library line. `propose` and `transaction` share `param_names` (`contract/cli/src/tamber_serve.rs:1054`); scrub frames do too (`contract/cli/src/tamber_scrub.rs:89`). A `when` line does not carry the param name; the design tool's "default, then when" pair shares the `written` spelling, so the second op keeps the full name. An existing line, including one already spelled short, keeps that spelling (`line.or(earlier)`), and an unset-then-set in the same transaction does too because the lookup is on the pre-transaction snapshot. That file was already unbuildable. `rename-token` rewrites param values only (`tamber/src/edit.rs:865`). `set-row`, `add-param`, and promote write contract `param.<name>`. Import does not emit `set-param`.

Keep advertising `short`. The wire still accepts a short name; this server canonicalizes a new library line to `p.full`. `lineName` in the design tool can keep sending `short`.

The new test fails without the fix: the file would contain `Card.inset = 20px`, and `compile_path` would hit `tamber-param-short-library` (`contract/lower/src/theme.rs:307`). Propose and the write both go through `param_names` and `plan`, and `diff` is the unified diff of that text (`tamber/src/session.rs:271`), so equality fails if only one path rewrites. The full-name assertion and the compile catch a miss on both.
