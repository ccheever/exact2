# Code review: LLP 1121 stage 2, commit 2, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `49a36ff0b7a996be8886e8408e88054d2ce6a634563422658b5c27f04dae48a4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 2's commit (a3bc65d2f)

---

**[P1] Take a profile colour out of the fold when it has a `when` — `contract/lower/src/theme.rs:335`**

`profiles` only pulls a token whose default is `color(--…)` when `whens` is empty. A design whose default tuple is the profile colour and which also varies on an axis stays in the design:

```
ink = color(--brand 0.1 0.2 0.3 0.4)
  when dark-style=dim: #112233
other = #abcdef
```

`check_design` then rejects `` `--brand` is not a `color()` space `` (`tamber-syntax`). `declare_theme` returns an empty theme, so `other` is dropped with it. The previous stand-in rewrote that default regardless of `when`, and stage 2 still publishes only tuple 0. An alias that merely mentions the profile under `when` does hit the new `lower-theme-value` refusal.

**[P1] Match a profile token under every spelling the crate already accepts — `contract/lower/src/theme.rs:337`**

Extraction, the later word scan (`theme.rs:356`), and the synthetic filter (`theme.rs:157`) all require the authored string (`p.name == text` / `p.name == w`). The crate, once the profile token has been removed, still resolves the other spellings and any ASCII case to that role. These compile and bind the role's own name:

- `Canvas = color(--brand …)` / `ink = canvas` → plan value of `ink` is `Canvas`, while `Canvas` stays `color(--brand …)`
- `secondary-label = color(--brand …)` / `text = -exact-secondary-label` → `text` is `-exact-secondary-label` (inline `theme` and `app.tamber`)

A row `color="ink"` therefore parses the platform colour. The inner parse does not re-enter the theme, so it misses the override. `color-mix(in srgb, canvas 50%, #fff)` against a token named `Canvas` fails as `tamber-residual` over the host role `Canvas` instead of `lower-theme-value`. An exact alias (`text = ink`) still copies the profile colour through.

**[P2] Visit literals on an expanded action call — `contract/lower/src/theme/scan.rs:275`**

`stmts` walks `Let`, `Assign`, `Command`, `Send`, `If`, and `Match`, and the `_` arm skips `Stmt::Call`. After expansion that call is where an action-to-action argument lives (`args`, and the parameter `let`s in `body`). `declare_theme` runs on the expanded root, so this compiles with no synthetic:

```
action paint(c: string)
  colour = c
action go()
  paint("light-dark(brand, card)")
```

A direct `colour = "light-dark(brand, card)"` in the action does become the synthetic and folds. `Expr::Call` on an attribute is scanned.

## Assessment

The re-check loop terminates, and the `retain` counter is the original index. It continues only when every diagnostic is a loose synthetic, and `check_design` reports at least one of those, so each pass removes something. A loose `light-dark(brand, CanvasText)` beside a loose `light-dark(brand, card)` drops the residual and keeps the one that folds, renamed from the compacted vec.

Adding `.` to the JS identifier class does not rewrite other bound values. `.` is literal inside the class; `url()`, quotes, comments, and `#hex` are earlier alternatives; a miss copies the match through. `1.5px`, `0.5`, `.5`, `-0.5`, `50%`, hex, comments, and `url(...)` stay as written, including when a token is named `1`. `surface.card` is one lookup. A dotted word such as `brand.5` is no longer split, which matches a whole component.

The manifest key, the design source id, and `?:` branch spans are consistent with the checks that read them.

VERDICT: fix (MAJOR contract/lower/src/theme.rs:335, MAJOR contract/lower/src/theme.rs:337, MINOR contract/lower/src/theme/scan.rs:275)
