# Code review: LLP 1121 stage 2, commit 2 (the compiler reads designs), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `7aa7231a51f8e27dc8544c3eb36cbb31ec12d2bb8872ffe90723af392ee45d3d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 1's commit (d4ed14f26)

---

[P1] Profile stand-ins are folded into view expressions — `contract/lower/src/theme.rs:149`

`stand_in_profiles` replaces a whole `color(--profile)` value with a hex (`#00000101` for the first such token) and refuses another token that names that token inside an expression. Synthetic tokens are pushed afterward, so a row expression such as `light-dark(brand, #000)` or `color-mix(in srgb, brand 50%, #fff)` is folded against the stand-in. The restore at line 177 only swaps a value back when its folded css equals the stand-in exactly, so the mix stays the stand-in mix and both hosts bake it. The same restore also rewrites any other folded colour whose css equals a stand-in, and the first profile token’s stand-in is always `#00000101`.

[P1] A bound colour-function string resolves only in the kernel — `kernel/src/style/theme.rs:92`

A literal colour function over tokens is kept under its whole text, and `resolved` now treats any component containing `(` as that name. A ternary or `match` arm that repeats the literal is still that string at run time: `set_plan_style` binds the plan table and returns the folded colour. The JS target’s `theme_map` (`host/web-js/src/style.rs:457`) copies each colour function through with `v.slice(m.index, j)` and never reads that key, so the browser keeps `light-dark(brand, card)` and the property does not paint. Static literals are baked before emit, which is why the existing `light-dark(brand, #000)` fixture still passes: that string is not an entry of that plan.

[P2] `app.tamber` refusals and one-scheme warnings are pinned at 0:0 — `contract/lower/src/theme.rs:366`

Crate diagnostics go through `design_refusal`, which puts `path:line:col` in the message. `token_span` for a file is `Span::default()`, and `lower-theme-name`, `lower-theme-tint`, and `lower-theme-value` use that span with no path in the message. `theme_decl` also stores every design entry at `Span::default()`, so a one-scheme warning on a role override in `app.tamber` points at the start of the contract.

The exact-text match holds for a colour row, a gradient stop, a shadow colour, and a border or column-rule word: those splits keep a parenthesized function intact and pass that slice to `parse_light_dark`, which only trims the ends. The synthetic test’s `linear-gradient(light-dark(brand, card), #ffffff)` compiles, so that stop matches. Retention keeps every literal name (including names inside those functions), every role override, and every token once any colour attribute is computed; inlined component bodies are on the expanded root, and `fn` / derive / state literals are scanned. Keyframe colours still go through motion’s parser, which does not read the theme table, so a token there fails the build rather than being dropped for later. `resolved`’s new branch clones the `Arc` and does not allocate; the alloc test passed. An app with no design still stores an empty theme. Inline-theme tests passed. Expose, `apps/llp`, `apps/markdown`, and `apps/markdown-stress` were not compiled.

No test covers a profile token inside a view expression, a bound synthetic on the JS target, or an `app.tamber` `lower-theme-*` location.

VERDICT: fix (MAJOR contract/lower/src/theme.rs:149, MAJOR kernel/src/style/theme.rs:92, MINOR contract/lower/src/theme.rs:366)
