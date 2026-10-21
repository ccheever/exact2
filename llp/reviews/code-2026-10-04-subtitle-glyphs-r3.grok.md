# Code review: the title view's subtitle as a line of symbols and text, round 3 (final) (0a1b13944), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at the round-2 artifacts commit.
- **Method:** one brief (sha256 `cf2ec214eb870be7ab26baa4eb767d25b6a9fc96b1c017767f56cbbcdf9aefee`), shared with astra. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** LAND.
- **Disposition:** No findings to take. Landed.

---

**Round 2**

1. **Resolved.** Controls and links are skipped with the same set `passive` uses (`isButton`, `actsAsButton`, `press`) before any avatar or subtitle read, unless the node contains the heading (`NavigationTitleIOS.swift:70-76`). After the heading, a filled box is not entered (`:87-89`).
2. **Resolved.** A `symbol:` image sets `symbols` even when `symbolName` is blank, so the line still returns its texts and the container label (`:57-66`).
3. **Resolved.** The line’s resolved direction is stored, included in `source`, and applied as the paragraph’s base direction (`:84`, `:107`, `:185`). `.natural` when it is not right-to-left matches the host’s existing rule for `ltr`. Per-run bidi isolation stays deferred from round 1.
4. **Resolved in part, deferral acceptable.** Turning the muted line off now requires both `accessibilityValue == nil` and a hidden subtitle (`NavigationBasicsIOSTests.swift:129`). That is the real state: `toggle-subtitle` has already cleared the string, so the line does not come back as "Online". Rendered-color and content-size checks stay deferred.

**Avatar inside the pressable group.** The new `control(child)` skip does not drop it. `HeaderShape` sets `tap` to the pressable that contains the heading (`NavigationBarIOS.swift:93`) and `HeaderTitle` walks that node (`NavigationTitleIOS.swift:93`), so `control` is never applied to the group itself. A nested pressable that still contains the heading fails the `!heading.isDescendant(of:)` test and is entered. The avatar is the filled column inside that group (`app.contract:249-250`): it is not a button, link, or `press`, so the check at `:77` still runs and `BadgeFace` still matches.

No findings.

Verdict: LAND
