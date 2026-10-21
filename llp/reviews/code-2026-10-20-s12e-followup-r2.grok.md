# Code review: Stage 12 E follow-up: PostButton's pressed and dim params, the packages' sizes and type as tokens, neutral colours as roles (LLP 1139 §5 E), round 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `41ef1e5e02`.
- **Method:** one brief (sha256 `3bc70b10589ef56024bed18531e9494c00b91f18da3b0fb63c59013d9a09740a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no findings
- **Disposition:** none needed

No findings.

The commit is consistent end to end. Every new length token is the old literal and is named by a row that reaches the paint: icon and count sizes on the action row, avatar and thread sizes, the reply nudge, type sizes, and `cards.hairline` on the divider, the link frame and split, and the avatar ring. Neutral colours are platform roles; brand colours stay hex. `cards.avatar-ring` stays a translucent `light-dark` pair, and the comment says why.

`PostButtonLook`'s `:active` reads `param.pressed` and its `:disabled` reads `param.dim`. An unread `pressed` is `tamber-param-unused` inside `compile_path`, so the test's unwrap fails. A fill/dim swap fails the two computed backgrounds: enabled `compose-post-on` expects `ColorValue(Fixed(Color(4278190335)))` (`0xFF0000FF`, `#ff0000`) and disabled `compose-post-off` expects `ColorValue(Fixed(Color(16711935)))` (`0x00FF00FF`, `#00ff00`). The plan row for the pressed param is `#0000ffff`. The `Dir` guard is built before the temp directory is cleared.

`cards.avatar-corner` of `999px` matches the old `size/2` on these squares. CSS, Apple's `BorderPaint.reduced`, and the Linux painter's `border::reduced` scale two `999px` corners by `side/1998`, which is exactly half of 42 and of 32 in both `f32` and `f64`, inside Apple's half-side check. Android replays those already-reduced radii. The terminal rounds any radius above 0 (`host/terminal/src/paint.rs:400`), so `999px` rounds and `radius.full`'s terminal 0 would not. The corner comment matches that.
