**Messages iPhone parity** (`apps/messages`): the local chat example now runs on
web and iOS. Finish native back/Tapback/reply gestures, timestamp motion matching (recognition threshold and release curve; resisted/capped travel and stationary labels now work; extent/rate/paging probes rejected, a linked Exact-engine spring now measures 1.14-point RMS error versus 7.70 for scroll snap, with physical vertical/reversal checks; R4 reuses current editor exclusion and identity-bound Presenter.dragX: physical read-only selection moves rows 0 points versus 44, and Back-during-hold emits 0 gone-target writes versus 11. Of three additional attempts approved by Charlie on 2026-09-11, R5 fails compilation on unsupported inset() clip-path; R6 margins/overflow hide rest labels with 23 native rectangles unchanged, ten physical assertions passing, zero retired writes and browser geometry verified (`/tmp/messages-timestamp-ownership/r6/`). All three additional attempts are consumed; authored intent and browser ownership still need integration),
anchor-removal fallback and typing/insertion motion, then compare actual touch
behavior and screenshots with Messages.
`apps/messages/README.md` records the current implementation gaps.
The focused reply surface, swipe entry, and reply indicator are in place; finish thread positioning,
and connecting lines. Transparent curved tails now work over its material.

*Filed under “Later”.*
