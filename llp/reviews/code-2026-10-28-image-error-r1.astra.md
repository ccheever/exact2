# Code review: the image error event on Linux, Android and Windows, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6cabaac038`.
- **Method:** one brief (sha256 `23ba770527bee7230eb53486b32c0f9ee094d4af1ebbb845259b3a6e1b34d71c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/image/svg.rs:188](host/linux/src/image/svg.rs:188) — **SVG diagnostics still leak signed URLs.** Workers pass the original source into `svg::decode`; parsing failures and warnings print it before the sanitized worker log. Apple’s [package-admission log](host/apple/Sources/ExactKit/RasterLoader.swift:622) also prints the raw source. Redact these diagnostic lines, including URLs embedded in diagnostic text.

2. [host/linux/src/image/remote.rs:26](host/linux/src/image/remote.rs:26) — **Redaction misses rejected URLs without `://`.** For example, `Location: //bad:port/a.png?hm=secret#fragment` fails redirect parsing and is interpolated into `Failure.message` unchanged. The helper leaves its query and fragment in both the event and stderr. Sanitize known URL fields before formatting, including malformed and relative references. [Apple’s helper](host/apple/Sources/ExactKit/RasterRetry.swift:82) has the same limitation.

3. [host/linux/src/image.rs:634](host/linux/src/image.rs:634) — **Off-screen cleanup loses permanent decode failures.** If an image leaves the viewport while decoding, this branch cancels a completed failed request before the error handling at line 672 sees it. The sync cleanup at line 435 can do the same. The assignment never errs while hidden and can decode again when shown. Process final failures before off-screen cleanup in both paths.

4. [host/linux/src/image.rs:798](host/linux/src/image.rs:798) — **Terminal request-admission refusals never settle.** This branch only records the refusal and resubmits on later polls. A concrete case is `SubscriberLimit` when replacing a source at the subscription cap while retaining its old pixels. Unlike Apple, no `error` reaches the handler. Settle non-passing request refusals here; preserve temporary queue waits.

5. [host/linux/src/image.rs:453](host/linux/src/image.rs:453) — **An oversized source fails silently.** The length rejection clears `view.source` and skips assignment bookkeeping. `poll` subsequently skips the empty source, so a 4,097-byte source never produces its final error. Track the rejected assignment and queue one checked invalid-source error while retaining the previous picture.

**Non-blocking**

1. [host/linux/src/image.rs:140](host/linux/src/image.rs:140) — `Shutdown` is terminal, so it does not belong in `passing`. Current source/plan call sites cannot return it, making this latent rather than an active failure.

2. [host/linux/src/image/package.rs:185](host/linux/src/image/package.rs:185) — Package pictures still discard failure messages without queuing an event. This predates HEAD, but the guide’s broader promise should distinguish that existing gap. Package change/revocation semantics need their own handling.

3. [host/linux/src/presenter/images_tests.rs:468](host/linux/src/presenter/images_tests.rs:468) — Positive assertions depend on fixed 300–1,500 ms runs. Wait for the expected event or request count with a generous hang timeout; these durations can fail under scheduler contention.

Read-only review; no tests, builds, or network access.
