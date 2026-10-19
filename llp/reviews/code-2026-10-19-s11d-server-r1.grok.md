# Code review: the server half of stage 11 D (base names on the wire, tamber/1.14), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e9b1291f3c`.
- **Method:** one brief (sha256 `6a36bb1e6749d89950ace9daadcb41fecff3b0c1490e157250fdadc9a6b1c2c0`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review this commit read-only: the message, the wire shape, and the files you named, then check override detection, describe targets, and cost.The commit and the brief’s base-row shape are in view. Next I’ll read the wire code against that shape and the existing describe/resolve/explain paths.The wire path and the checker disagree in a few places. I'll run the two allowed tests, then pin the findings to lines.Two blockers. The row shape for a design that checks matches theme2's base row, and both allowed test runs passed (`tamber_serve_base` 2, kernel `scale` 10). Those tests never leave the happy path.

## Blocker

**1. `resolve` and `explain` of a base name skip the file check when the name looks unset.** `contract/cli/src/tamber_base.rs:40-43`, `:100-101`, `:119-121`.

`design()` turns a missing file and a parse error into `None` (`.ok()`). `row()` then stores `override: null`, and both methods return the unset body without calling `s.resolve` / `s.explain`. A request that used to fail now succeeds:

- `resolve`/`explain` `{token: "radius.lg", file: "nope.tamber"}` while `app.tamber` sets `radius.lg` answers `{name, base}` with `override: null` and no tuples. `design_with` (`tamber_serve.rs:732-733`) would have said `` `nope.tamber` is no file of this design ``.
- The same for a file that does not parse (`app.contract`, a syntax error). `describe` still fails, because it calls `s.describe` first (`tamber_base.rs:77`).
- A package that declares `space.md` is refused before join (`contract/lower/src/theme/packages.rs:177-190`, `tamber-package-name`). `describe` reports that. `explain {token: "space.xs"}` does not, and answers the unset row.

Fix: run the same load as `design_with` first. Return the unset body only after that check succeeds and the checked design has no token whose name is exactly the base name. Take `override` from that token.

**2. `bases` misses every base reference that is not a token in the requested file.** `contract/cli/src/tamber_base.rs:129-146`.

The scan reparses that one file and looks up the request string in `tokens` only. `explain` itself folds the joined design (`tamber_params.rs:128-134`, `packages.rs:198-209`). A package token `reading.measure = space.md` is on that reply: its value text is `space.md`, and when the app has not set `space.md` the checker's `names` stays empty (`check.rs:92-104`, a bare unset base is a named length and never enters `refs`). `bases` is also absent. A param default or `params` line is the same miss: those live on the laid token and in `design.params`, not in the file's `tokens` under the request name.

That is the reference the field was added for. An app token in the requested file is the one case that works (`card.pad` in the test).

Fix: after `s.explain` returns, read `value` and each `whens[].value` off that reply (package tokens and params are already there). Collect CSS idents the way `tamber/src/css.rs` `ident` does, match them exactly to `scale::names()`, keep first-seen order, dedupe. Drop the request-file reparse for this.

## What holds

**Shape.** A checking app's `describe` reads `scale::row` / `cell` / `COLUMNS` / `names` / `css`. No copied table. Nine names in schema order. All five columns, each `{value, source}`. `Px` goes through `scale::css`. `Full` is `"full"` (not `9999px`); other `radius.full` columns are `{kind: "full"}`, and its terminal cell is `"0"` / `{kind: "terminal"}`. Terminal `Cells(0)` is `"0"`; the table's other cells are `1` and `2`, so Display yields `"1cell"` and `"2cell"`. Sources are `metric.ref`, `measured.id`, `guideline.cite`, `full`, `terminal`. `stretch` is not in `ScaleSource`, so it is not emitted. `kind` is `radius` for `radius.*` and `length` otherwise. `override` is `null` or `{file, line, col, text}`.

**Override.** Exact token name, the same test the checker uses (`check.rs:96-98`). A `when` line cannot declare a token (`syntax.rs:511-514` requires `name = value`), so a when under `card.pad` is not an override of `space.md`. `text` is the default value; `line`/`col` are the name's, 1-based. `file` is the request's `file`, otherwise `s.design`. A real file that parses and contains the token gets that path. A linked package cannot set a base name, so the app line is the only legal override. A second `.tamber` is the design only when `file` names it; it is not joined as a package.

**`resolve` / `explain` when the file checks.** Unset resolve is `{name, base, files}` and no tuples. Set resolve is the usual `{token, tuples, files}` plus `base`. Unset explain is `{kind: "base", name, base, files}`. Set explain keeps `token`, `value`, `whens`, `dials`, `names` and adds `kind`, `name`, `base`.

**`bases` on an app token in that file.** Whole words: `space.md2` and `x.space.md` stay one word, and `css.rs` `ident` keeps them as one ident too, so neither matches `space.md`. `when` values are scanned. `light-dark(space.md, space.sm)` splits on `(`, `)`, and commas, so both names are found; the checker sees those idents and refuses a base inside `light-dark` (`fold.rs:299-306`), so a design that checks never carries that value. Duplicates keep the first, default then whens. Direct names only, which matches the protocol sentence ("values name"); `card.pad = other` with `other = space.md` does not list `space.md`.

**Targets.** `shipped_targets` is `targets()`, the same list `draws` is keyed by, sorted by name. `column_for` matches the hosts: Apple `cfg!(target_os = "macos")` then `Column::Mac` else `Column::Ios` (`host/apple/src/scale.rs:15-18`); web and Linux set the neutral role column, and `metrics::column()` follows that when no metric column was set (`kernel/src/style/metrics.rs:86-94`); Android's reader calls `show_material_look()`, which sets `metrics::Column::Material`. `macos` maps to `mac`. `tvos`, `visionos`, `watchos` map to `ios`; `windows` maps to `neutral`.

**Cost.** No non-base request newly fails. `resolve` of a non-base returns at `tamber_base.rs:94-96`. `explain` of a non-base, an icon, or a param still finishes in `s.explain`, so a missing file and a refused design still error there. `describe` of a bad file still errors in `s.describe`.

## Should-fix

**Quoted strings are counted.** `tamber_base.rs:137-139`. The splitter cuts on `"`, so a family the checker accepts, `ink: family = "space.md"` (`typeface.rs:257-284` never calls `named` on a quoted member), gets `bases: ["space.md"]`. The ident walk in blocker 2 covers this. Add that case next to `card.pad`.

**`describe` and every `explain` read the design twice.** `tamber_base.rs:77-79` and `:115-116`. `s.describe` / `s.explain` already snapshot and parse; the second snapshot is the parse that cannot see joined package tokens. Once override and `bases` come from that first check, delete the extra `snapshot()`.

**A shipped target with no column disappears.** `tamber_base.rs:83-86`. `filter_map` drops whatever `column_for` returns `None` for. `targets()` (`icons.rs:178-186`) also copies every key of `deploy.store` and `deploy.binaries`, and both objects allow any key (`scripts/app.schema.json:683-687`). `draws` still lists that key. Every schema host (`ios`, `macos`, `android`, `linux`, `windows`, `web`) has a column, so a normal app loses nothing. Keep the target and send `"column": null` so the set stays the draws set.

**Tests.** `contract/cli/tests/it/tamber_serve_base.rs`. They lock kernel name order, per-column value text via a copy of `value()` (so `"1cell"` and `"0"` are not checked against the spec), `source.kind` is a string, one app-file override, `radius.full`'s iOS value, ios/macos targets, unset vs set resolve/explain, and `card.pad`'s `bases`. They do not lock `ref` / `id` / `cite`, a `file` request's `override.file`, a missing file, a package token or a param, `space.md2` / `x.space.md` / a quoted family / a duplicate, or that non-base resolve/explain still errors. `each_targets_column_is_the_one_its_host_reads` (`:119-120`) only asserts that `Column::Mac` and `Column::Ios` both occur in `host/apple/src/scale.rs`, so swapping the `cfg` arms still passes. Assert the branch, and assert `windows` → `neutral`.

## Nit

**Column object key order.** `tamber_base.rs:51-60`. `serde_json::Map` is a `BTreeMap` here, so the wire emits `material` before `neutral`. All five keys are present. Every other object on this wire sorts the same way. The nine rows are in schema order. No change.

**Doc wording.** `docs/tamber-protocol.md:378`. "The app's own line" is the default request. A `file` request reports that file's token. Say the design file the request names.
