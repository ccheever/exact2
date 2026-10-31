# Code review: the web look's popover margin, continuous corners and popover edge, adversarial (sol)

- **Family:** OpenAI. GPT-6.1 Sol (`gpt-6.1-sol`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `ca47dfedff` (4 commits on `5ba900e41c`).
- **Method:** one brief (sha256 `620581102928e294302250e87428db826c322846a454d06828512ee6313781e5`), one reviewer. The author (Claude, Opus 5.5) is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths.

```text
ca47dfedff Unsaid: a popover its author leaves unsaid
fa3f967837 Web: a placed popover keeps clear of the window's edge
46d16c43ec Web: the platform's corners are Apple's continuous curve
5131b83549 Web: a popover of the author's own content keeps it off its edges
```

**Blocking**

1. **46d16c43ec — Sidebar tabs override authored corner shapes.** [host/web/index.html:250](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt2/host/web/index.html:250). On an engine supporting `corner-shape`, a sidebar tab authored with `corner-shape="bevel"` or `"round"` now gets `superellipse(1.6)`. The new declaration is `!important`, so it beats both the wasm target’s inline declaration and the JS target’s generated class. The selector also reaches custom tabs without `data-button-style`. Previously their authored shape could apply. **Established by reading both lowerings and reasoning through the cascade.**

2. **fa3f967837 — The invoker margin shifts author-placed popovers.** [host/web/neutral.css:428](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt2/host/web/neutral.css:428). The selector includes popovers with authored `left`/`top`. Their override at line 430 disables `position-area` but retains the new margin. In RTL, `left:40px` with no authored margin consequently places the border at **56px**. In LTR, a popover flipped leftward by `--exact-inset-start` ends **16px before** its former position. Context menus escape this through `margin:0`; author-placed popovers do not. The existing press-point test also supplies `margin:0`, masking the regression. **Established by reading and [positioning-spec reasoning](https://www.w3.org/TR/css-position-3/#abspos-layout), not browser execution.**

3. **fa3f967837 — Mixed-direction content puts the clearance on the wrong side.** [host/web/neutral.css:428](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt2/host/web/neutral.css:428). With an LTR document and an RTL popover, `span-inline-end` selects the viewport’s rightward area, while `margin-inline-end:16px` becomes the popover’s **left** margin. A long popover can still touch the right edge, while gaining an unwanted gap beside its invoker. **Established by source and [the specification’s distinction between containing-block and self directions](https://www.w3.org/TR/css-anchor-position-1/#position-area).**

**Non-blocking**

1. **46d16c43ec — The image-only exception also reaches non-capsules.** [host/web/index.html:216](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt2/host/web/index.html:216). An image-only native button with `appearance="auto"` and an authored `border-radius=8` keeps that radius but receives `corner-shape:round`. An equivalent text button keeps the continuous curve. This contradicts LLP 1077 D1’s new statement that an authored radius retains the curve: the capsule exception does not exclude authored radii. **Established by reading selectors and lowering rules.**

2. **fa3f967837 — The edge test can report success without testing placement.** [host/web/tests/neutral-look.test.mjs:498](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt2/host/web/tests/neutral-look.test.mjs:498). Unsupported anchor positioning returns `true`, indistinguishably from a successful assertion. Otherwise, the test checks only `innerWidth - right >= 15.5`; it never checks visibility, invoker alignment, the left edge, RTL or flipped placement. A misplaced surface entirely left of the viewport would pass. **Established by reading the test.**

I could not execute browser or native visual checks, including small-size corners and forced colors. The attempted static tests stopped during module loading because `rolldown/utils` was unavailable in this worktree; the read-only sandbox also prevents creating browser profiles. No files were changed.