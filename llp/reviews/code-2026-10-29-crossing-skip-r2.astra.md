# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `aed74f34cd`.
- **Method:** one brief (sha256 `f2ce6e9ddd1b14bb0cc3cf154777a7e9a2b3ee2d09a091b96900b31585fc6b92`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **The token regex can match inside an ordinary string.** [js/src/binary.js:447](js/src/binary.js:447): an unfrozen value containing `'a"' + "\u0000" + "1"` serializes as `"a\"\u00001"`. `TOKEN` starts at the escaped quote, but `SPELLED` recorded no place for that string. Beside kept rows, this rejects a valid reply with the count-mismatch error. The existing `nulQuote` fixture ends with another quote, which prevents this match. **Fix:** scan complete JSON string tokens while respecting escapes, then recognize placeholders; test this suffix in both values and keys.

2. **Intrinsic unboxing does not predict a boxed string’s JSON.** [js/src/binary.js:478](js/src/binary.js:478): `String.prototype.valueOf` reads the internal string, whereas JSON serialization performs string coercion, honoring an own `toString` or `Symbol.toPrimitive`. For example, define an own `toString` on `new String("ordinary")` returning `"\u0000" + "1"`. The emitted token-shaped string has no recorded place, so a reply containing kept rows fails. Conversion in the opposite direction creates a phantom place. **Fix:** perform JSON’s actual boxed-string coercion once, return that primitive from the replacer, and record the resulting string.

3. **The size heuristic adds observable reads on proxy arrays.** [js/src/binary.js:468](js/src/binary.js:468): JSON reads an array’s length to start serialization; this code additionally reads `this.length` for every object element. A proxy array whose length trap increments a counter, with row getters returning that counter, now produces different values—even when every row is unfrozen and nothing is kept. Rejecting proxy *elements* does not protect the holder. **Fix:** remove this observable length probe or guard it with a trusted engine-level check that excludes proxy holders. Add a proxy-array regression case.

4. **Frozen prototypes do not establish the required Proxy guarantee outside Hermes.** [js/src/binary.js:462](js/src/binary.js:462): freezing both prototypes enables caching in the web/Bun realm. There, a proxy over a frozen target can pass `isFrozen`. Put such a proxy inside a frozen row and have its `get("toJSON")` return a hook reading a changing counter: the row passes `frozenData`, and subsequent replies splice stale text. **Fix:** enable caching only in the trusted native runtime with the required guarantees; keep other realms uncached unless they provide an equivalent guarantee. Extend the hardened-realm test with this nested proxy.

**Non-blocking**

1. **Records shorter than 64 bytes do not normally hit the cache.** [js/value/src/decode.rs:107](js/value/src/decode.rs:107): insertion hashes only the element’s bytes, while lookup hashes the remaining input, including following delimiters/elements. Thus `{"n":1}` gets different insertion and lookup keys. Either explicitly exclude short records or support matching their shorter prefixes. Add allocation-identity assertions around the 64-byte boundary.

2. **Superseded values remain retained.** [js/value/src/decode.rs:434](js/value/src/decode.rs:434): `{"tag":0,"value":[record],"value":[]}` returns an empty list but keeps the earlier record in `Rows.next`. Repeated list-valued record fields have the same issue. Output remains correct, but the stated bound to the final value’s elements does not hold. Clear superseded contributions, or narrow the documented bound and test it.

3. **The proxy test does not prove exclusion from caching.** [js/tests/it/kept_json.rs:124](js/tests/it/kept_json.rs:124): even a cached top-level proxy receives JSON’s `toJSON` lookup before the replacer, so its read counter still increases. Assert the engine’s exclusion directly and test a nested proxy whose serialization changes.

The tag-3 issue is resolved at [decode.rs:66](js/value/src/decode.rs:66), with real-call allocation checks present. Syntax errors clear the cache; shape errors retain individually valid rows while returning the shape error. Captured replies bypass it at [lib.rs:827](js/src/lib.rs:827), and streams use the same decoder at [lib.rs:1437](js/src/lib.rs:1437).

`starts_with` is sound for these successfully decoded **record objects**: their closing brace fixes the value boundary, and the parser still validates the following separator. Bucket collisions cannot change values because bytes are compared, and buckets are capped at eight.

The remaining bookkeeping fixes are present: local splice state, exclusions for omitted keys and array keys, post-`toJSON` processing, capture before recording, and replacement text not being rescanned. Function/symbol exclusions and Excord’s row freeze are present. The capped files are 1,487 and 1,499 lines.

Static review only; no files changed, network access, tests, or builds.
