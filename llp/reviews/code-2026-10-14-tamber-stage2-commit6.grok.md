# Code review: LLP 1121 stage 2, commit 6 (the web's app modes), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `f81b3d72b9b6e68eb22d3abff1483065cdfbdd8b464a155f75a1c334c669bb56`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: eight medium, one low; Grok: three high, one medium.
- **Disposition:** all fixed in the round-1 commit, except the accent-only boundary writing inherited rows, kept out to match the wasm host (and later refined)

---

## High

**1. A varying accent token cycles `--exact-accent` with its custom property.** `host/web-js/src/modes.rs:145` and `host/web-js/modes.js:65`

The first projection of `accent-color` bound to a token whose value is the accent role follows `host/web/src/css.rs:343`: it writes `accent-color: var(--exact-accent, AccentColor)` and leaves `--exact-accent` unset, so the role inherits. The sentinel pass binds that token to an opaque colour, so the second projection writes both `accent-color` and `--exact-accent`, and `substitute` turns both into `var(--x-<token>)`. The tuple rule still sets `--x-<token>: var(--exact-accent, …)`. Any element that has that `accent-color` is an accent boundary, so it also receives `data-x-modes`. On that element `--exact-accent: var(--x-<token>)` and `--x-<token>: var(--exact-accent)` cycle, and both properties become invalid.

The same write in `modes.js` copies channel 4 onto `--exact-accent` as well as `accent-color`. Copying `var(--exact-accent, AccentColor)` or `var(--x-<token>)` onto a boundary makes the same cycle or a self-reference, so the boundary drops an authored accent inherited from above.

This is the authoring `accent-color="accent.text"` when that token varies and names the accent. The conformance plan uses `accent.text` as `color` under a literal accent, and `an_accent_naming_token_makes_an_authored_accent_a_boundary` only checks `data-x-b="127"`.

Fix: keep the first projection’s decision. If the substituted `--exact-accent` is `var(--x-<token>)` and any tuple of that token contains `--exact-accent`, omit `--exact-accent` from the class. For tuples whose value is a real colour, set `--exact-accent` to that colour from the tuple rule on the class that actually sets this accent (`cN[data-x-modes="…"]`), so a concrete tuple still publishes the accent and the role tuple inherits. In `apply`, set `accent-color` from the ancestor text and call `removeProperty("--exact-accent")` when that text contains `var(--exact-accent` or `var(--x-`. Add a test with `accent-color` bound to a varying accent token, above and under a `color-scheme` boundary.

**2. A dynamic inherited colour on a boundary is treated as always set.** `host/web-js/src/modes.rs:187` and `host/web-js/modes.js:61`

`own` is set when the node has the style binding, including a dynamic one that is absent from the class CSS. `apply` then skips that channel for good. The runner (`runner/src/instance/theme.rs:274`) re-specifies an inherited colour whenever `themed.sets` is false, which is the unset binding. A pin or `color-scheme` node with `color={flag ? "#fff" : null}` therefore keeps the parent’s already computed colour while the binding is null, instead of the ancestor token in the boundary’s own tuple. A Dim pin under a page `color="text.primary"` shows the page’s colour. `Sx` does record unset as `$xi[j] === undefined`, and it runs before the microtask `apply`, but the static bit ignores that.

Fix: set a `data-x-b` bit only for a channel the class CSS declares. In `apply`, also skip a channel while `b.$xi?.[j] !== undefined` (the binding is currently set, so the `Sx` write must stand). When the bit is clear and `$xi[j]` is undefined, write the implied row from `channel(parent)`. An accent-only node still has every bit set and still writes no row. Add a test with a nullable bound colour on a pin and on a `color-scheme` boundary, including the transition from a set colour back to null.

**3. The first mode pass strips the inline `var(--x-…)` that a symbol’s clones need.** `host/web-js/modes.js:48` and `host/web-js/src/emit.rs:1042`

Symbol nodes (and their descendants) are emitted with the whole class CSS inline, because Chrome paints `<use>` clones from the element’s own attributes and not from the page’s class rules. The first `apply` treats every element of a class in `Vars` as adopted markup and `removeProperty`s each property whose value contains `var(--x-`. That removes the load-bearing inline `var()` from symbols already in the tree. `Adopted` stays true, so the colour does not come back. Elements created after that pass keep the inline value.

Fix: leave an inline value that is already `var(--x-…)` in place. Where the inline value is a renderer-baked colour, set it to the class’s `var(--x-…)` text on symbols instead of removing it. Ordinary elements can still drop a baked colour so the stylesheet `var()` wins. Add a test that a symbol whose `fill` or `color` is a varying token still has that `var(--x-…)` inline after `apply`.

## Medium

**4. The accent companion write clears the token bit it just recorded.** `host/web-js/src/rows.rs:360` and `host/web-js/modes.js:92`

Both `accent-color` and `--exact-accent` are recorded on channel 4. For a dynamic accent whose mapped value is exactly `var(--exact-accent, AccentColor)` (a non-varying token or role that lowers to the accent role), the first `Sx` stores the text index and the second map returns null, so `Sx` assigns `e.$xi[4] = undefined`. A `color-scheme` boundary below then sees no token and does not re-specify accent. The card keeps the parent’s computed accent. The wasm host writes `accent-color: var(--exact-accent, AccentColor)` on that boundary, so the system accent follows the card’s scheme. Varying tokens miss this path because `theme_map` has already replaced the name with `var(--x-…)`.

Fix: pass channel 4 only for the `accent-color` write. Emit the `--exact-accent` companion as a plain `S` / `Sm` / `Sr`. Add a test with a dynamic accent bound to a token whose value is the accent role, and a Light card under a dark page.

## Low

Nothing at low.

## Checked and sound

No render-host throw. `render.mjs` sets `__exactRender` before the module runs; `schedule` and `modes.set` return before `apply`, and `apply` is the only caller of `getElementsByClassName` and `closest`, which the fake DOM does not implement.

Apps without modes do not load the runtime. `modes.js` is imported only when `Modes::new` returns `Some`, which is axes, or a theme plus a `color-scheme` binding. `a_plan_with_no_modes_loads_none` covers that.

`prefer mode` checks every pin before page or fold facts, then sets. `setMode` takes exactly two strings and journals the runner’s refusal sentence (`runner/src/runner/mode.rs:108`). An invalid `-exact-mode` stays a boundary and keeps the surrounding tuple, as `bridge::pinned` does. The wasm glue’s extra `/[,=]/` rejection and `mode: {Debug}` prefix are the comma/equals wire format, not a JS divergence.

Non-colour tokens are not a current defect. The checker refuses them with `tamber-stage` until stage 6, so a plan this emitter sees has only colour slices. `rule`’s `transparent` fallback for a value that does not parse as a colour would be wrong for those later tokens.
