# Code review: exact-tamber crate, round 1, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `3053457b2862e5f1d5a9511b0468827b8591f7146ad4c8f579572e1ab359d6b1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 10 MAJOR, 8 MINOR; Grok 2 MAJOR, 6 MINOR).
- **Disposition:** all fixed in round 2 but two kept as declared limits: missing components (none) refused, not folded; color-mix needs in <space>

---

**MAJOR** — `color(srgb)` relative channels are scaled as `rgb()` 0–255. `tamber/src/fold.rs:661`

`color(from … srgb …)` and `rgb(from …)` share `MixSpace::Srgb`. Line 645 sets the `color()` percentage base to 1 (CSS Color 5: 1.0 is 100%), then line 661 multiplies every sRGB channel by 255 and line 679 divides the result by 255. Channel keywords round-trip. A literal, a percentage, or a number inside `calc()` is only divided.

Folded results:

| Source | Result | CSS Color 5 |
|---|---|---|
| `color(from #ff0000 srgb 1 0 0)` | `color(srgb 0.00392156862745098 0 0)` | `color(srgb 1 0 0)` |
| `color(from #ff0000 srgb 100% 0% 0%)` | same 1/255 red | `color(srgb 1 0 0)` |
| `color(from #ff0000 srgb r g b)` | `color(srgb 1 0 0)` | red |
| `color(from color(srgb 0 0 0 / 0.6) srgb alpha 0.6 0.6 / 0.9)` | channels `0.00235` | `color(srgb 0.6 0.6 0.6 / 0.9)` |
| `color(from #ffff00 srgb r g calc(b + 0.5))` | blue `0.00196` | blue `0.5` |

`color(display-p3 1 0 0)` stays `color(display-p3 1 0 0)`. `rgb(from #ff0000 1 0 0)` → 1/255 is the right `rgb()` scale.

**MAJOR** — A hue percentage resolves to 0°. `tamber/src/fold.rs:613`

The hue slots in the percentage table are `0.0` (`hsl`/`hwb` at 613–614, `lch` at 616, `oklch` at 618). CSS Color 5 types that channel as `<hue>` (`<number>` or `<angle>`): “90 is equivalent to 90deg”. A percentage is invalid there. The folder accepts it and stores hue 0, with no diagnostic.

`hsl(from red 50% 100% 50%)` folds to `color(srgb 1 0 0)`. `hsl(from red 180 100% 50%)` folds to cyan, so the number path is fine. `oklch(from #ff0000 l c 50%)` and `lch(from #ff0000 l c 50%)` store hue 0; the same forms with `180` store hue 180. `relative.json` has no percentage-hue case.

**MINOR** — A refused dial or token is reported again as unknown. `tamber/src/check.rs:249`

A dial error `continue`s before `dials.insert` (line 318). A stage-refused token `continue`s before `names.insert` (line 381; same pattern at 345, 353, 360). A later use is `tamber-unknown-token` or `tamber-kind`, as if the name had never been declared. A token that was inserted and then failed to fold is suppressed (empty diagnostic id). These names never enter the map, so that suppression does not run.

`tamber/tests/corpus/refuse-dials.err` records it: line 14 `far is no token, dial, role or CSS colour`, line 15 `far is not a number here`, beside the real range error on line 4. A stage-6 `w: length` referenced by a colour token adds the same false `tamber-unknown-token`. The design is still refused.

**MINOR** — A role used as a dial value gets the literal diagnostic. `tamber/src/check.rs:697`

`Literal::named` rejects every ident `exact_color::parse` does not know, including `-exact-label`, with “a dial's value is a literal”. The host-value arm at `check.rs:235` never sees a role. `currentcolor` does reach it. `refuse-dials.err:9` is that wording, and the dial is then omitted, so the theme line also says `host` is unknown.

**MINOR** — An `under scheme=dark` light side must match as CSS text. `tamber/src/check.rs:674`

