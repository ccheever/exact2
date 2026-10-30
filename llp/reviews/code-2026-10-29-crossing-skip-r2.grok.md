# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `aed74f34cd`.
- **Method:** one brief (sha256 `f2ce6e9ddd1b14bb0cc3cf154777a7e9a2b3ee2d09a091b96900b31585fc6b92`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `js/value/src/decode.rs:107-112` and `:123-126` — an element shorter than 64 bytes is stored under a hash the lookup does not recompute. `key` hashes `bytes[..min(64, len)]`. `keep` hashes the element alone (`:134-135`); `get` hashes `items.rest(start)` (`:306`), which still holds the following comma or `]}`. For length L < 64 the two hashes cover different bytes, so `starts_with` is never tried. The miss path decodes in full (`:312-318`), and the value stays a full decode's. Excord rows and the random-window rows are longer than 64 bytes, which is why `ptr_eq` holds there. The collision test (`:453-482`) uses `{"id":"r{i}","n":{}}` (about 16 bytes) with every hash forced to 0 (`:113-116`), so those short rows hit anyway. Fix: skip storing an element shorter than 64 bytes, or keep short elements in a per-shape list that `get` scans with `starts_with` only. The lookup cannot know the element's end without parsing.

2. `js/src/binary.js:444` and `:477-480` — a boxed string is recorded via `String.prototype.valueOf`, while `JSON.stringify` unboxes a replacer return with `ToString` (`@@toPrimitive`, then `toString`, then `valueOf`). A default `new String` agrees, and `boxed` covers it. A String whose `toString` or `@@toPrimitive` disagrees with `valueOf` records a place the text lacks, or the reverse. With any kept row, the count check throws (`:486`) and that valid reply fails. With nothing kept, `:483` returns the stringify text unchanged. Fix: unbox with `ToString` / `@@toPrimitive`, in the order JSON writes.

3. `js/src/binary.js:439-440` and `:453` — "a proxy is never kept" is Hermes's `Object.isFrozen`. A forwarding proxy over frozen data answers `isFrozen` true on a spec-compliant engine, and `keptReply` would cache the first trap results. The native engine is Hermes, and the web module realm fails the `keeps` gate (`:462`), so this host pair does not keep proxies. The probe (`js/tests/it/kept_json.rs:111-128`) runs on Hermes; the hardened bun test has no proxy. Fix: if a hardened JSC or V8 realm is in scope, reject a proxy explicitly there. Otherwise state the Hermes limit in that comment.

4. `js/src/binary.js:454-457` — `frozenData` sees own names only. A hole in a frozen array still reads `Array.prototype[0]` during `JSON.stringify`, and an inherited `toJSON` runs again inside the cached `JSON.stringify(item)` (`:470`). A closure over mutable state makes that cached text stale. Hardening before user code closes this, and an unhardened realm keeps nothing (`host/web/kept-reply.test.mjs:19-27`). Fix: if module code can touch those prototypes before they freeze, skip keeping when `Object.prototype.toJSON` or an array index on `Array.prototype` is present.

5. `js/value/src/decode.rs:471-472` — the comment says the one entry kept is the last row's. `put` keeps the first 8 and never replaces (`:138-142`). The test compares bytes, so it passes either way. Fix: say the bucket keeps the first 8 and the byte compare rejects the others.

6. Paths whose code matches the rule above and have no direct assertion. A captured reply leaves `Rows` in place, including when its JSON fails to parse (`js/src/lib.rs:826-835`); `large` checks the restored value, not the cache. A stream `ok` replaces that source's rows and `fail` leaves them (`js/src/prelude.js:1373-1374`). A `\u0000`+digits string inside a deep-frozen kept row: in `nul` and `boxed` the token element is the unfrozen one, and its neighbors are what get kept (`js/tests/fixtures/kept-json.ts:29-36`). A shape-error reply's good siblings being reused on the next success (`js/value/tests/it/rows.rs:238-245` checks the error). A short element looked up with `COLLIDE` off. Fix: one assertion each; for the captured reply and the stream, `Rows::len` or `ptr_eq` on a following window of the same source.

7. `apps/excord/render.ts:379`, `js/tests/fixtures/kept-json.ts:1`, `js/tests/it/kept_json.rs:2` — the comments still name `prelude.js keptToken`. The function is `binary.js keptReply`. Fix: point them at `keptReply`.
