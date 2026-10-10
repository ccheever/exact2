# Filesystem issues on main fail the format check

**Status:** Closed
**Resolution:** Recovered audit repairs the three malformed headers; original qualifiers are preserved in body text or Resolution. No new blocking check added.
**Systems:** issues, scripts
**Severity:** P3
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** docs/issues.md; scripts/issue.mjs

`bun scripts/issue.mjs check` on main reports 7 problems in 3 files. It is not part of `scripts/caps.mjs` or `scripts/boot.mjs`. `docs/issues.md` says to register it inside an existing blocking check. Wiring a new check needs a human's yes (the five-check budget). Repairing the files does not.

- `issues/20261002-scroll-efficiency.md` has no `Systems`, `Author`, or `Date`, and its `Status` is a sentence about commits on the `ide/exact2` fork. An open path requires `Status: Open`. If that work landed, close it with `issue.mjs close` and a resolution. If it did not, give it a real header and keep the sentence in the body.
- `issues/closed/20260924-grant-readers-disagree.md` puts a qualifier on `Status`. `issue.mjs check --fix` moves that text into `Resolution` verbatim.
- `issues/closed/20260930-taffy-patches-not-sent-upstream.md` has a qualified `Status` and no `Resolution`. `--fix` will not invent one. Move the parenthetical into `Resolution` by hand, or write the resolution and then `--fix` the status token.

Do not close the scroll issue with `git mv`.
