# Code review: setRootFontSize("125%"), round 2, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `b5bb95021d3c6f1b12ad1bc544f455b143341d781ad04fdf86980eaefad32c3a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: fix (one MINOR); Grok: ship.
- **Disposition:** the JS target computes the ratio first, as the runner does, with a test at hosts 18 and 24

---

**MINOR — [host/web/navigation.js:876](/Users/admin/x2p2/apps/exact2-comp/host/web/navigation.js:876): arithmetic order still breaks runner parity.** JS computes `host * n / 100`; the runner computes `host * (n / 100)`. Reproduced through `appRootFontSize`:

- Host **18px**, `"3.8924957342356035e-45%"`: JS overwrites the active percentage; the runner’s calculation rounds to zero and rejects it.
- Host **24px**, `"1.4178431532488902e+39%"`: JS rejects it; the runner’s calculation remains finite.

Compute the ratio first, then validate `Math.fround(host * ratio)`. Add a non-default-host boundary test.

The parser fix, added state-preservation assertions, and existing rollback snapshot look correct. Both focused JS tests passed. Rust tests weren’t rebuilt under the read-only restriction.

VERDICT: fix (MINOR with host/web/navigation.js:876)
