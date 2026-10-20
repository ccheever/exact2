# Code review: held :hover and :focus-visible looks editable, the source map's state parts (LLP 1122 queue item (1)), round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `b6e0727700`.
- **Method:** one brief (sha256 `205d9146d6c27e2f25833e1e8dffc68b240253aa47447f10f6b4f117e97a2fdc`), shared by both reviewers, each blind to the other's review; commits d76b990cb7 (c1), 709c0bb8bc (c2) and b6e0727700 (round 1's fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 SHOULD-FIX, 3 NIT).
- **Disposition:** all taken post-round, in the commit named in the sign-off request:
  - (1) as Astra's 2.
  - (2) docs/tamber-protocol.md: `states` entries' `at` is absent for a conditional class's part, as is an `:active` row's; the border rule as Astra's 3.
  - (3) the join omits `at` for a part with no place rather than publishing `at: null`; tested.
  - (4) as Astra's 1.

Nothing blocks. The three round-1 fixes hold, and the reorder, `writtenAs`, plan-byte, and `class_spans` checks do not regress. One reader of a conditional-class `:active` part drops `state`.

Round 1, as the records describe it:

- A border side is named from the source at its part. `writtenAs` (`scripts/tamber-inspect.mjs:187`) returns `border-color` or `border-{side}-color` only when the text at that column starts with that name, and null otherwise, so `stateRung` offers no edit (`:214`). A shorthand still names `border-color` for all four sides; a later or sole `border-top-color` names that longhand; a place that says neither is not editable. `set-state` then addresses the attribute that is actually there.
- A look’s class-or-own origin is `class_spans` (`contract/lower/src/nodes.rs:66`, used at `:775`), the same test as a written base row (`:532`). Those spans are the class attributes’ own spans, taken before animation composition and list-row insertion. State-part attributes are `written: true` (`contract/syntax/src/parser.rs:732` and `:1202`) and the look keeps that span (`contract/lower/src/states.rs:450`, `..row.clone()`). A class hover stays `class:Pulse`; the node’s own hover stays `own`. `a_look_row_beside_a_composed_animation_is_still_its_nodes` covers that case. A grouped list only prepends rows, so it cannot move a span out of the set.
- A conditional class part omits `at` for every state, including `:active` (`contract/cli/src/map.rs:321`). The held row’s `source` falls back to the node (`scripts/tamber-inspect.mjs:423`). `stateRung` and `rowRung` return null without a file (`:204`, `:250`), and `editable` refuses `class:A/B` (`:352`). `docs/reference.md:198` says so.

These do not regress:

- `rowRung` (`scripts/tamber-inspect.mjs:249`) still returns null for a base row with no `attr` before any read (`:256`), still reports a layout row from `b.attr` before opening the file (`:258`), still returns the package reason before the read (`:259`), and still returns null on an unreadable file before `writtenAs` (`:262`). A style state part still bails at `:253`, so it stays on `stateRung`.
- Non-side names, including `:active` press rows, never read the line: `writtenAs` returns `authored()` immediately (`:188`). `press_color` is still `color`, `press_opacity_to` is still `opacity`.
- With no map requested, the `continue` is after `self.attr` (`contract/lower/src/nodes.rs:771`), so the look bindings and `set_looks` inputs are unchanged. `origins.resize(bindings.len())` still runs for every attribute when a map is asked for (`:780`), and `style_rows` is called only then (`:789`). Plan bytes were not re-hashed; the plan path does not read `origins`.

## Findings

1. **SHOULD-FIX** — A conditional-class `:active` part keeps the node’s place but loses `state`. `contract/cli/src/tamber_uses.rs:396` and `scripts/agent-inspect.mjs:117`.

   `class=(flag ? A : B)` with both styles writing the same literal `:active color="accent"` compiles (identical literals stay that literal, `contract/lower/src/class.rs:79`; the span is style A’s). The map entry is `{state: "active", origin: "class:A/B"}` with no `at`. Before this fix, `uses` named A’s line and set `state`. The new `.filter(|p| p.get("at").is_some())` (`tamber_uses.rs:399`) wraps both the place copy and `at["state"] = …` (`:404`). Looks repair that afterward (`:411`, `state` is set even with no `at`). `:active` is not a look, so the site stays at the component’s line and column with row `press_color` and no `state`. The comment at `:409` says a part with no place keeps the node’s place “in its state”; that is true for `:hover` and `:focus-visible` only. `docs/tamber-protocol.md:508` says an `:active` read carries `state`.

   `agent-inspect` does the same split. `attach` accepts the binding (a part needs no `at`, `:106`) and sets `style.origin` to `class:A/B` (`:119`), then assigns `state` and `at` only when `placed()` (`:122`). A press-only conditional-class row is reported as that class with no state. The node stays `compatible`. This is not a wrong write: the rung is still refused, and the place is the node rather than style A. A client that treats `state` as what makes the site the press part (`tamber-protocol.md:508`) will not see it. The new tests check the missing map key and `editable.rung === null` (`scripts/tamber-inspect.test.mjs:264`, `:280`), not this field.

2. **NIT** — `docs/tamber-protocol.md` still describes the pre-fix wire. The fix commit updated `docs/reference.md` only.

   `:116` still says every `states` entry is `{state, origin, at}`. `:1408` still says every `:active` row carries `at` as a place; the conditional-class exception at `:1415` is about base rows. `:1425` still says a border side is edited “as the `border-color` its part writes whole,” which is the round-1 blocker. The rung’s `row` is now the longhand when that is what the part writes, and the in-repo inspector sends that `row` (`apps/design-tool/inspector.ts:64`). A client that hardcodes `border-color` from `:1425` would change all four sides again.

3. **NIT** — The selection join publishes `at: null` for that same `:active` part. `scripts/tamber-inspect.mjs:436`.

   `part` is `{ state: b.state, at: b.at ?? null }` whenever the row is a state part. For `press_color` under `class:A/B`, the selection row has `state: "active"` and `at: null`. The map omits the key (`contract/cli/src/map.rs:321`); a conditional base row omits it too (`:435`, `written` is `{}`). The edit is still the conditional-class refusal, and the in-repo inspector reads `source` and `editable`, not `row.at`. A reader that treats a present `at` as a place and opens `at.file` throws.

4. **NIT** — `writtenAs` slices a JavaScript string, while map columns are UTF-8 bytes. `scripts/tamber-inspect.mjs:190`; `plainAt` uses the byte column (`docs/tamber-protocol.md:1398`).

   One line, `:hover background-color="#café" border-top-color="#111111"`. `é` is two UTF-8 bytes and one UTF-16 unit, so `slice(at.col - 1)` starts inside `border-top-color`. The prefix is not `border-color` or `border-top-color`, `writtenAs` returns null, and the side is not editable. The byte index is never smaller than the UTF-16 index, so the read cannot start early and name a different property. One attribute per line, the layout the tests use, is unaffected.
