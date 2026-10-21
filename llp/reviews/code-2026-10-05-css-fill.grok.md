# Code review: an ended CSS animation without a forwards fill stops holding its property (Apple, agent clock), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `180e9494b3304c9914ba6a36bcd81ec3f7a19dd7cebbd14ee27ee5e65e8d61db`), shared with astra. Round 1, blind to the other review. Reviewed the uncommitted diff in a worktree at origin/main. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** Nothing to take.

---

**Verdict: READY.** The held-clock early return in `CssAnimations.make` matches CSS fill, and nothing else that calls `make` still needs the old last-frame hold.

The change is at `host/apple/Sources/ExactKit/SvgScene.swift:177`: once `active >= total` and fill is not forwards or both, `make` returns nil. Fill none (`0`) and backwards (`2`) drop; forwards (`1`) and both (`3`) still pause just inside the end and keep the last frame. `active` is local time minus delay, and `total` is `duration * iteration count` (`infinity` when `n < 0`), so a delay still shows the first keyframe for backwards and both, and a live infinite run never takes this return. Direction is already folded into the track, so returning nil does not depend on alternate.

`apply` hashes the spec (including `h` and fill) plus the clock, and removes the key before `make`. An ended animation is cleared, and a seek back into the interval installs it again. The offscreen path shares that return and draws the underlying value. The held branch is in the shared Apple file, so macOS gets the same fix. The new test lives in the existing ExactKitTests directory and covers both the `h` path (`clock` nil) and `apply` removing the animation.

No bug, suggestion, or nit.

- **Mode**: local uncommitted changes (base named `origin/main`; review is the working tree only)
- **Files reviewed**: 2 — `host/apple/Sources/ExactKit/SvgScene.swift`, `host/apple/tests/ExactKitTests/CssAnimationFillIOSTests.swift`
- **Diff stats**: 1 file changed, 4 insertions, 1 deletion; 1 untracked file
- **Issues**: 0 bugs, 0 suggestions, 0 nits
- **Review**: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-ce82e894.md`
- **Summary**: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-ce82e894.md`
