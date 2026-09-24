# Render server redirects and header values are not safe

**Status:** Open
**Systems:** Render server
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1048.000

`host/render` binds loopback and is the server a public site proxies to. Two of its response headers are attacker-controlled, and `Response::write` copies every header value in raw.

**Open redirect.** `static_file` refuses a backslash, then `canonical_path` does not. A path that canonicalization changes, such as `GET //\evil.com HTTP/1.1` or `GET /\evil.com/ HTTP/1.1`, is answered `301` with `Location: /\evil.com`. WHATWG URL parsing treats `\` as `/` against an https base, so that location is `https://evil.com/`. `//evil.com` is collapsed to the same-origin path `/evil.com`. The backslash form is not.

**Response splitting.** The same `Location` is the request target with empty segments removed, so a raw line feed in the target is copied into the header. `head robots` is an ordinary string prop (`kernel/tables/schema.json`, `headRobots`). `document` copies it into `X-Robots-Tag` with no filter (`host/render/src/serve.rs`). The HTML meta tag escapes that string (`host/web/src/page.rs`). A view that binds `robots` to data can end the header block early (`noindex` followed by CR LF CR LF and markup). The CSP line is then body text.

Reject CR, LF, and NUL in every header value. Emit a `Location` only when it is a same-origin path: no `\`, no `.` or `..` segment, no controls. Keep `robots` a token list. Done when `/\evil.com/` stays on the app origin and a robots value containing a newline is a refused header, not a second response.
