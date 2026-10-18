# Code review: LLP 1122, stage 6 kinds in the tool, step 3 (typed fields by kind), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, a throwaway worktree at `69ea44a2b`.
- **Method:** one brief (sha256 `74a8aacc917645e431ee8bd95d756de5ba1f5c2cef977fa49675143e2a577f64`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 high, 3 medium).
- **Disposition:** taken, as in the Astra record of this round, plus:
  - a text-shadow list gives no fields (the server refuses one);
  - an emptied required length drafts nothing rather than writing `0`, so nothing is written that the person didn't type.
  - Not taken: refusing fields for a `rem` literal or a non-px length token in a shadow. The word is still that shadow's length, the field is labelled right, and the server's refusal names it.

The typed editors round-trip the fixture’s whitespace, quotes, commas, and `light-dark()`, and a px length token or dial is not swapped with a colour token. Several values the server accepts are split into the wrong field, and clearing a required shadow length moves the later ones.

**High** — A text-shadow blur written with `+` or an exponent is the colour field. `apps/design-tool/parts.ts:12`

`1px 2px +3px` is a text shadow the server folds as x = 1, y = 2, blur = 3 (`css.rs` accepts a leading `+` and an exponent; unitless `+0` and `0e0` are zero lengths). `pxLength` only allows an optional minus, so `+3px` is the one colour and blur is an empty slot inserted in front of it. Changing that colour to `red` writes `1px 2px red`, which has no blur. The same split happens for `1e2px`, `+0`, and `0e0`. A literal `1rem` lands in the colour field too, while `shadow.rs` rejects it as a relative length. A length token or dial that is not px is filed as a length (`kinds.get(w) === 'length'` in `app.ts:785`, dials included) even when the server rejects rem, em, and named lengths inside a shadow. Px length tokens, px dials, and colour tokens such as `surface.inset` are classified on the right side of that line.

Fix: match `shadow.rs::length` — CSS px with sign and exponent, unitless zero, and a length token or dial whose folded value is px. Return no typed fields for a rem/em literal or a non-px length token.

**High** — Clearing blur on a four-length shadow makes the spread the blur. `apps/design-tool/parts.ts:107`

Blur is non-optional when a spread is present so it keeps its slot, but an emptied non-optional part still joins as no word (`parts.ts:46`). Clearing blur on `0 1px 2px 4px red` produces `0 1px  4px red`, which the server accepts as blur = 4px and no spread. Clearing y slides the same way: `0  2px 4px red` is y = 2px, blur = 4px.

Fix: join an emptied required length as `0` (a length the server accepts) so later lengths stay in their slots.

**High** — A whole-number weight the server accepts is edited as the size. `apps/design-tool/parts.ts:150`

`typeface.rs` takes a weight when `parse::<f64>()` is a whole number from 1 to 1000, and `explain` returns the source text. `600.0 16px`, `+600 16px`, and `600e0 1rem` fold as weight then size. The editor’s `/^\d+$/` test leaves those words as the size and the real size as the family. Setting family on `600.0 16px` to `Inter` writes `600.0 Inter`, which the server reads as weight 600 and size `Inter`.

`bold`, a weight before the size, a named text style as the size (`-exact-title3 / 1.2`), and a family stack with quotes and commas all join back byte for byte. Those paths match the server.

Fix: use `fold`’s weight test (optional sign, a whole number in range, including `600.0` and an exponent) and keep that word in the weight part.

**High** — A shadow that still names a colour token is shown as pinned. `apps/design-tool/app.contract:1138`

The follow gate now also requires `not typed.ok` (`app.contract:1122`). `shadow.ring` is `0px 1px 2px #00000033, inset 0px 0px 0px 1px surface.inset`. Explain names `surface.inset`, so the branch is an expression, and the value also has typed fields. The else branch shows “Pinned here: no longer follows surface.inset” and drops the links to that token. The source still names it. A type whose family is a token gets the same sentence.

Fix: show that sentence only when `pinning` is true. Keep the typed fields. A follows note can sit above them.

**Medium** — Italic after a weight, and `normal`, are offered as fields. `apps/design-tool/parts.ts:149`

`fold` accepts `italic` only while the weight is still unset, and it rejects `normal` and `oblique`. `600 italic 16px` and `bold italic 16px` are parsed here as style plus size; the server stops at `italic` and tries to read it as the size. `normal 16px` becomes size `normal` and family `16px`. `scripts/design-tool.test.mjs:1097` requires `600 italic 16px` to parse, so the suite locks that in.

Fix: accept `italic` only before a weight, return null for `normal` and `oblique`, and drop that case from the round-trip list.

**Medium** — `partEdit` rebuilds the line from parts that can still be the previous value. `apps/design-tool/app.contract:708`

`typed` is a resource of `shownValue`. Until `loadParts` returns, the previous parts stay on screen, and `typed.of` (`app.ts:786`) is never compared with `shownValue`. Changing amount on `12px` to `16`, then the unit to `rem` before that reply, composes `rem` onto amount `12` and sets `editValue` to `12rem`. The contract test clocks data between those two edits, so it does not catch this. The raw line updates immediately and the typed fields do not.

Fix: keep parts in state once `typed.of == shownValue`, and have `partEdit` map that state. Ignore a part edit while `typed.of` is behind the value on screen.

**Medium** — A text-shadow list gets one editor per shadow. `apps/design-tool/parts.ts:85`

The server rejects more than one text shadow. `1px 1px red, 0 0 blue` gets shadow 1 and shadow 2 fields. Editing shadow 2’s x writes a second shadow the server will not read.

Fix: return null when a text shadow has more than one item.

`scopeOwn` uses the same `branch.own` branch as the raw field, and `draftStale` still suppresses a stale draft. Scrub follows `editValue`, so a typed edit previews through the existing scrub. Aria labels are the token plus the part label; the multi-shadow labels are what the unit test lists. None of those three are covered by a test. `stub/dials.json` matches `dials.tamber` (the file digest is `09a10c1a…`), including `space.edge`, `shadow.ring` folded per density through `surface.inset`, `type.title`, `opacity.muted`, and the contrast warning moved to line 43.

Verdict: the happy-path join and the one-line `set-token` writes hold, but the shadow and type splitters disagree with the server on values it accepts, and `shadow.ring` is labelled pinned while it still follows `surface.inset`.
