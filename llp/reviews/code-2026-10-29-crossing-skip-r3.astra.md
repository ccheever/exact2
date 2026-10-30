# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f31626a43f`.
- **Method:** one brief (sha256 `a7fed35572c98562732ebc567faebb68300f4f82ada5115402fb44a80e34e904`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Boxed-string conversion still changes exception behavior** — [js/src/binary.js:482](js/src/binary.js:482). The `catch` covers both the internal-slot check and `String(out)`. If a boxed string’s `toString` throws once and succeeds next time, the first exception is swallowed and JSON retries the conversion; ordinary JSON would fail immediately. An always-throwing hook runs twice. Catch only the `unbox` brand-check failure, then perform `String(out)` outside that catch. Add a throwing, counted conversion hook. Successful conversions otherwise use the correct conversion at the correct point.

2. **The eligibility check invokes ordinary `toJSON` getters again** — [js/src/binary.js:458](js/src/binary.js:458). JSON has already read a list element’s `toJSON` before calling the replacer. A frozen object with a non-enumerable `toJSON` getter returning `undefined` is read again here, before its accessor descriptor causes rejection. A getter that throws on its second read therefore breaks a previously valid reply. Reject accessor descriptors before performing this lookup; add a regression asserting the getter’s call count and exception behavior.

3. **The proxy residual is broader than documented** — [js/src/binary.js:455](js/src/binary.js:455), [js/src/binary.js:459](js/src/binary.js:459). A proxy over `Object.freeze([,])` can always return `undefined` for `toJSON`, yet return a counter-dependent value for missing index `"0"`. The own-property walk sees only `length`, accepts the graph, and caches stale array contents. This does not require the documented changing-`toJSON` case.

   Reject proxy-bearing graphs before probing or caching them. There is no general standard JS proxy predicate, but this controlled realm could track construction through both `Proxy` and `Proxy.revocable` before application code runs; a native predicate is another solution. Requiring an immutable own `toJSON: undefined` closes that particular trap, and rejecting sparse arrays closes this example, but neither preserves arbitrary proxy trap side effects. The current exception is therefore not accurately bounded as stated.

**Non-blocking**

1. **`nulTailKey` is defined but never exercised** — [js/tests/fixtures/kept-json.ts:48](js/tests/fixtures/kept-json.ts:48). The key-error test invokes only `nulKey` at [js/tests/it/kept_json.rs:89](js/tests/it/kept_json.rs:89). Add `nulTailKey` to that comparison against the plain stringify’s shape error.

2. **The negative-zero assertion does not test reuse** — [js/value/tests/it/rows.rs:307](js/value/tests/it/rows.rs:307). Its lone `-0` is in the uncached optional-record list; the earlier records overwrite `-0` through a duplicate key. Use a retained record exceeding 64 bytes, decode it twice, and assert reuse and preserved sign.

3. **The collision fixture initially retains nothing** — [js/value/src/decode.rs:469](js/value/src/decode.rs:469). With `n = 0`, its records are 61 or 63 bytes. The later passes exercise collision misses, but no successful collision-bucket lookup. Lengthen the fixture and assert that the initial bucket contains eight entries.

4. **The removed 16-element threshold remains in a comment** — [apps/excord/render.test.ts:74](apps/excord/render.test.ts:74). Remove that threshold claim.

The **TOKEN prefix rule is sound** at [js/src/binary.js:448](js/src/binary.js:448): an interior escaped quote has an odd backslash run; a closing quote is followed by punctuation or the end, not `\u0000`. In `":"` and successive key/value or array matches, the colon, comma, or opening container remains available as the next match’s prefix. Matches cannot steal a required prefix character.

The length probe is removed, and [the under-64-byte guard](js/value/src/decode.rs:134) resolves that retention issue. The added [captured-reply](js/tests/it/kept_json.rs:125), [kept-token-string](js/tests/fixtures/kept-json.ts:63), and [shape-error retention](js/value/tests/it/rows.rs:247) assertions are present. No further Rust-path or Excord implementation defect found by inspection.

No files edited, network used, tests run, or builds run.
