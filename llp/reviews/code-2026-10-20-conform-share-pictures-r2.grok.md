# Code review: conform --linux: the Linux reference takes the web's canSharePictures (metronome's main red), round 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `4a855e474c`.
- **Method:** one brief (sha256 `1614d56882dd35b8f0009e9e155955e157ecb413774d6c61e9b4723d6340fe0c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking, should-fix or nit
- **Disposition:** none needed

**No blocking, should-fix, or nit.**

The final state in `host/web-js/conform.mjs` is correct. Right after a successful Linux open, and before settle or the boot compare, `conform --linux` runs `prefer can-share-pictures false` on that host only. That matches LLP 1125 D12: under the agent the web substitute keeps `canSharePictures` false, and the Linux substitute sets it true. `prefer` starts from the current page and flips only that field. A failure is recorded as `linux-open` and the Linux reference is closed, so later compares do not run against the unaligned host.

Of the eight `exactPage` fields, the agent defaults differ on two. `canSharePictures` is the one this aligns. `navigationType` is `"navigate"` on the web agent and `""` on a native launch; it is not a `prefer` fact, it is not in the compared state keys, and no conformed plan declares or renders it. The other six (`visibilityState`, `onLine`, `canShare`, `canShareFiles`, `canOpenFiles`, `hasFocus`) already match. The comment states that, including that `navigationType` is rightly left out.
DONE
