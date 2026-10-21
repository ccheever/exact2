# Code review: conform --linux: the Linux reference takes the web's canSharePictures (metronome's main red), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f5605580c1`.
- **Method:** one brief (sha256 `497067410fc67007c8ab7b81767a26ce6b0a68854e40bae152845c4713b57084`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking or should-fix; one nit (the comment's 'the one page fact': navigationType differs too)
- **Disposition:** taken in 4a855e474c

No blocking or should-fix findings. The harness is the right place for this alignment.

- **Nit — [conform.mjs:375](/tmp/rv-share1/host/web-js/conform.mjs:375):** “The one page fact” is inaccurate. `navigationType` also differs: web `"navigate"`, native `""`. Fix the comment to name the picture-sharing difference specifically. Navigation type is intentionally different and cannot be set through `prefer`; no other mutable `exactPage` default needs this alignment.

The requested checks:

- **Spec:** Both hosts are correct. [LLP 1125:1101](/tmp/rv-share1/llp/1125-a-transport-midi-and-the-small-pieces.rfc.md:1101) explicitly requires this split. Linux initialization, runner encoding, web `pageReporter`, and JS `facts.js` agree.
- **Placement/errors:** [conform.mjs:380](/tmp/rv-share1/host/web-js/conform.mjs:380) awaits the override before settlement, steps, or comparison. Failure records `linux-open`, closes `L`, and clears it. Later `prefer can-share-pictures true` still reaches all three hosts; this is an initial value, not a permanent pin.
- **Observable effects:** [runner/page.rs:20](/tmp/rv-share1/runner/src/runner/page.rs:20) recommits affected readers. It produces a page journal entry and rerender, but no mutation write or input event by itself. For metronome, it removes the share button without changing slots. Settlement precedes comparison; generic page journal entries and node IDs aren’t compared. Post-boot alignment would need reconsideration for a future consumer with fact-dependent startup effects.
- **Coverage:** Native `true` behavior is intentionally outside this differential run. The standalone [share corpus smoke](/tmp/rv-share1/scripts/smoke.mjs:1353) remains unaffected. Explicit picture-sharing still has different native/web behavior; setting the fact does not change either executor’s capabilities.

Static review only; no files changed, builds, or tests run.
