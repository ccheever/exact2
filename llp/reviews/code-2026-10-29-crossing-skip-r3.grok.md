# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `f31626a43f`.
- **Method:** one brief (sha256 `a7fed35572c98562732ebc567faebb68300f4f82ada5115402fb44a80e34e904`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the last three commits read-only, starting with the diffs and the data-module crossing they describe.The three commits are the crossing stages. I'll read the patches and the code they touch, focusing on the token rule, boxed strings, and the proxy residual.The splice and the reuse path are the risky parts. I'll read those implementations and the tests that claim to pin them.The keep check and the row freeze both walk objects they do not own. I'll see whether that freezes store data or changes what JSON observes.The equality claim hinges on a few edge cases. I'll check those against the engine's `JSON.stringify`, not the project tests.**Blocking**

1. `js/src/binary.js:458` — `frozenData` reads `v.toJSON` with `[[Get]]` while it is still deciding whether to keep an ancestor. That read happens in the ancestor's replacer, before `JSON.stringify` serializes the nested value and reads `toJSON` again. An own accessor (or a proxy get trap) therefore runs once more than a plain `JSON.stringify`. A getter that returns `() => ({ seen: n })` and increments `n` on each get produces `seen: 1` from plain `JSON.stringify` and `seen: 2` from `keptReply`. The same split shows up for a nested proxy whose trap counts gets. A `toJSON` on the array element itself is fine: JSON calls it before the replacer, so the probe only sees the result. The nested `hooked` case is a data property, so retrieving it is not a user hook; this path is untested.
   Fix: decide from the own descriptor and do not `[[Get]]` an accessor or a data `toJSON`:
   ```javascript
   var hook = describe(v, "toJSON");
   if (hook) {
     if (!("value" in hook) || hook.value !== undefined) return false;
   } else if (v.toJSON !== undefined) return false;
   ```
   Add a frozen row whose nested frozen object has a counting `toJSON` accessor and assert the reply equals a plain stringify (`seen` 1).

**Non-blocking**

1. `js/src/binary.js:480-482` — A boxed string that converts successfully matches JSON: `String.prototype.valueOf` is only the `[[StringData]]` brand check (no user hook), and `String(out)` is `ToString`, so an own `toString` or `Symbol.toPrimitive` runs once and the quoted text is the same. If `ToString` throws, the `try` swallows it and JSON runs the hook again (twice, then the throw). Brand-check in the `try`, and let `String(out)` throw.

2. `js/src/binary.js:458` (the `else` branch above) — A proxy that answers `toJSON` only from a get trap still needs that `[[Get]]`, and JSON gets it again, so a trap with side effects diverges on the first reply. There is no JavaScript way to both refuse that trap and let JSON read `toJSON` once. The later-change residual is real: this runtime's `structuredClone` accepts a live proxy (`vendor/ibex/crates/ibex2/src/bindings/structured_clone.js:306-309`), and no language check tells a Proxy apart. State the extra get next to the "answers none now and one later" comment. V8's `structuredClone` throwing does not apply on the native door, which is the only place anything is kept.

3. `js/tests/fixtures/kept-json.ts:48` — `nulTailKey` is built and never requested. `kept_json.rs` covers `nulTail` (value) and `nulKey`, not the key `a"\u00001`. The same regex treats it correctly (`"a\"\u00001"` is not a token; a following spelled value still matches once). Call `rows("nulTailKey")` the way `nulKey` is called.

4. `apps/excord/render.test.ts:74` — The "past the 16" comment is stale. Keeping no longer depends on list length (`js/src/binary.js:473`).

Round 2 is otherwise resolved. The token rule is sound: `TOKEN` (`js/src/binary.js:448`) matches a `"` only after an even run of backslashes, and in `JSON.stringify` output the only raw quotes are delimiters. A quote inside a string is `\"` plus an even run from the content, so the run before it is odd (`"a\"\u00001"`, a key of that shape, 0–5 trailing backslashes). A key's closing quote is followed by `:`, not `\u0000`. `":"` and `[` / `{` / `,` are the opening quote's prefix and are put back in `before`. The next match does not need that character: the match ends at this token's closing quote, and the next prefix is the structural character after it (`,"\u00002"`, `:"\u00002"`). Under-64-byte elements are not kept (`js/value/src/decode.rs:137-138`), the length probe is gone, and the collision, captured-reply, and shape-error tests are in place.
