# Code review: LLP 1121 stage 3, commit 7, round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `f1c756ace1eb95ef9285ba56d4cfe8f6f1f2f92f127d9c25077b267f87acda31`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: six medium; Grok: four high, one medium, two low.
- **Disposition:** applied without re-review; selector specificity beyond whole root and dark blocks is not modelled (a dark declaration holds the dark side)

---

## High

**1. A dropped token is still named by other values** — `tamber/src/import/engine.rs:168` and `:233`

`failed` removes the token, but `colour_of` still writes that token’s name for any reference that resolves on a side that does have a colour. Shadcn often has no winner on the broken side, so the dependent is not itself `failed`.

```css
:root { --x: white; --y: var(--x); }
.dark { --x: black; }
.theme-blue { --x: 1rem; }
```

`--x` is dropped because `.theme-blue` is not a colour. `--y` is only set on `:root`, resolves, and is emitted as `shadcn.y = shadcn.x`. A selector list (`:root, .theme-blue { --y: var(--x); }`) does the same on every target that can still see the good side. `check` then reports `tamber-unknown-token`.

A theme-only failure should drop that theme’s `when` line and must not fall back through `pair` onto another side. A failed default side should drop the token, then every value that names a dropped token, to a fixpoint (chains included).

**2. Inline redefinitions still colour own values and indirect chains** — `tamber/src/import/tailwind.rs:407`

The shadowed check applies only to a theme key whose whole value is `var(--that-inline-key)`. These still resolve against the inline key’s theme colour (the `.dark` redefinition is not in `decls`):

```css
@theme inline { --color-d: red; }
@theme { --color-e: green; }
.dark { --color-d: blue; --color-e: var(--color-d); }
```

Dark `--color-e` becomes `tw.d` / red. Runtime `--color-d` is blue.

```css
:root { --brand: var(--color-d); }
@theme { --color-button: var(--brand); }
```

`--brand` stays in `decls`, so `tw.button` is red on both sides.

A pure `@theme` chain of `var()` fails closed: the middle key is omitted and the outer key does not resolve, so it is dropped rather than painted red. The message there is the generic “no colour” line.

Treat a name as shadowed when its value reads a shadowed name, including own-block declarations, and walk that to a fixpoint. Keep the inline key’s own token. Do not let other declarations resolve through it.

**3. A backing variable declared in `@theme` ignores later `:root` / `.dark`** — `tamber/src/import/tailwind.rs:472`

Round 2 emits unnamed `@theme` variables as backing colours. The own-block guard still rejects every non-colour theme key as “a kind not built yet (stage 6)”, so the override is reported and not applied:

```css
@theme { --ink: red; --color-a: var(--ink); }
.dark { --ink: blue; }
```

`tw.a` is red on both sides. The dark side should be blue. A later `:root { --ink: blue; }` stays red the same way.

Limit that guard to `Kind::Pending`. Let `UNNAMED` backing names through as ordinary own declarations so each side resolves them.

**4. `@custom-variant dark` is a raw substring search** — `tamber/src/import/tailwind.rs:112`

`css.find("@custom-variant dark")` does not require the name to end, and it reads the original text (comments and strings included), case-sensitively, first hit only. `@variant dark` itself is matched case-insensitively.

- `@custom-variant dark-mode` (or `darker`) is taken as the definition of `dark`. If that is the only match, every real `@variant dark` is reported and skipped, including when `dark` was never redefined and still means the built-in dark side.
- `@custom-variant DARK (@media print)` is missed, so `@variant dark` is imported as the dark theme and print colours become the dark side.
- A comment or an earlier known definition hides a later real one. The block form `@custom-variant dark { ... }` stops at `{`, so the definition is empty and always “unknown”. Two spaces (`@custom-variant  dark`) miss.

Parse it the way `css_blocks` parses rules: strip comments, require a name boundary, case-fold, and use the last real `dark` definition. Compare the known selectors against that definition.

## Medium

**5. A later root write replaces an earlier `.dark` value** — `tamber/src/import/tailwind.rs:486`

Every light declaration, including the hoisted `@theme` row, writes the dark target. Last write wins, so the dark colour is lost in both of these:

```css
.dark { --color-a: blue; }
@theme { --color-a: red; }          /* both sides red */

@theme { --color-a: red; }
.dark { --color-a: blue; }
:root { --color-a: white; }         /* both sides white */
```

`.dark` after the theme, with no later `:root` for that name, still works, which is why the fixture passes. Tailwind’s dark variant also matches descendants of `.dark`, and those keep the `.dark` value by inheritance; a later `:root` rule does not clear it.

Write the dark target from a light or theme declaration only when that variable has no dark-origin declaration. Dark-origin rows keep source order among themselves. With no `.dark` row, a later `:root` still sets both sides, so `tw.a = blue` and `tw.early = red` stay as they are.

## Low

**6. The unmapped line names one failed block** — `tamber/src/import/engine.rs:233`

`failed` is a `BTreeMap`, so several bad targets collapse to the greatest key. The token is still dropped; the message can name `.dark` when `:root` failed too. Keep every failed target, or the one the winning declaration actually set.

**7. A dropped shadcn variable is still “the token it would become”** — `tamber/src/import/shadcn.rs:267`

The length / “not a colour” arm passes `token: Some(shadcn.x)` after `Unresolved` has refused to create `shadcn.x`. Pass `None` when the variable is in `failed`.

## Checked, no defect

- **Sort ties.** `sort_by_key` is stable. Only `@theme` rows share a sequence number, and each variable is pushed once, so equal keys do not reorder two writes of the same variable. Rows in one stylesheet block keep source order.
- **Counting.** Each source declaration is either one `mapped` fate or one unmapped row. Shadowed keys, resets, and `Unresolved` are not reported twice. `mapped + reported == source` holds for these paths, including a dropped token whose dependent is still mapped (finding 1 is a bad token, not a bad count).
- **Default `initial`, `initial` outside `@theme`, `none`, overlong lists, grid exceptions.** A default single-key `initial` yields before it can reset, matching Tailwind’s `add()` order. Outside `@theme`, `initial` stays and the failed side drops the token. `none` is replaced only as a whole ident. An empty `nested()` result with rules present is reported instead of read as `:root`. `--grid-column-*` / `--grid-row-*` keep the start and end keys, as in Tailwind’s `clearNamespace`.
