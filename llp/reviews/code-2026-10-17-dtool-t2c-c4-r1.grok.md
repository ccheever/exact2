# Code review: LLP 1122 T2c step 4 (Linux answers inspect unsupported), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `09a5435e4`.
- **Method:** one brief (sha256 `856fa5e0f041c92b043bc9865d61fb154aed473ad0c0899d9a580767199b3523`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** approve, with one low finding (the docs' list of hosts).
- **Disposition:** taken. The docs now say a development JS page also answers `inspect` unsupported, until the JS target names the capability. Android and Windows never poll the channel, so they stay off the list.

---

I'll review HEAD as a read-only pass: the Linux inspect-unsupported behavior, its test, and whether the protocol docs match the other hosts.The diff is small. Next I’ll read the Linux channel, the pin test, and how the other hosts answer `inspect`.The Linux path treats `inspect` as reserved. I’ll read the parser, the presenter, and the other hosts’ build channels next.No High or Medium findings. The Linux channel answers `inspect` the way this step specifies, and the pin test locks that answer.

### Low

**`docs/tamber-protocol.md:772`** — The sentence this commit rewrote names only tvOS and Linux as answering `inspect` unsupported. A development JS page also does. It registers as `web-js` with `capabilities: ['pin']` and, on an `inspect`, `outline`, or `hold` frame, posts `{kind: "unsupported", what: <kind>}` (`host/web-js/dev.mjs:428`). The server accepts that report (a missing `detail` is allowed). A client treating line 772 as the full list will not expect the report from a JS build.

Fix: add the JS target to that sentence, as a development build that answers `inspect` unsupported until it names the capability.

What I checked and found sound:

- **Order.** A tick of `pin`, then `inspect {on, seq}`, then `outline` queues the two `unsupported` reports first and `status` after (`host/linux/src/presenter/build_channel.rs:41-52`, asserted at `:304-312`). That is the order the server uses to bind a report that names no revision to the next status.
- **`detail`.** `None` is JSON `null`. The server allows null or a missing `detail` (`scripts/tamber-builds.mjs:77` and `:95`). The new assertion matches the outline report that was already there.
- **Register.** The only registration sends `capabilities: ["pin"]` (`host/linux/src/build_channel.rs:271`), and the reconnect test still requires exactly that (`:565`). The server sends `inspect` and `outline` only to a build that names `inspect`, including the replay on register.
- **`inspect {refresh}` and `outline`.** `Frame::parse` keys only on `kind` (`host/linux/src/build_channel.rs:413`), so `{on, seq}`, `{refresh: true}`, and `{refresh: true, status: true}` are the same `unsupported` report. The test’s bare `outline` frame takes that same arm. Nothing in the parser treats `refresh` as a request to resend status.
- **Live frames and the commands poll.** `is_live` is only `tamber-batch` and `tamber-snapshot` (`host/linux/src/build_channel.rs:202-207`). `poll_development` runs the command step before the live step (`host/linux/src/presenter/delivery.rs:51-90`), and the poll copies every frame through with no kind filter (`host/linux/src/build_channel.rs:301-305`).
- **Android and Windows.** Both boot the Linux presenter, and neither loop calls `poll_development` (`host/linux/src/android.rs`’s frame loop; `host/windows/src/window.rs`’s draw path). They never hello, register, or answer, so the protocol is right to leave them off this sentence. tvOS still has no `inspect` capability and reports any non-`pin` command unsupported (`host/apple/Sources/ExactKit/BuildChannel.swift:55-58` and `:165-168`).

Linux answers an `inspect` command `unsupported` (before `outline`, then `status`), with `detail: null` and no `inspect` capability; the protocol line omits the JS target, which answers the same command.

