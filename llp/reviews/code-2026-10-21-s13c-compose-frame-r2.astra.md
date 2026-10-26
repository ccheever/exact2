# Code review: @exact/actions ComposeFrame (stage 13 C piece 3), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5648b6471b`.
- **Method:** one brief (sha256 `c78145f184f3883312f3152e0ec3032101a2ed6e8031948b73be0092fe22a518`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** both nits taken in dc1a178f97

VERDICT: RIGHT WITH NITS

1. **nit — [packages/actions/compose-frame.contract:152](/tmp/iosfix-rv-cf2/packages/actions/compose-frame.contract:152):** The language selector’s fixed `aria-label` overrides `langLabel`, so its accessible name omits the selected language. Include `langLabel`, for example “Post language: English”.

2. **nit — [apps/library-gate/app.contract:108](/tmp/iosfix-rv-cf2/apps/library-gate/app.contract:108):** The reply sample sets `drafts=false`, so it cannot exercise reply-specific Drafts suppression. Use `drafts=true, reply=true` and verify Drafts is absent from the projected bar.

All four round-1 fixes check out statically. The root slot is valid under D2/D4: placed once, unconditional, unforwarded, with no component-owned ancestor that could hide it. D5 explicitly permits no slot-bearing part. Source-based slot discovery and the revised variant cells are consistent.

Inlining introduces no wrapper. iOS gets the required header/scroller adjacency when the fill starts with the screen’s scroller; macOS and web retain their existing header handling. No additional host regression found.

For a positive limit, negative counts produce an empty arc, equality produces a full normal ring, and counts above twice the limit produce a saturated red pie. Both denominators remain positive for zero/negative limits. `countInk`, the Counter group, tabular digits, and decorative SVG hiding are correct.

Static review only; no files changed or builds/tests run.
