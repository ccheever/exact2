# LLP 1124 (host extensions), round 1 blind review: Grok 4.7, xhigh

Reviewed at PR #567's head `fdf91d204` (then numbered 1123), 2026-10-16. The number-collision finding is resolved by the renumbering to 1124.

The PR adds only `llp/1123-host-extensions.rfc.md`. Git merges cleanly onto current `origin/main` (`c3c0bd8e0`): different path, no content conflict. The description is right that there is no code, no new agent operation, and no `llp/current/` link (the working set is still 15). The spike commits it names exist, and `for` / `if` / `toFixed` did land between `7fa397d4e` and `d15a3f8ed`. There are no earlier review comments to check. The number it took is no longer free.

## Findings

1. **Blocker.** `llp/1123-host-extensions.rfc.md:1` (status at line 3). This file is titled LLP 1123. Current main already has `llp/1123-content-owners-and-local-safe-areas.rfc.md` (landed 2026-10-15 in #627, `8d38b64a2`), and the kernel, runner, and Apple host cite that document as LLP 1123 (D2). The renumber on 2026-10-14 was before that landing, so the two 1123s were chosen independently. After merge, "LLP 1123" means two documents, and a review file named `llp/reviews/1123-…` collides with the content-owners reviews. Numbers are not reused; 1124 is free on `origin/main`. **Failing scenario:** someone follows a "LLP 1123 D2" comment and opens this RFC, or archives the wrong 1123 out of `llp/current/`.

2. **Should-fix.** `llp/1123-host-extensions.rfc.md:159` against `:81` and `:182`. D5 says the runner turns a source marked `sync` into `native.call`. D1's sketch gives the web implementation `call`. D6 says the web exports `later` and `connect` only, because LLP 1067.000 D9 has no synchronous call and a `call` throws. The app does not write the source (D5), so the portable `try call / catch later` in 1067.000 D9 is not in the path. Line 221 says this closes LLP 1069.007 D5, which requires the substitute to specify synchronous reads. **Failing scenario:** an extension source marked `sync` bakes, answers on iOS, and throws on the web.

3. **Should-fix.** `llp/1123-host-extensions.rfc.md:201-207`. Setting `prefer extension` stores the value and does not call the module. A live source updates only when the implementation announces `battery.battery` (D5, lines 160-162). Other `prefer` groups re-answer readers in that commit (LLP 1069.007 D2). This one does not, and it also does not say that a later announcement is ignored while a prefer is set. **Failing scenario:** the view shows the substitute's "50%". The agent sets `battery.battery` to `0.2`. Nothing announces, so the tree stays "50%". If the substitute keeps announcing on the clock, as the §6 spike does, the next re-ask either shows the substitute again (prefer lost) or waits up to a minute to show `0.2`.

4. **Should-fix.** `llp/1123-host-extensions.rfc.md:113-115`. Adding a source, send, or word does not bump `version`, and an implementation only names which versions it answers. D3 and D9 check that some implementation of that version exists, not that it covers the new name. The plan's SHA-256 (D4) is recorded and shown, not compared to what the host built. **Failing scenario:** `battery` version 1 gains `lowPower`. A host binary that still says it answers version 1 has no such source. The app lists `battery: 1` and reads `lowPower()`. The bake passes. In a release build the reply is not checked (D5 checks only development builds).

## Recommendation

MERGE AFTER FIXES: renumber to 1124 (finding 1), and close the web `sync` path, the prefer invalidation, and the additive-version hole (findings 2–4) before Charlie rules on the text.

VERDICT: FINDINGS

