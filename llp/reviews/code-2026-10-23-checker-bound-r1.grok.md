# Code review: the design checker's bound (rt1's PRIORITY finding), round 1 of 3, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `bbdda3f86a`.
- **Method:** one brief (sha256 `04b7242e627b089e6b63a37fb3b51dbc640786f62df8ab925d596c0e5520a086`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at bbdda3f86a, before the rebase onto exact47's main; that commit is a06ea011dc there.

The byte cap, the depth cap, and the per-function residual cap are in shared code, and a `clamp` chain is cut off at each function. Two paths still miss those caps.

## BLOCKER

**A number or length dial past the cap is reported by echoing the value — `tamber/src/check.rs:569`**

`Checker::dials` parses a number or length dial with `number` (`check.rs:1353`), which turns `css::parse` into `Option` and drops the error. When the value is over 4096 bytes, `over_limit` fails the parse, `number` returns `None`, and the branch at `check.rs:566` reports `tamber-dial` with `` `{d.value.value}` `` interpolated. The same happens for a range end at `check.rs:607` (`` `in {lo}..{hi}` ``). A colour dial in the same function keeps `syntax()` and the `over_limit` text (`check.rs:532`). `edit::valid` refuses a wire `set-dial` as `tamber-value-size` with no text (`edit.rs:285`). `contract build` and a server load of the same file both go through `Checker::dials`, so they echo the value and use a different id and sentence than the wire refusal.

**One `calc`/`min`/`max` can assemble every stored residual before the 256-atom check — `tamber/src/measure.rs:378`**

`func` runs `func_unbounded` to completion and only then rejects `Quantity::Base` whose `atoms()` exceed `MAX_RESIDUAL_ATOMS`. `sum` (`measure.rs:471`) `plus`es every operand first. `Residual::add` (`residual.rs:86`) finds a like term with a linear structural `==`, and `pick` (`residual.rs:123`) does the same with `rest.contains`. A stored length may already hold 256 atoms: 128 distinct `min(b, Npx)` terms fit in one value and pass the check (`256` is allowed). There are only nine base names, but those `min` atoms do not compare equal across different px constants, so they do not merge. A later value `calc(w0+w1+…)` of a few hundred such tokens — each name is a few bytes, so this fits in 4096 — builds on the order of `references × 128` terms and does a quadratic number of those comparisons before `func` returns the refusal. That is seconds to around a minute on one check. Each `wN` is itself legal, so a client can commit them and stall the server on the summing write. Nested `clamp(prev, prev, prev)` is bounded; this wide sum is not.

## SHOULD

**`-exact-platform-color` re-parses a slice whose parens the depth scan never counted — `tamber/src/css.rs:312`**

`over_limit` (`css.rs:49`) and `Reader` toggle on every `"` and ignore backslashes. Bare `(` groups and function calls match that count, so one `Reader` stays at 32. `platform` then splits arguments with `lex::split_top` (`lex.rs:102`), which keeps a quoted `\"` inside the string. The sequence `"\""` leaves `over_limit` inside a quote and `split_top` outside one, so the fallback after the comma is parsed by a fresh `parse` while those `(` bytes were invisible to the outer scan. Each nested platform adds a live `call` frame, and the innermost `parse` may use its own 32. The 4096-byte cap keeps this near 150 frames, short of a stack overflow, but the value is accepted with more than 32 nested calls. On the plain reader, a `\` before `(` is the other way: the scan counts the paren, and `item` errors at `\` without calling `call`.

**A box-shadow comma split can report a depth refusal as `tamber-syntax` — `tamber/src/shadow.rs:219`**

`fold_kind` runs `over_limit` on the whole text (`check.rs:1151`). `shadow::split` (`shadow.rs:126`) cuts on commas and does not treat quotes, then each piece is `css::parse`d again. A value such as `0px 0px "a, calc(calc(…))"` has its parens inside a quote for the outer scan, so the whole value passes. The second piece, parsed alone, is over 32 deep. `parse` refuses before it recurses, and `shadow::fold` labels that error `tamber-syntax`. Build and server share that id. `text-shadow` returns on a comma before this parse (`shadow.rs:211`).

**Alias chains still recurse on the call stack — `tamber/src/check.rs:1010`**

`value` pushes the token and folds it, and `Scope::named` (`check.rs:1435`) calls `value` again for the token it names. Cycle detection is a `Vec` scan, not a depth cap. A few thousand `tN = tN+1` lines overflow the stack. Each value is a short name, so none of the new caps apply. A chain that size fits under the 1 MiB request door (`tamber_serve.rs:80`, `scripts/tamber-server.mjs:111`). Token count is explicitly left to those doors (`QUEUE.md`), and this failure is a crash during one check.

**`bounded` measures the unmapped report string, which Astryx synthesizes — `tamber/src/import/mod.rs:770`**

Astryx sets `Unmapped.value` to `light | dark` when the two sides differ (`import/astryx.rs:171`). Each side can be within 4096 and already accepted by `source_bound` (`astryx.rs:150`). The joined report is past 4096, and `bounded` then fails the whole import with `tamber-value-size`. Tailwind and shadcn store the source value, which `source_read` already bounded.

**Pencil's pre-check walks every string under `value`, including theme labels — `tamber/src/import/dtcg/pen.rs:784`**

`json_strings` collects every string in the variable's `value`. An entry list is `{ "value", "theme" }`, so a theme label past 4096 bytes fails the entire import with `tamber-value-size`. The 1024-byte literal rule (`pen.rs:662`) only looks at `value.as_str()` and still reports a scalar in `(1024, 4096]` with depth ≤ 32. `import_pen.rs` was not updated; the new hard refusal is only the one colour string in `tamber/tests/it/limits.rs`.

## NIT

**The CSS importer's size sentence drops the actual length — `tamber/src/import/mod.rs:834`**

`source_read` says `` `{name}`: a value is at most 4096 bytes ``. `over_limit` also says `this one is N`. The id matches. Depth on a copied declaration still goes through `source_bound`.

**Metadata over 4096 bytes is refused as a name — `tamber/src/edit.rs:383`**

`long` (`edit.rs:303`) is shared with names, params, and axis idents. The sentence is always `a name is at most 4096 bytes`. The bytes are not echoed.

## What holds
