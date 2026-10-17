# Code review: LLP 1122 T2, web dev hosts' report order, round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `122c226a5`.
- **Method:** one brief (sha256 `d7edfa0013391eefbba92149d2b8716e1196237dc67e4164c7a9fa760294c5d5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken: only a report gives up after 10 s; a registration waits as long as it takes, so a slow answer no longer leaves the stream registered as failed. Grok's run died after its preamble again (no findings); it was run again for round 3.

---

1. **Medium — Registration timeout permanently suppresses subsequent reports.** [host/web/dev.js:327](/tmp/rv-dt-t2d-r2/host/web/dev.js:327), [host/web-js/dev.mjs:372](/tmp/rv-dt-t2d-r2/host/web-js/dev.mjs:372). The new POST timeout also applies to registration, whose rejection becomes a permanently cached `registered = false`. If the server accepts registration but its response takes over 10 seconds, while EventSource remains connected, the page keeps receiving and applying pins but discards every subsequent status and refusal. Registration runs again only after a new nonce arrives, so recovery requires a reconnect or reload. **Fix:** retry registration while its nonce remains current, or reconnect on registration timeout and report fresh status after registering.

The first-round stream isolation and command-chain fixes otherwise hold. Existing report senders use the queues, ordinary promise rejections are handled, and no caller consumes the resolved return value. There is no `selected` sender yet.

Static inspection only; no tests or file changes.

Verdict: Request changes for registration-timeout recovery.
