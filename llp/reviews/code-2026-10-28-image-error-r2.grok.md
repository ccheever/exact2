# Code review: the image error event on Linux, Android and Windows, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `d788b85214`.
- **Method:** one brief (sha256 `194f1ec2ac61aa9f2861b14459579302f0e8716fd0b9b8926c50f6be71d313dc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/image.rs:475` (hash kept across the symbol return at `:412` and the package return at `:437`; `rejected` is cleared on the fall-through at `:490`). `rejected` survives a source that leaves through `symbol:` or a package picture, and `settled` / `errored` stay set. Over-long A, then a symbol or a package picture, then the same A: the hash matches, `assignment` does not move, and no second `error` is pushed. The symbol branch has already dropped the bitmap, so the image is empty and that assignment’s failure is never delivered. A new assignment of that source has to error again. Clear `rejected` on those returns, whenever the kernel source is no longer that over-long string, so the next observation increments `assignment` and pushes the event.

2. `host/linux/src/image.rs:464` and `:537`. The over-long path clears `view.source` so the string is not stored. `Images::renew` (LLP 1078, from `Presenter::renew` before `sync_images`) keeps the picture when `view.source` equals the kernel source. An unchanged over-long source compares `""` with the real string, so `renew` removes the view, drops the bitmap, and clears the intrinsic. The next sync builds a new view with `rejected` empty and sends `error` again for the same source. In `renew`, treat an empty `view.source` whose `rejected` is `Some(hash(kernel source))` as that same source and keep the picture. Leave the string unstored.

**Non-blocking**

- Package pictures on Linux still send no `load` or `error` (unchanged from before this work).
- A missing bundled file still says `decode failed`. Apple says the source names no file this host loads.
- Apple refuses a non-`data:` source it will not acquire as `invalid raster source` (`RasterLoader.swift:644`). Linux says `a source over 4096 bytes`.
- The redirect tests check that `hm=` and `ex=` are absent. They do not check that the host and path remain.
- A redirect `Location` with userinfo is still quoted with that userinfo. The cut is at `?` and `#` (`remote.rs:500`).
- An SVG warning body is still logged as written: a dropped stylesheet selector in full, a style value clipped at 40 characters (`svg.rs:198`, `lower.rs:174`, `lower.rs:400`). A short `url(…?…)` inside the file can still show a query. The source name on that line is stripped.
