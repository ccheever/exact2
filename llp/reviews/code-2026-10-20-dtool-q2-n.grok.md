# Code review: a folded :disabled or :checked row read as its base (LLP 1122 queue item (2)), the narrow verification round of the post-round fixes, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `311f838635`.
- **Method:** one brief (sha256 `8c8b71a86632ad3dc5fec8ee3db4fdfdc30cc146c75cbabb28a7a9362a53c7d3`) quoting round 2's two reviews verbatim, asking of each finding "closed? broke anything?"; commit 311f838635. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings' heading; its narration before them left out).
- **Verdict:** nothing blocks (1 SHOULD-FIX, 2 NIT).
- **Disposition:** taken in the commit named in the sign-off request (and a second narrow round on it):
  - (1) as Astra's 1: exact identity from the map's `home`.
  - (2) not taken: a later full literal on one side shadowing an earlier part of its state is not told apart from one with a `none` arm without evaluating the fold's expression; each part stays listed, true to the source, labelled by its place, and its edit writes only its own line.
  - (3) taken: a role's spelling matches in any case, bare or `-exact-` (`canvastext` for `CanvasText`); tested.

**Closed**

1. **Param text (Review A blocker).** `writtenToken` trims, then accepts only one `param.<identifier>` (`scripts/tamber-inspect.mjs:490-493`). `" param.fill"` and `"param.fill "` resolve to the param, and an empty params map still refuses with "not read yet" (`:500`). `param.fill param.edge` does not match, so there is no `unread` and no `reason`; `editable` keeps the template rung (`:366`, `:502-503`). A part still returns before the lookup (`:495`), so `param.fill` on a part stays that part's template.

2. **Package param (Review A 3, Review B 2).** The base is read with `writtenAt`, which does not go through `rowRung`'s package refusal (`:447`, `:469-480`). The lookup is `#Component.name` among params read (`:496-498`), so `@acme/ui/card.contract#Card.fill` matches a file under `node_modules`. The part still goes through `rowRung`, so the package literal stays read-only (`:452-455`). An ordinary app file with one `Card.fill` still resolves while the disabled branch shows, because a single suffix match is enough (`:497`).

3. **Both parts of a state (Review A 2 and 5, Review B 1).** The same-state `retain` is gone (`states.rs:518-520`). The map writes every part (`contract/cli/src/map.rs:289-290`). `a_folded_row_keeps_each_part_of_a_state` (`contract/cli/tests/it/source_map.rs:1018-1070`) is the overlapping case: class `:disabled border-color` and node `:disabled border-top-color` with a `none` arm. Top keeps class line 12 and node line 6; right keeps only the class. `choose` / `over` are untouched, so a `none` arm still paints the earlier colour. Same attribute name on class and node is still one part, because `class.rs:42` drops the class attribute before the fold. `:checked` still runs before `:disabled`.

4. **Role case (Review A 4).** The written spelling is lowercased, then a leading `-exact-` is stripped (`tamber-inspect.mjs:502`). `-Exact-Secondary-Label` matches a runtime name `secondary-label`, and the token text is that name, so `editable`'s own compare succeeds. The template text stays the source spelling.

5. **Part under an unread host param (Review B 3).** The part's `editable` call passes `false`, not the host row's `unread` (`:455`). A literal `#999999` stays a template while the base param is unread.

Canonical join behaviour still lines up with the code: full name with `short: null`, param lookup while the disabled branch shows, refusal of an unread `param.fill`, a part's own edit, `color` / tint names, a placeless part at its node, and literal part text. One part of a state keeps its old inspector label; two of one state are told apart by place (`apps/design-tool/inspector.ts:109-112`). That label is display-only. The edit address is still the rung.

**Still open**

1. **SHOULD-FIX.** Two components of the same name are no longer separated by file. `writtenToken` (`scripts/tamber-inspect.mjs:496-499`) keeps every param whose full name ends with `#Component.name`. If several match and the runtime names do not include one, the row is refused ("one of N params"). Round 1 built `{file}#{component}.{name}`. For an app file that is `home()` (`contract/cli/src/sources.rs:286-287`, `:725-731`), so a disabled branch still selected `a.contract#Card.fill` rather than `b.contract#Card.fill`. A package needs that same `home()` (`@acme/ui/card.contract#Card.fill`, not the `node_modules` path). The suffix fixes the unique package case and gives up the file whenever the name is shared and the other branch is showing. It does not write the wrong param.

2. **NIT.** A later full literal on one side still lists the earlier part. Keeping every part is right for a `none` arm and for sides the later part does not write. A node `:disabled border-top-color="#999999"` over a class `:disabled border-color="#111111"` paints `#999999` on top: `over` leaves a literal alone (`states.rs:211`), so the class colour sits only in the not-disabled arm and never shows. The top row still carries the class part (`:520`). The other three sides correctly carry only the class.

3. **NIT.** A bare system-colour spelling still needs the runtime's exact name. `:502` case-folds only the `-exact-` form. `n === text` is unchanged, and `n` itself is not folded. The parser accepts `canvastext` and `CanvasText` as one role (`kernel/src/style/roles.rs:47-49`, `:945`); the schema name is the mixed-case one (`tamber/src/lib.rs:93`), and that is what a theme read records (`kernel/src/style/theme.rs:357-361`). A folded base `background-color="canvastext"` with the runtime reporting `CanvasText` still loses the token. The reported `-EXACT-SECONDARY-LABEL` case is fine, because that canonical name is already lowercase.
