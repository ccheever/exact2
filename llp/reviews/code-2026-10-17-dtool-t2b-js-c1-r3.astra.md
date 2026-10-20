# Code review: LLP 1122 T2b, the JS target, step 1 (the page's plan digest and its map), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `24efee359`.
- **Method:** one brief (sha256 `2a6bb3c4e28331a659cc2bada1335cb58b99169f9cb2580b852e5a62568a0c22`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 1 low).
- **Disposition:** taken (the last round). A reload certifies its design only when the shape held: then the natives on the resident producer take it as a batch. A shape change (an axis, a role, or a reordered value table with the same digest) leaves `designServed` null, so a build left on its old plan stays read-only. The T2 test's poll beside the drive is stopped and awaited in `finally`, before the tool goes.

---

- **Medium — A shape-changing reload can certify an older build as editable.** [host/web-js/dev.mjs:376](/tmp/rv-dt-js1r3/host/web-js/dev.mjs:376). The round-2 fix sets `served` before checking `shaped`, including when no compatible snapshot or batch is sent. The design digest hashes token names and values, excluding axes ([runner/src/design.rs:136](/tmp/rv-dt-js1r3/runner/src/design.rs:136)). For example, change `variant = a | b` to `b | a`, and change a token’s conditional blue value from `when variant=b` to `when variant=a`. Its ordered value table—and design digest—can remain identical while its shape changes. A native still running the previous plan shows red for `variant=a`, but [designMoved](/tmp/rv-dt-js1r3/scripts/tamber-inspect.mjs:98) accepts the new certificate and offers editing against the file where that branch is blue. **Fix:** bind live certification to compatible plan/shape identity. Shape-changing reloads must not certify older plans; preserve certification for compatible reloads such as the accent case from round 2.

- **Low — The new test poll is not cleaned up on failure.** [scripts/design-tool.test.mjs:735](/tmp/rv-dt-js1r3/scripts/design-tool.test.mjs:735). If browser launch or `drive()` fails before registration, execution skips `driving = false` and awaiting `saidPlan`. Cleanup stops the tool while the poll remains active; its token fetch is outside the attached `.catch()`, so it can reject unhandled after the test has failed. **Fix:** stop and await the poll in `finally`, before stopping the tool, and handle or abort both requests.

The round-1 fixes otherwise hold: fast edits hash their compiler input, full builds use captured source hashes, failed startup caches the preserved build, and the test observes registration while the page is open. Map retention covers normal and overtaken publications; kept builds and scrub previews retain the running plan’s map. The digest matches the encoded plan used by the compiler and map. I found no request-writable map source or no-digest fallback defect.

The test meaningfully checks initial digest reporting and the on-disk map’s digest. It does not exercise cached-map lookup, retention, or `designServed`; those remain deferred coverage. No builds or tests were run.

**Verdict: Request changes—round 2’s reload fix still permits incorrect editability across design-shape changes.**
