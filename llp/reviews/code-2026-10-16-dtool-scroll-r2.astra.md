# Code review: a selection's outlines and scrolling (LLP 1122 T2b follow-up), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `f287163e1`.
- **Method:** one brief (sha256 `d14b247ad907e8b78e24f23b990c3acf2f5fc04ba555f59d6a11201830d75f04`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 1 low).
- **Disposition:** taken, but the tests' finding in part.
  - Only a pointer selection stamps (`select(id, true)` from pointer-up); a refresh does not.
  - The stamp is the selection's site and plan and its time. Only an outline on that plan whose sites include that site, within 10 s, takes it, and that one scrolls nothing, even if the copy clicked has gone meanwhile. Any other outline (the tool's, an unrelated one, an empty one) leaves the stamp and scrolls its first copy in, per D4.
  - Leaving inspect mode or a `clear` drops the stamp. A report that is not a selection no longer clears another press's stamp. A refused report's stamp expires after 10 s.
  - In part: the theme fixture has no repeated copies, so the e2e still counts `scrollIntoView` calls on one copy (none for the click's outline, one for the tool's). The three-copy feed case was seen in the gate on the clone.

---

- **Medium — [host/web/dev.js:635](/tmp/rv-dt-scroll-r2/host/web/dev.js:635): An unrelated outline consumes the click exception.** Click a later copy of A while its report is pending, then receive the tool’s Outline for B. Any valid same-plan frame clears `clicked`: if B excludes A, A’s eventual selection outline scrolls again; if B includes A, the tool’s Outline incorrectly skips D4’s first-copy scroll. **Fix:** correlate automatic outlines with their originating build and selection request; suppress scrolling only for that response on that page.

- **Medium — [host/web/dev.js:489](/tmp/rv-dt-scroll-r2/host/web/dev.js:489): The marker is neither click-specific nor reliably cancelled.** `refresh()` also calls `select()`, arming suppression without a new click. A refused, dropped, or unjoinable report can leave it armed indefinitely; `clear` and leaving inspect mode do not reset it. After such a selection, clear or exit/re-enter, then explicitly outline its site: the required scroll is skipped. **Fix:** distinguish pointer selections from refreshes and scope pending suppression to a request, cancelling it on failure or inspect-session cancellation.

- **Medium — [host/web/dev.js:637](/tmp/rv-dt-scroll-r2/host/web/dev.js:637): DOM membership cannot establish which response this is.** If asynchronous data removes the clicked row before its outline arrives, remaining copies fail the membership check and the first copy scrolls—the unwanted jump returns. Conversely, an in-place generation reload retains `clicked`; looking up its bare ID in the replacement view map can suppress scrolling for a different element. **Fix:** use generation-qualified request correlation, preserve no-scroll for the matching selection response even after unmount, and discard obsolete-generation responses.

- **Low — [scripts/tamber-server.test.mjs:482](/tmp/rv-dt-scroll-r2/scripts/tamber-server.test.mjs:482): The test proves only the serial case.** Counting calls now catches the old unconditional `scrollIntoView`, unlike round one. But the test selects one visible copy and waits for its outline before requesting the tool’s Outline, so all failures above can pass. **Fix:** test repeated copies with the first offscreen, control response ordering, cover refusal/refresh/clear/reload/unmount, and assert actual scroller offsets and explicit Outline visibility.

Foreign-plan frames correctly return before consuming the marker. A full document reload resets it; the in-place reload does not. Tests were not run under the read-only restriction.

Verdict: **Request changes—response correlation and marker lifetime still allow selection jumps and suppress required tool scrolling.**