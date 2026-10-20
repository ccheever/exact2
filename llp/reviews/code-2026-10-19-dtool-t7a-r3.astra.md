# Code review: LLP 1122 T7a (navigate, a status's location, framed copies that say their build; tamber/1.14), round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `159a8d77a9`.
- **Method:** one brief (sha256 `5c68268af72cdfd240df5a61d4810fc851ad5270dd0e52dd40f992294baffd8d`), the design inline, shared by both reviewers, each blind to the other's review; commits 7465598517..159a8d77a9, with the records of rounds 1 and 2. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (3 SHOULD-FIX).
- **Disposition:** post-round, in the commit named in the sign-off request:
  - `navigate` takes only a location already spelled as URLs and the router write it: RFC 3986's path, query and fragment characters but `'`, each `%` a whole escape. Both pages compare that spelling literally, with no decoding, so `/%66oo` and `/foo` stay distinct routes, and a pipe can't be escaped past the bound (1, 2).
  - **Not taken:** a navigation that arrives while a Back is still traversing may stop at the intermediate entry without its fragment (3). The copy's status then says where it is, so the tool sees it and the next move corrects it. This is recorded as a known limit.

**Nothing blocks. Three SHOULD-FIX findings remain.**

1. **SHOULD-FIX — decoded equality can skip a different route or decorate a redirect.**  
   [host/web/dev.js:485](/tmp/rv-dt-t7a3/host/web/dev.js:485), mirrored at [host/web-js/dev.mjs:712](/tmp/rv-dt-t7a3/host/web-js/dev.mjs:712), compares locations using `decodeURI`, while the router preserves existing escape spelling. With distinct routes `/foo` and `/%66oo`, navigating from `/foo` to `/%66oo#c` calls neither handler nor router and leaves `/foo#c`, without unsupported. A handler redirecting `/%66oo` to `/foo` also incorrectly receives the requested fragment. Conversely, `/post/a|b%#c` still loses its fragment: decoding throws, and the literal fallback differs from `/post/a%7Cb%`. Compare the router’s canonical spellings instead.

2. **SHOULD-FIX — the length bound still misses the router’s escaping.**  
   [scripts/tamber-builds.mjs:68](/tmp/rv-dt-t7a3/scripts/tamber-builds.mjs:68) accepts `'/post/' + '|'.repeat(700) + '#c'`: 708 characters, unchanged by URL serialization. The [router serializer](/tmp/rv-dt-t7a3/host/web-js/router.js:31) escapes each pipe, producing 2,108 characters. Both pages navigate successfully, then `here()` reports `null`, without unsupported. Round 2’s quoted-fragment example is fixed, but an admitted navigation can still erase the reported location. Bound the canonical routed location too.

3. **SHOULD-FIX — an intermediate history traversal is mistaken for a completed redirect.**  
   [host/web/dev.js:497](/tmp/rv-dt-t7a3/host/web/dev.js:497), mirrored at [host/web-js/dev.mjs:712](/tmp/rv-dt-t7a3/host/web-js/dev.mjs:712), stops waiting whenever `history.state.url` changes. Suppose Back from `/post/3` to `/post/2` is pending when `/post/1#c` arrives. The first popstate [drains the queued navigation](/tmp/rv-dt-t7a3/host/web/navigation.js:305), starting another traversal. If the next polling turn precedes that second traversal, the command finishes at the intermediate entry. The final address becomes `/post/1`, missing `#c`, with no refusal. Completion needs to distinguish a settled redirect from another traversal still pending.

The prior dispositions check out as follows:

| Record | Disposition at HEAD |
|---|---|
| **Round 1 Astra** | Stream binding and presence fixes hold. Ordinary redirects are preserved; diagnostics are bounded. Specified JS handler/link/Back and wasm hashchange coverage was added. Arrival and length remain affected by findings above. Routed wasm remains the recorded omission. |
| **Round 1 Grok** | Agent launch keys survive; dot-segment `//` paths are refused by server and pages; history exceptions produce unsupported; stale refusals are not journaled. ASCII validation holds. Unique-name assumptions, over-limit `null`, and the defensive no-router guard remain as recorded. |
| **Round 2 Astra** | Ordinary pipe arrival and quoted-fragment bounds are fixed, with the remaining cases above. Scheduler test (c) now checks suppressed routing and reused-sequence routing. |
| **Round 2 Grok** | Ordinary committed redirects avoid the wait, but finding 3 remains. The one-or-two-digit name rule is documented consistently. Scheduler coverage improved; recorded JS-race and agent-key integration omissions remain. |

Both pages bind navigation before queueing and check ownership before routing, fragment changes, and refusals. The `exact-copy` rule matches on both pages; names are only read, and registration messages contain only `type` and `id`. Write scope, journaling, and sender-only filtering look correct. I found no origin escape.

The committed tests do not cover the three cases above, exact message-payload shape, or refusal delivery to two actual clients. Review included in-memory probes of extracted handlers and history helpers; no files changed, builds, or test suites run.
