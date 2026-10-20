# Code review: stage 4 commit 5: the build channel, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the whole range, the second-round fixes first.
- **Method:** one brief (sha256 `b062928ca6b6c8cdd013071270bec6321f5baebc23d9433e4db195e5ff8b723d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a hostile report ending the loop; connected builds bypassing the cap; unbounded polled queues; a failed poll answer losing commands; Linux reverting a live design during a reload; web pins over the app's setScheme; status missing the app's own changes).
- **Disposition:** applied without re-review (the last round) in the third-round commit.

---

Blocking findings remain.

- **P1 — A LAN peer can crash the dev loop.** [scripts/tamber-builds.mjs:168](/tmp/rv-s4-8/scripts/tamber-builds.mjs:168): obtain a nonce, register, then report `{"kind":"unsupported","what":{"toString":null,"valueOf":null}}`. String coercion throws through `serveReport`. Reproduced Bun exiting with code 1.

- **P1 — Connected builds bypass the 256-build cap.** [scripts/tamber-builds.mjs:152](/tmp/rv-s4-8/scripts/tamber-builds.mjs:152): eviction only removes disconnected builds. An unauthenticated peer can hold SSE streams and register distinct IDs indefinitely, growing retained state and event fanout. Reproduced 300 connected builds.

- **P1 — Polled command queues remain unbounded.** [scripts/tamber-builds.mjs:105](/tmp/rv-s4-8/scripts/tamber-builds.mjs:105): repeated registrations enqueue a pin each time. A peer polling within five seconds can enqueue faster than the bounded answers drain. Reproduced 10,000 retained frames on one nonce; the response cap does not bound memory.

- **P2 — Failed poll responses permanently lose commands.** [scripts/tamber-builds.mjs:267](/tmp/rv-s4-8/scripts/tamber-builds.mjs:267), [host/linux/src/build_channel.rs:291](/tmp/rv-s4-8/host/linux/src/build_channel.rs:291): after the server drains a poll, a connection reset loses its frames. Both clients retain the nonce and retry an empty queue, without replay or resynchronization. Reproduced a lost pin while the build remained registered.

- **P2 — Linux can revert a newer live design during reload.** [host/linux/src/presenter/delivery.rs:45](/tmp/rv-s4-8/host/linux/src/presenter/delivery.rs:45): while a generation containing D0 downloads, the independent channel can apply D1 to the running plan. Installing that generation restores D0, but requests no snapshot afterward. D1 remains lost until another edit or reconnect.

- **P2 — Web pins override the app’s `setScheme`.** [host/web/dev.js:355](/tmp/rv-s4-8/host/web/dev.js:355), [host/web-js/dev.mjs:239](/tmp/rv-s4-8/host/web-js/dev.mjs:239): an app that selected light becomes dark when a dark pin arrives, because both write the same inline property. Reproduced; this contradicts the precedence ruling.

- **P2 — Status misses app-driven changes.** [host/web/dev.js:349](/tmp/rv-s4-8/host/web/dev.js:349), [host/web-js/dev.mjs:238](/tmp/rv-s4-8/host/web-js/dev.mjs:238): app calls to `setScheme`, `setMode`, or `setRootFontSize` do not trigger reporting. The tool retains old facts until an unrelated channel/media event. Apple similarly misses app mode/font changes through its [preference-only notification](/tmp/rv-s4-8/host/apple/Sources/ExactKit/Session.swift:1280).

Seven unit tests passed; two socket tests could not bind here. Worktree and index unchanged.
