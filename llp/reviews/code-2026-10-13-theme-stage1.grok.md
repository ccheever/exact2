# Code review: LLP 1121 stage 1 (theme and its reach), round 1, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `097903c5ad86423aad04882b8afb0938809179ed4a8c5fb11d43b1e2734184af`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 5 MAJOR, 11 MINOR; Grok 2 MAJOR, 4 MINOR).
- **Disposition:** all fixed in round 2 (0e81ae8b9)

---

**MAJOR** — The JS runtime map bakes a tint role into a concrete colour. `host/web-js/src/style.rs:422`

`theme_map` records every entry name in `N`, including `AccentColor`, `LinkText`, `-exact-link`, and `-exact-symbol`, as the override's CSS. The roles map skips those tint roles, and the comment says they stay themselves so `--exact-accent` carries the theme colour under an authored accent. A dynamic string `"AccentColor"` (a data binding, or that token inside a shadow) becomes the theme hex. The literal path leaves the row as the role and emits `var(--exact-accent, …)`, so the same word follows an authored `accent-color` there and ignores it through the map. `a_bound_theme_name_writes_its_entry` never passes `AccentColor`. `theme-reach.contract`'s `color="AccentColor"` is a literal, so it never hits this map.

**MAJOR** — Linux paints a tint role as the theme colour, and symbols stay on the `symbol` role. `kernel/src/style.rs:729`

`ColorValue::resolve` substitutes an override for every role, including roles `follows_accent` is true for. `themed()` (`kernel/src/style/roles.rs:304`) keeps those roles, which is how Apple's tint and the web's `--exact-accent` let an authored `accent-color` win. Linux text, backgrounds, SVG fills, and shadows call `resolve`, so `color="AccentColor"` under `accent-color="#1083fe"` paints the theme colour. Untinted symbols resolve the `symbol` role (`host/linux/src/paint/svg.rs:115`), so a theme `AccentColor` never reaches them. Controls that go through `control::accent` do both halves: the authored accent wins, and otherwise the theme accent is used. This diff does not change the Linux painter.

**MINOR** — `GrayText` misses the default look. `kernel/src/style/roles.rs:324`

`theme_css` lowercases a CSS system name, so `GrayText` becomes `--exact-graytext`. The apple sheet was updated to that spelling. The default neutral sheet still paints disabled buttons, fields, and menu items with `--exact-gray-text` (`host/web/neutral.css:30`, and the rules at 77, 119, and 248), and that sheet wins over the apple rules. A `GrayText` override recolours a row that names the role and leaves the neutral look's disabled chrome unchanged.

**MINOR** — Two spellings of one role are both accepted. `contract/syntax/src/parser/theme.rs:42`

The duplicate check compares the written name only. `secondary-label` and `-exact-secondary-label` lower as two entries for one role id. `keys()` emits the canonical spelling from the first entry first, and both `resolved()` and `overridden()` keep the first match, so the second colour is unreachable. LLP 1121 treats the bare key and the spelled role as one override.

**MINOR** — A tint-role override other than `AccentColor` never shows as the fallback. `kernel/src/style/roles.rs:514`

For `LinkText`, `link`, and `symbol`, `reference_css` writes `var(--exact-accent, <override>)`. The neutral sheet always defines `--exact-accent`, and `theme_css` assigns that variable only for `AccentColor` (line 323). The override colour is unused. Apple installs only `ThemeRoles.color("AccentColor")` as the tint (`host/apple/Sources/ExactKit/ThemeRoles.swift:53`). An entry that overrides `LinkText` and leaves `AccentColor` alone changes neither links nor symbols on the web or on Apple.

**MINOR** — A rejected boot still replaces the thread's overrides. `runner/src/runner.rs:692`

`set_role_overrides` runs before the clock, schema, app-id, and root checks. Any of those can return an error, and the journal line is written only after a successful boot (line 1062). A failed second boot leaves the new plan's roles in place and bumps the reported generation, so a session still running the previous plan repaints in the rejected theme with no journal line.

The per-thread override table, with a journal line when a second boot succeeds, matches the stage-1 limit. Cycles, CSS colour names as entry names, hex skipping the theme table, embedded colours at parse, the Apple `roles` op (colour values through `values()`, oracle included), and `:root` winning over the neutral sheet for `--exact-accent`, `Canvas`, and `CanvasText` all look right.

VERDICT: fix (MAJOR host/web-js/src/style.rs:422, MAJOR kernel/src/style.rs:729, MINOR kernel/src/style/roles.rs:324, MINOR contract/syntax/src/parser/theme.rs:42, MINOR kernel/src/style/roles.rs:514, MINOR runner/src/runner.rs:692)
