# Code review: contract gallery: a symbol's prop gets a role and a handler's action prop an inert action of its payload (stage 13 C), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `92c9a0fbbe`.
- **Method:** one brief (sha256 `294583b2d57c42e9e60e312708387c86e342c431ed61e95e9cc9c6b9565f5be1`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken: the stem; an inert action only where every bare use passes the same types; any payload type declared, its name spelled from the types; tests widened

**VERDICT: WRONG**

The ListRow and SwitchRow path is right: an exact `` `symbol:${symbol}` `` becomes `back`, a checkbox `change=set` becomes `noop_bool(checked: bool)`, a text `input` becomes `noop_string(value: string)`, and `press` stays `noop`. App plans are untouched. Three holes in the new walk still refuse cells the compiler would place.

1. **should-fix** — A symbol stem is rewritten as a whole role. `contract/cli/src/gallery/inert.rs:102`

The match allows trailing template parts (`..`). `` image `symbol:${chevron}-left` `` matches. `chevron` is not a role, so `role` substitutes `SYMBOL_ROLES[0]` (`back`). Inlining folds that to `symbol:back-left`, which `symbol_role` rejects, and the cell is refused. The ordinary string placeholder is the prop's own name, and `symbol:chevron-left` is an alias of `back`. The same rewrite breaks `check` plus `-circle-outline` (`symbol_role("check")` is `checkmark`, and `checkmark-circle-outline` is not a spelling) and `volume` plus `-up` (`volume-up` is `speaker`; `back-up` is not). `` `symbol:${star}-fill` `` still works only because `star` is itself a role, so the substitution stays `star` and `star-fill` is a role. `back-fill` is not.

Require the prop to be the entire remainder. One prop is one value, so substitute a role only when every `symbol:` use of it is exactly `symbol:` plus that prop. `symbol_role(prop)` is the right test that its own name names one: it accepts the role, an Apple spelling, and an alias, and the canonical role it returns is a valid `symbol:` source. ListRow's `` `symbol:${symbol}` `` has no trailing part and stays on `back`.

2. **should-fix** — The first non-empty payload wins, and a zero-payload use is invisible. `contract/cli/src/gallery/inert.rs:76`

`press=go` and `focus=go` have an empty parameter list, so they never set `found` and never disagree with a later `navigate=go`, whose payload is `(location, string)`. The arity graph intersects those uses to 0 parameters: `navigate` is 0..=1 with no record, `press` is 0..=1 ending in `MouseEvent`, and the only count they agree on is 0. `noop` places the cell. The gallery declares `noop_string(location: string)`, and `press` rejects a trailing `string`.

Falling back to `noop` on any second handler is too coarse. A text field's `input=edit` and `navigate=edit` both take one string; the intersection is exactly 1, and `noop` would refuse the field. Keep a signature only when every bare use accepts it. Two required payloads of different types (a checkbox's `bool` and a field's `string`) already share no action that typechecks, so the cell is refused either way.

A `for` item is `Ref::Item` and a `match` binding is `Ref::Bound`. `check_handler` rejects both with `analyze-unknown-action`, so a component that passes the shadowed name to a handler does not compile. The walk's ignorance of scope does not refuse a cell the compiler would place.

A non-handler attribute produces no payload. Every non-empty row of `payload` is a handler name. `aria-label=set` yields an empty list and is ignored. `resize=<ident>` is a handler (`is_handler`); CSS `resize` is a string.

A handler on a `Use` is skipped. `each` at line 123 walks a `Use`'s children and not its arguments, so `Child(set=set)` stays `noop` and the compiler decides. Slot children are still walked; those elements are this component's own view.

3. **should-fix** — Spellable payloads outside `string` / `number` / `bool` are dropped, and the action name cannot carry them. `contract/cli/src/gallery/inert.rs:83` and `:32`

`handler_parameters` is the right list: `payload` without the prose, and without the optional event record, which an action may leave off.

- `scroll` is `(scrollLeft: number, scrollTop: number)`. Arity is 2..=3 (`ScrollEvent` optional); two numbers are accepted.
- `transformgeometry` is `(bw: number, bh: number, pw: number, ph: number)`. Arity is 4, all numbers, matching the type check.
- `reorderdrop` is `(item: string, before: option<string>)` (`contract/types/src/payload.rs:45`). Analysis requires exactly those two types, and `action receive(item: string, before: option<string>)` is the spelling already used. The filter rejects `option<string>`, `handler` returns `None`, the prop stays `noop`, and the cell is refused (arity minimum 2).

The same filter drops `files: Picked[]`, `selection: MarkdownSelection`, and `event: InputEvent`. Those are known types in any file. `MarkdownSelection` is a legal identifier. `Picked[]` and `option<string>` are not, so they cannot be spliced into the action name.

The names that are declared (`checked`, `scrollLeft`, `currentTime`, `bw`, and the rest) are ASCII identifiers, none of the sixteen reserved words, and they are not inserted into `taken`. They cannot rename a shape or a component.

Widening the filter without changing `unique(&format!("noop_{}", types.join("_")))` emits `action noop_string_option<string>(...)`. That is a syntax error on `Gallery`, and `generate` then fails the whole gallery rather than refusing one cell. Sanitize the name (`noop_string_option_string`) and keep the type text only in the parameter list.

4. **nit** — The new test locks the two rows and nothing around them. `contract/cli/tests/it/gallery.rs:694`

It checks `symbol` to `back`, the checkbox to `noop_bool(checked: bool)`, the text field to `noop_string(value: string)`, and `press` to `noop`. Still untested: a prop whose own name is a role (`star` stays `star`; the alias `gear` becomes `settings`), a taken name (`noop_bool` already used becomes `noop_bool_2`), a prop given with arguments (`change=set(v)` stays `noop`), a prop only forwarded on a `Use`, and the stem and conflict cases above.

`inert.rs` matches `gallery.rs`: module docs, `///` on the items the gallery calls, `unique` against `taken`, and a `BTreeMap` so declaration order is the signature order.

Byte-identity holds. An empty `declarations()` is `""`, so the header stays `action noop` then `view`. `Inert::new` runs after `placed` and `shapes` have taken their names, and nothing reads `taken` after that, so existing shapes and components keep their names. Signature insertion walks components in app-use order and props in declaration order, so a uniquified suffix is stable. `handler_parameters` only reads the existing payload table; the compile path does not call it. This was read from the diff. `cargo` was not run.
