# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e58c1eda88`.
- **Method:** one brief (sha256 `3d938665967ad333fb61bccb4343c511020cce777cd12cfc024e99a2aecebd60`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Ordinary calls clear the decode cache before using it.** [js/src/lib.rs:837](js/src/lib.rs:837) sends every envelope through the cached decoder. `begin()` first decodes the tag-3 acknowledgement at [line 1008](js/src/lib.rs:1008); that envelope has no rows, so [decode.rs:68](js/value/src/decode.rs:68) replaces the previous cache with an empty one. Excord’s next window therefore fully decodes again. Preserve the cache across tag-3/tag-1 control replies; replace it on a completed answer and clear it on terminal failure. Add a repeated `Module::answer` test asserting row allocation reuse.

2. **Boxed strings can be mistaken for splice tokens.** [prelude.js:1027](js/src/prelude.js:1027) checks only primitive strings, but `JSON.stringify` unboxes String objects *after* calling the replacer. A reply containing cached rows and `new String("\u0000" + "0")` leaves `keptClash` false; [line 1044](js/src/prelude.js:1044) replaces that user string with cached row 0. An embedded quote can also corrupt the split. Splice only locations recorded as synthetic replacements, rather than every matching substring.

3. **The U+0000 fallback repeats observable work and breaks captures.** [prelude.js:1041](js/src/prelude.js:1041) serializes the answer twice, invoking getters and `toJSON` twice. More directly, cached rows beside a primitive U+0000 string and a 70,000-character string register the same capture path twice. [engine.rs:384](js/src/engine.rs:384) rejects the second restoration because the first already filled its placeholder. Use collision handling that requires only one serialization; merely clearing captures before retrying would leave the getter/hook discrepancy.

4. **`frozenData` admits values whose JSON can change.** [prelude.js:1015](js/src/prelude.js:1015) treats functions as harmless primitives: a frozen row containing `f.toJSON = () => counter` caches the counter’s old value. The prototype check also admits a proxy over a frozen plain target whose `get` trap supplies a changing nested `toJSON`. A frozen Number wrapper with a null prototype and counter-reading `Symbol.toPrimitive` passes too. Native intrinsic hardening prevents none of these application-owned hooks. Reject callable and exotic descendants, and establish ordinary, non-proxy objects through an engine guarantee. Also avoid the extra observable proxy `length` reads at [line 1028](js/src/prelude.js:1028).

5. **Serialization state is not re-entrant.** [prelude.js:1039](js/src/prelude.js:1039) resets module-global `spliced` and `keptClash`. A `toJSON` that calls `__exact_call`/`__exact_settle` for another answer overwrites the outer token table. An inner spliced reply leaves it `null`, causing the outer operation to throw; an inner ordinary reply can discard the outer tokens. Make splice state local to each `ok()` invocation and pass it into its replacer. This also releases temporary strings when serialization throws.

6. **The hardening assumption fails on the browser wasm host.** [prelude.js:1016](js/src/prelude.js:1016) accepts intrinsic prototypes without checking their immutability. [module-glue.js:185](host/web/module-glue.js:185) and [module-worker.js:112](host/web/module-worker.js:112) execute this prelude without native hardening. For example, cache a frozen row containing a frozen sparse array, then assign `Array.prototype[0]`: full stringify changes the hole’s value, while the cache retains `null`. Restrict this optimization to a realm with the required hardening guarantee, or provide an equivalent guarantee there.

7. **Both runtime files exceed the binding source cap.** The additions at [prelude.js:1012](js/src/prelude.js:1012) and [lib.rs:153](js/src/lib.rs:153) bring those files to 1,537 and 1,502 physical lines. Both exceed 1,500; [caps.mjs:165](scripts/caps.mjs:165) counts the trailing empty line additionally. Extract code before landing.

**Non-blocking**

1. **The public decode API needs a shape-lifetime invariant.** [decode.rs:109](js/value/src/decode.rs:109) identifies shapes by address. Mutating a record field from `Number` to `Option(Number)` in place preserves that address, so identical bytes return the old, unwrapped Value. Current `Sig` usage avoids this: shapes remain unchanged and [source.rs:327](js/src/source.rs:327) clears signatures on bind. Bind public `Rows` to an immutable shape lifetime, or explicitly enforce invalidation.

2. **“Dropped on failure” is only partly implemented.** [decode.rs:68](js/value/src/decode.rs:68) checks syntax success, so a shape-invalid reply retains its successfully decoded rows. Captured replies bypass cache settlement at [lib.rs:829](js/src/lib.rs:829), allowing stream sources to retain an older window. Retention remains bounded, but final success/failure handling should implement the stated replacement policy.

3. **The tests leave important claims unproved.** [rows.rs:179](js/value/tests/it/rows.rs:179) proves direct decoder reuse, but omits runtime acknowledgements, shape changes, duplicate keys, `-0`, and deliberate escape-boundary cases. [kept_json.rs:49](js/tests/it/kept_json.rs:49) tests NUL, captures and hooks separately; it misses their interactions, boxed strings, proxies and re-entrancy, and would pass with splicing disabled. [render.test.ts:74](apps/excord/render.test.ts:74) verifies its sample’s freezing but has only two rows, below the splice threshold.

Static review only; no files changed, tests/builds run, or network used.
