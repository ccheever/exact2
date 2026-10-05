# Code review: the title view's avatar at its authored size, round 3 (final) (5e549f132), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at the round-2 artifacts commit.
- **Method:** one brief (sha256 `5d15797dd7c367e5750a391c1975307273b2a7ed86a39f39e4965b83be773120`), shared with astra. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** LAND.
- **Disposition:** No findings. Landed with astra's round-3 fix (childless boxes excluded).

---

**Grok 1 (Should-fix, one-child gate) is resolved.** `face_box_size` no longer reads children. A filled point-sized view keeps `faceBoxSize` with any child count, including initials beside a status dot. `BadgeFace` still decides which of those boxes is a title avatar.

**Grok 2 (Nit, `expect`) is resolved for the part the disposition took.** Each of the avatar, the transparent box, and the unstyled box must have a create props object. The wider transition list (box outside the header, fill loss, `when` swap, 40-to-32) stays uncovered and is acceptable to leave. Do not add the suggested second-child clear: a second child must keep `40x40`.

**Grok 3 (Nit, protected native region) stays acceptably deferred.** `create` and `update` still return before the helper for a protected id (`paragraph.rs:255` and `:322`), and the candidate still builds props with `props_for` only (`content_region/host.rs:398`). A title avatar is not authored inside that region.

**Astra 1 (Should-fix, `currentcolor`) is resolved in the code.** `background_color == None` is the `currentcolor` keyword, and that path now uses `text_color()` before the alpha test. An omitted `background-color` is not that keyword. The schema default is `Some(Color(0))`, transparent, so the new unstyled box correctly gets no prop. The style projection’s `unwrap_or` does the same thing: it fills only `None`.

**Astra 2 (Should-fix, child-list allocation) is resolved.** The count check is gone, so a touched column no longer allocates its child vector on this path.

**Astra 3 (Nit, transitions and UIKit) is the partial take already recorded.** `expect`, transparent, and unstyled are in. UIKit resize and hide/restore stay deferred.

**How many more nodes carry the prop.** The gate is now: a `View`, both axes `Dimension::Points`, and a fill whose light or dark alpha is non-zero. Unstyled views stay out. Buttons stay out (`NodeType::Control`). Images and text stay out. What joins the one-child avatars from round 2 is every other filled, point-sized view: zero children (the inbox unread dot at `apps/messages/app.contract:277`, the three typing dots and two tail bubbles in `bubble.contract`, the composer waveform bars) and two or more children (the typing bubble’s `57.5×35` row, and an avatar that also holds a dot). That is a handful per screen, plus one dot per inbox row. Each create gains about 22 bytes (`"faceBoxSize":"8.5x8.5"`), kept in the Rust mirror and the Swift props dictionary. An unchanged value is not put in `set` or `clear` (`paragraph.rs:336`). The only reader is still `BadgeFace(box:authored: true)` (`NavigationTitleIOS.swift:77`). A box with no text or symbol descendant cannot become the avatar, so those extra fields do not change the bar. A multi-child box that already qualified in Swift now draws at its authored size, clamped to 20–44, instead of 36. That is the fix.

1. **Nit — The comment describes the keyword as an omitted background.** `host/apple/src/paragraph.rs:51`. `None` is authored `currentcolor`. The initial value is transparent `Color(0)`, which is why the unstyled box in the test has no `faceBoxSize`. The comment says the opposite, and the commit message repeats it.
   **Fix:** Say that `None` is the `currentcolor` keyword and is resolved with `text_color()` before the alpha test, and that an unset background stays the transparent initial value.

2. **Nit — Neither new positive path is locked, and the test header still requires one child.** `host/apple/tests/it/header_box.rs:1`, `host/apple/tests/it/header_box.rs:32`. The avatar still has one child and an explicit hex fill. Restoring `children().len() != 1`, or going back to `is_some_and`, leaves this test green. The transparent and unstyled assertions already passed on the parent.
   **Fix:** Add a `40×40` box with a text and a dot, and a `40×40` box with `background-color="currentcolor"` (inherited `color` on the parent), and assert `40x40` on both. Change the module comment so it no longer requires one child.

Verdict: LAND
