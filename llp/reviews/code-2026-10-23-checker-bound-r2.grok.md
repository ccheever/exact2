# Code review: the design checker's bound (rt1's PRIORITY finding), round 2 of 3, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8cb003dc33`.
- **Method:** one brief (sha256 `71d576d59472af389e233daea7fe962cfaadc549e321d932e5c2654137ccc007`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at 8cb003dc33, before the rebase onto exact47's main; that commit is 0860f2a345 there.

Round 2 of `origin/main..HEAD`. Four requirements still fail. The residual explosion, the kind-cycle explosion, platform-colour depth, and well-formed dial, range, icon, and metadata values look closed. I did not run the tests; the stack finding is from the call graph and the thread sizes.

## BLOCKER

**A legal chain still overflows the 2 MiB stacks.** `tamber/src/check.rs:1030`, `tamber/src/measure.rs:614`, `tamber/src/check.rs:828`, `tamber/src/check.rs:904`, `contract/cli/src/tamber_check.rs:358`

`MAX_REFERENCE_DEPTH` (512, `tamber/src/bound.rs:28`) counts tokens. `MAX_VALUE_DEPTH` (32, `tamber/src/css.rs:35`) counts functions inside one value. Those compose. Folding `calc(calc(… next))` keeps `func` → `sum` → `product` → `operand` (`tamber/src/measure.rs:614`) live across the next token, about four frames per nested function. The guard at `check.rs:1030` runs only when `value` is entered, so the frames are already on the stack. A chain of 512 such tokens is inside both caps (the guard is `>= 512`, so the 512th token is entered) and is about `512 × 32 × 4` frames. At a 64-byte frame, a floor for a return address and a few pointers, that is about 4 MiB before the caller. The dev-server check worker is spawned with no `stack_size` (`tamber_check.rs:358`), so it gets Rust’s 2 MiB default, and the tests run on libtest’s 2 MiB thread. A chain the checker is supposed to accept aborts there; a 513th token, which should be `tamber-reference-depth`, aborts on the same stack before the diagnostic. Kind inference has the same shape one level thinner: `measures` recurses per function without adding to `walk.depth` (`check.rs:904`), then `named_kind` calls `kind_of`, and that walk runs before the fold.

Bare aliases are a few frames per hop. `tamber/tests/it/limits.rs:291` only folds 500 of those (`b{k} = b{k+1}`) and treats that as proof the cap sits under a 2 MiB stack. The Windows CLI respawns at 8 MiB (`contract/cli/src/main.rs:54`); other mains use the OS stack, and `contract tamber serve` checks on that main thread. Debug frames are larger than 64 bytes, so the full legal chain is also past a comfortable 8 MiB.

**An unknown token kind is echoed in full.** `tamber/src/check.rs:710`, `tamber/src/bound.rs:49`, `tamber/src/syntax.rs:517`

`ink: <megabyte ident> = #fff` parses. `token` stores `kind` with no length cap (`syntax.rs:517`). `preflight` walks name, value, `when`, and metadata, and never `Token.kind`. `declare` then builds `` `{other}` is not a kind ``. A wire `AddToken` kind goes through `ident` → `long` and comes back as `tamber-edit` with a size and no text (`tamber/src/edit.rs:377`). The file and the wire disagree, and the file echoes.

**Wire names and metadata are a second limit.** `tamber/src/edit.rs:305`, `tamber/src/edit.rs:384`, `tamber/src/edit.rs:543`

Values share `css::over_limit` (`tamber-value-size`, “a value is at most 4096 bytes; this one is N”). Names, params, axis values, and metadata do not. `long` and the metadata loop refuse as `tamber-edit` (`edit.rs:543`) with “a name is at most 4096 bytes…” and “a metadata word is at most 4096 bytes…”. The protocol puts a name or metadata word past 4096 under the same bullet (`docs/tamber-protocol.md:767`). The lengths differ too: the wire measures the whole `key=value` word; preflight measures the key and the unquoted value separately (`tamber/src/bound.rs:58`). A 2000-byte key and a 2000-byte value is refused on the wire and accepted on disk.

**Reimport still echoes an oversized name.** `tamber/src/export.rs:594`, `tamber/src/import/mod.rs:821`, `tamber/src/export.rs:734`

`dtcg_doc` bounds JSON string values only. `json_strings` walks object values and skips keys (`import/mod.rs:821`). `tokens_by_context` reports a bad `$ref` or a nest past 16 as `` `{context}`: {e} `` (`export.rs:594`), and `context` is that key. An incoming document whose context key is a megabyte, with `sources: [{"$ref": "#/nope"}]`, is refused by echoing the key. The explicit baseline (`export.rs:734`), the baseline parsed again at `export.rs:755`, and `baseline_for` at `export.rs:681` use raw `serde_json::from_str`, so a baseline `$ref` is echoed from `export.rs:566` the same way. Incoming `$value` strings are bounded; that part of round 1 is closed. `budget::REIMPORT` caps copies, not this echo.

## SHOULD

**Parse diagnostics still quote the offending text, and `check` parses before preflight** (`tamber/src/lib.rs:183`). A shape that never becomes a `Design` never reaches `preflight`. The quotes are `tamber/src/syntax.rs:357` (dial kind), `:435` (malformed range), `:444` and `:449` (metadata), `:388` and `:537` (names), `:597` (`when` condition), `:322` (`under`), `:131` (header), `:203` (export line), and `tamber/src/component.rs:80` (the whole line). Well-formed `in low..high`, `key=value`, dial values, and icon values are covered by preflight and are not echoed.

**A CSS source value past 4 KiB uses a shorter sentence.** `tamber/src/import/mod.rs:834` emits `tamber-value-size` as `` `{name}`: a value is at most {VALUE} bytes `` and drops “this one is {len}”. Same id, different words from `over_limit`. The long name no longer hides the long value.

**Preflight’s column is a neighbour’s.** Conditions (`tamber/src/bound.rs:54`), metadata (`:59`), range ends (`:67`), and axis values (`:93`) are reported at the `when`, the token value, the dial value, or the axis name. The refusal still happens, the words are `over_limit`’s, and the scan is one pass over each string: linear in the design text.

**Icon aliasing runs before preflight on the laying path, and it is cubic.** `icon::aliased` (`tamber/src/icon.rs:126`) does up to `icons.len()` passes, each scanning every icon. `params::laid` calls it at `tamber/src/params.rs:170` and returns. `edit.rs:501`, `session.rs:332`, and `fold_with` (`contract/cli/src/tamber_params.rs:183`) lay, then `check_design`. On that path a chain of alias icons with unbounded values is compared and cloned before `preflight`. `check` / `check_design` (`tamber/src/lib.rs:191`) preflights first, so a plain check only cubes strings of at most 4096 bytes. Token count is still uncapped.

## NIT

**The cycle diagnostic still carries twelve names.** `tamber/src/check.rs:1018` keeps six, `…`, and six. Each name can be 4096 bytes, so the message can be about 50 KB. The middle of the chain is elided, which is what round 1 asked for.

**Preflight skips `Design.name` and a component param’s kind.** A header that is an identifier of any length is stored (`tamber/src/syntax.rs:127`) and copied into the DTCG header (`tamber/src/export.rs:400`). A component param kind is stored by `ranged` and the checker does not format it. Neither is the echo at `check.rs:710`.

## Closed, and the two questions that are not defects

Residual growth is bounded as it accumulates: each sum term (`tamber/src/measure.rs:502`), the arguments together before pick or clamp (`measure.rs:385`), and the result, with the fold memoized per token. Kind inference memoizes through cycles (`check.rs:822` hits `known`, `check.rs:825` returns `Color` for a token already on the walk and does not store that sentinel). A small cycle read twice a level is no longer exponential.

Always memoizing does not change kinds or diagnostics for an acyclic design. The memo key is the token index. The cycle sentinel is not written into `known`. `known[i]` is written only after `inferred_kind` returns. That function does not depend on which path reached the token, and `walk.depth` is still the path length, incremented and decremented around the call. A shared suffix is not walked twice; the kind stored is the kind a second walk would have computed.

Platform fallbacks count depth in the reader (`tamber/src/css.rs:278`, `parse_at` at `:347`). `refusal_id` maps that runtime message to `tamber-value-depth`. The reader and `over_limit` use the same quote rule, so an escaped quote does not make them disagree. The reader’s own counter stops the recursion at 32.

`under_value` folds the light side, then the dark side, while the token is already on `stack`. Typeface and motion resolve one name through `Names::named` → `value`. Dial values use `Literal` and do not follow tokens. Colour references in `fold.rs` go through the same `value` cap. None of these is a second unbounded recursion. Nested colour functions sit in the same frame-composition hole as nested `calc`. Export of a folded design does not walk source chains again; `export_batch` forwards the checker’s diagnostic.

Across 51 `.tamber` files outside `vendor`, the longest line is 119 bytes (`tamber/tests/corpus/bluesky-rfc.tamber:17`) and the deepest parenthesis nest is 3. Nothing in the corpus approaches 4096 bytes, 32 functions, or 512 names. Importer chains stop at 256, under 512. The checker was not re-run on those files in this pass.
