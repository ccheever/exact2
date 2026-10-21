# Code review: LLP 1122 T7a (navigate, a status's location, framed copies that say their build; tamber/1.14), round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `159a8d77a9`.
- **Method:** one brief (sha256 `5c68268af72cdfd240df5a61d4810fc851ad5270dd0e52dd40f992294baffd8d`), the design inline, shared by both reviewers, each blind to the other's review; commits 7465598517..159a8d77a9, with the records of rounds 1 and 2. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX, 1 NIT).
- **Disposition:** post-round, in the commit named in the sign-off request:
  - `navigate` refuses a `|` and a broken `%`. Only a location spelled as the router writes it is taken, and both pages compare it literally (1, 2).
  - **Not taken:** an agent session's launch keys pushing a long address past the bound (3). The copies the tool frames carry no agent query; the status then says null, as recorded.

Round 1 and 2, as they stand now:

- A navigate frame is stamped with the stream nonce before it is queued (`host/web/dev.js:923`, `host/web-js/dev.mjs:714`) and dropped before routing or `unsupported` if that stream is gone (`host/web/dev.js:474-476`). The scheduler test parks a routable `/post` behind a pin, reconnects, and checks that nothing was routed and nothing was answered; the new stream's same seq then routes once.
- `there()` treats the router's entry as arrival, so launch keys on the address (`agent`, `seed`, and the rest) stay put and a redirect is not overwritten. The wait ends as soon as `history.state.url` is a different route (`host/web/dev.js:497`).
- A dot-segment path that resolves to `//…` is refused by `located` and by both pages. Detail text is cut to 1024 characters, with the location inside it cut at 200. A stale navigate refusal returns before the journal line.
- `exact-copy-<n>` (`n` of one or two digits) is the storage key on both pages. The name is only read. The parent message is `{type:'exact-build', id}`.

1. **SHOULD-FIX** — A path of `|` is admitted and then the copy's location becomes null. `scripts/tamber-builds.mjs:71`, `host/web-js/router.js:31`, `host/web/dev.js:458`.
   The server measures the URL parser's spelling. That parser leaves `|` as itself, and the router's canonical form writes each one as `%7C` (`route/src/location.rs:110` does the same on the wasm page). Send `'/post/' + '|'.repeat(681) + '#c'` to the copies app, whose route is `/post/:id`. The command is 689 characters and resolves on the page's origin, so `located` accepts it. The page routes, `there()` is true, and the fragment is applied. The address is then 2051 characters, `here()` returns null (`host/web-js/dev.mjs:707` too), and the status stores `location: null`. No `unsupported` is sent. `scripts/tamber-builds.test.mjs:715` covers a run of `"` (the parser expands those) and does not cover `|`, so it stays green.

2. **SHOULD-FIX** — A `|` next to a broken `%` escapes arrives without its fragment. `host/web/dev.js:485`, same check in `host/web-js/dev.mjs:712`.
   `spelled` decodes the router's entry and the requested path in one `try`. If either `decodeURI` throws, it falls back to raw equality. Send `/post/a|b%#c` (or `/post/a|b%80#c`). The server accepts it. The router writes `/post/a%7Cb%`, which decodes cleanly, while `decodeURI('/post/a|b%')` throws. The fallback then compares those two different strings, so `there()` stays false, `#c` is never set, and the status says `/post/a%7Cb%`. A bare `/foo%#c` still gets its fragment, because there the router's spelling and the request are the same string.

3. **NIT** — On an agent session, launch keys are appended after the 2 KiB check. `host/web/navigation.js:129`, `host/web/dev.js:458`.
   Open the page with `?agent=1&seed=1&locale=en-US&timeZone=America/Los_Angeles&epoch=0&storage=memory` and navigate to `'/' + 'x'.repeat(2040)`. `located` accepts the path (2041 characters). `historyURL` adds those keys and the address is 2123 characters, so `here()` is null. `there()` itself is fine: it matches `history.state.url`, which has no launch keys, and the fragment step keeps the query. Design-tool frames have no `agent` query, so the grid does not hit this.
