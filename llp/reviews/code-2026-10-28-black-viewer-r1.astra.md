# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b7b9ea0fe7`.
- **Method:** one brief (sha256 `1cf1d4e2359375f45a2ba83c3eccb2d5f1ee677dfbb2fd95d196a2c0b8be2a69`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Shared-source eviction strands other views** — [host/linux/src/image/workers.rs:252](host/linux/src/image/workers.rs:252). If visible A and offscreen B share a failed source, A’s retry removes its ID from the backend, but B retains that ID without a retry deadline. When B returns, `prepared()` returns `None` forever; B neither reacquires the source nor schedules a retry. Fix by rebinding every affected consumer, or separating eviction from name lookup from the identity lookup existing owners need. Cover two views sharing a URL, with one hidden during recovery.

2. **Some source changes leave a deadline that wakes forever** — [host/linux/src/image.rs:375](host/linux/src/image.rs:375). The retry reset is bypassed by the earlier symbol, package-source, and oversized-source branches. Change a failed URL awaiting retry to `symbol:close`: `poll()` skips it, but `retry_due_in()` eventually returns zero permanently. The presenter repeatedly marks itself dirty and schedules immediate turns. Reset retry state before those branches whenever the source changes, and ensure non-retryable views cannot retain deadlines.

3. **Android preload visibility violates the offscreen stop condition** — [host/linux/src/image.rs:480](host/linux/src/image.rs:480), [presenter/images.rs:133](host/linux/src/presenter/images.rs:133). Android defaults `EXACT_PICTURES_AT_MOUNT` to enabled, making every mounted image “visible,” including retained offscreen rows. Their retries continue after scrolling away and their budgets do not reset on return. Separately, the urgency calculation at line 456 does not check visibility at all. Keep actual viewport visibility separate from preload demand; use actual visibility for retries and urgency.

4. **Existing downloads never gain viewer priority or the longer bound** — [host/linux/src/image.rs:454](host/linux/src/image.rs:454), [image/workers.rs:162](host/linux/src/image/workers.rs:162). Urgency is considered only when acquiring a source, and an existing source is returned unchanged. A URL queued by the transcript remains ordinary when opened fullscreen; a box growing across the threshold has the same problem. Promote existing queued/in-flight loads when qualifying demand appears. Anchor any extended deadline to the original ask, so repeated size changes cannot renew it indefinitely.

5. **Front insertion does not ensure freedom from starvation** — [host/linux/src/image/remote.rs:251](host/linux/src/image/remote.rs:251). Continuing urgent arrivals can take every newly available worker while ordinary requests expire in the queue without being fetched. Individual download bounds and six retries per showing do not bound newly arriving urgent work. Add bounded urgent dispatch streaks, aging, or reserved ordinary capacity, and test ordinary-request progress under sustained urgent arrivals.

**Non-blocking**

1. **The new tests establish less than their descriptions claim** — [image/remote_tests.rs:580](host/linux/src/image/remote_tests.rs:580). The fixed three-second loop checks the retry counter, not seven actual requests; it hides the image only after exhaustion and never shows it again. The priority test checks queue contents, not dispatch fairness or the urgent timeout. Use observed milestones with a hang bound, count requests, and cover hiding/removal during backoff, reappearance, and both timeout policies. [viewer.test.contract:108](apps/excord/viewer.test.contract:108) checks node presence but does not establish visible fallback pixels while the original stalls.

2. **Settlement and clock semantics need clarification** — [image.rs:691](host/linux/src/image.rs:691). A scheduled retry currently counts as no pending work, so `Images::wait()` returns after the first transient failure. Count visible scheduled retries if this API should wait through recovery, retaining its caller-supplied timeout. Backoff uses real `Instant`, so advancing the agent clock cannot exercise it. Also, [hatch.rs:565](host/linux/src/presenter/hatch.rs:565) adds a current wall-clock remainder to the host’s **last** clock value, allowing early desktop wakes; convert against the loop’s current timestamp.

3. **The worker lifetime bound has an existing DNS exception** — [rustls_http.rs:217](vendor/ibex/crates/ibex2/src/transport/rustls_http.rs:217). Resolution blocks synchronously and checks cancellation afterward. The image clock can publish timeout without freeing that download thread. This predates these commits, but the claimed hard bound on worker occupancy needs this qualification.

4. **The permanent thumbnail underlay changes translucent images** — [viewer.contract:125](apps/excord/viewer.contract:125). It remains after full-image success: matching 50%-opaque pixels composite to 75% opacity, and a late thumbnail can change an already displayed translucent original. Retire the fallback on successful full-image presentation while retaining it on failure.

The full image retains its iOS zoom ID, and the thumbnail’s `aria-hidden` is appropriate. Both size comparisons use device pixels. The listed HTTP classifications and six backoff steps are implemented as described; normal source changes and node removal clear their view state.

Static review only; no files edited, network accessed, tests run, or builds run.