`same` compares `Half::css()` strings, and `under_value` uses that at line 626. `#ffffffff` and `color(srgb 1 1 1)` are the same opaque white. Both directions (`#ffffff` default with a `color(srgb 1 1 1)` light side, and the reverse) are `tamber-under-light`. The selection rule itself — ignore `when`s on the under axis, keep other axes — matches `axes.json`.

**MINOR** — `when` conditions reject spaces around `=`. `tamber/src/syntax.rs:490`

Conditions are split on whitespace, then each word must contain `=`. `when density = compact` is `tamber-syntax`. A token line `a = #000` allows those spaces. RFC §3 says the structure lexer is Contract’s, where `=` is its own token. The commit’s lexer note covers indent, `//`, and blank lines.

**MINOR** — A bare exact role reference is case-sensitive. `tamber/src/check.rs:717`

The case-insensitive fallback runs only when the lowercased key is in the roles map. That map is keyed by the row spelling (`-exact-label`, `canvas`). With `label` overridden to `#ff0000`, `label` and `-exact-Label` resolve, and `Label` / `LABEL` are `tamber-unknown-token`. `CANVAS` resolves a `Canvas` override. An un-overridden bare `label` staying unknown is consistent with D3: the bare key is an override spelling, and the value spelling is `-exact-label`.

**MINOR** — `from` and channel keywords are case-sensitive. `tamber/src/fold.rs:278`

`rgb(FROM #ff0000 r g b)` is `tamber-kind` (“not a colour”) because line 278 compares `from` as written. `rgb(from #ff0000 R g b)` is `tamber-kind` (“`R` is not a number here”) because `Channels::get` at line 362 compares the keyword as written. CSS identifiers are ASCII case-insensitive. Function names are already lowercased.

## Checked and sound

Color-mix matches CSS Color 5 §3: omitted percentages become 50/50 or `100 − other`, either side outside 0–100 or a sum ≤ 0 is refused, `t = q/(p+q)` is the weight of the second colour, and the alpha multiplier is `min(1, (p+q)/100)` applied after the premultiplied mix. `mix.json` and the Bluesky faded mixes match that, including a percentage written before the colour. Storing an hsl/hwb mix as Wide sRGB is the right kernel limit (`Space` has no HSL). Requiring `in <space>` is the documented difference from the spec’s oklab default.

`contrast-color` is WCAG 2 relative luminance, white on a tie, alpha ignored. `#808080` correctly folds to black (black contrast about 5.32, white about 3.95). The spec leaves the picker UA-defined, and the winner always clears WCAG AA large text. That documented choice is fine.

Nested `light-dark` flattens to the light side of the first argument and the dark side of the second. Absolute legacy forms stay 8-bit; `color()`, lab/lch/oklab/oklch, every mix, and every relative colour stay Wide. Tuple order is first axis slowest, last fastest, capped at 64. Fallbacks are the tuple-0 clip, a role’s schema pair, or null for `currentcolor` and `platform-color`. The digest is FNV-1a 64 over the IR body starting at `"name"`, with `format` outside it. The corpus goldens match that writer.

These commit-message differences hold: the line lexer, roles and fallbacks from `Vocab`, `light-dark(role)` as `tamber-residual` until the kernel can store a role side, the under-axis light side taken while ignoring under `when`s, a token reference meaning that token’s same-tuple value, the reserved-word set, and the RFC example’s indented `radius.md` being a syntax error (`bluesky-rfc.err`).

Cycles are reported once. A dial that no folded token uses is `tamber-dial-inert`; `refuse-dial-inert.err` is right.

VERDICT: fix (MAJOR color(srgb) relative channels divided by 255, tamber/src/fold.rs:661; MAJOR hue percentage stored as 0°, tamber/src/fold.rs:613; MINOR refused dial/token reported again as unknown, tamber/src/check.rs:249; MINOR role dial value diagnosed as a literal, tamber/src/check.rs:697; MINOR under light-side match is CSS text, tamber/src/check.rs:674; MINOR when conditions reject spaces around =, tamber/src/syntax.rs:490; MINOR bare exact role reference is case-sensitive, tamber/src/check.rs:717; MINOR from and channel keywords are case-sensitive, tamber/src/fold.rs:278)
