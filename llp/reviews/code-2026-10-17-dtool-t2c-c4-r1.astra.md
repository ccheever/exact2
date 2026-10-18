# Code review: LLP 1122 T2c step 4 (Linux answers inspect unsupported), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `09a5435e4`.
- **Method:** one brief (sha256 `856fa5e0f041c92b043bc9865d61fb154aed473ad0c0899d9a580767199b3523`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve, with one low finding (the docs' list of hosts).
- **Disposition:** taken. The docs now say a development JS page also answers `inspect` unsupported, until the JS target names the capability. Android and Windows never poll the channel, so they stay off the list.
  - Not as said: the test is not runnable on macOS. `host/linux/src/lib.rs` compiles the presenter module only for `target_os = "linux"`, so `cargo test -p exact-linux` here filters it out.

---

No High or Medium findings.

- **Low — [docs/tamber-protocol.md:772](/tmp/rv-dt-linux/docs/tamber-protocol.md:772):** The unsupported-host description is incomplete. Web JS also refuses inspect. Android’s agent path and Windows’s headless path can use the shared Linux channel and return the same refusal. Their normal display loops do not poll it. **Fix:** document these distinctions rather than implying a complete platform list.

The Linux behavior and assertion order are correct: inspect refusal, outline refusal, then pin status; both refusals contain `detail: null`. Registration advertises only `["pin"]`. Inspect on/off/refresh and outline all reach the reserved-command branch; live-frame filtering and commands polling do not divert them.

The commit message’s “Linux-only (cfg)” explanation for skipping validation is incorrect: [the test module](/tmp/rv-dt-linux/host/linux/src/presenter/build_channel.rs:252) has only `#[cfg(test)]`. Run the focused test on macOS; I did not execute it under this read-only review.

**Verdict: approve with a documentation clarification; runtime behavior and test expectations are correct by inspection.**
