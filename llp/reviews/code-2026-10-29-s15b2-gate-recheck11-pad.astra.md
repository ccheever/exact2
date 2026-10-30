# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck11-pad, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `699e74d03abd8a230461fc3da2d57d28b1bfe173f0329f375612e0a5685b31bf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (or its tail where the run failed).
- **Verdict:** request changes (2 medium).
- **Disposition:** taken in 8dfddf0646.

## Brief

Narrow re-check of TWO commits, HEAD~1 and HEAD (`git show HEAD~1 HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
The previous re-check (/tmp/iosfix-rv-g12-brief.txt, of the commit naming the first full run's differences) found:
- Astra P2: the iOS menus' sheet names missed UIKit's own sheet frame (10-12k px outside);
- both: new-chat wasn't in the keyboard rule;
- Astra P3 and Grok P2: the video panel name covered every host, though only the Mac's panel grows.
These commits:
- add a named entry's `pad` [top, right, bottom, left] for a frame the platform paints past its container. The menus use UIKit's sheet, 8 each side and 28 above, said in the entry's change. Measured on the run's iOS shots: content roots (8,643,386x223) before and (8,683,386x183) after; differences 0..401 x 618..857.
- add new-chat to KEYBOARD_UP;
- let a named entry name its `hosts`: the video feed is named by its follow button everywhere, and by its bottom panel on the Mac alone.
Check:
- whether a pad is acceptable under theme2's condition (b), "the diff's bounding box must lie inside the container", with the container now padded and the reason stated;
- the pad's numbers against the shots (target/stage12-gate/before|after/ios-light-post-menu.png and .out);
- host filtering through namedOf, namedReport and the shot records;
- the keyboard rule for new-chat (an autofocused search field);
- tests.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P2 — [scripts/stage12-s15b2.mjs:74](/tmp/iosfix-rv-g13/scripts/stage12-s15b2.mjs:74)** (also line 75): The pad still misses UIKit’s bottom paint. The saved `.out` gives post-menu root heights **189.48 / 150.75**, rather than 223 / 183. Mapping the retagged IDs in memory, the padded union ends at **y=834**, leaving the differing pixel **(31,857)** outside. Profile-menu light leaves **36 pixels outside**. Account for the painted bottom using the bounds the gate actually reads.

- **P2 — [scripts/stage12-s15b2.mjs:75](/tmp/iosfix-rv-g13/scripts/stage12-s15b2.mjs:75)** (also line 74): UIKit’s padding applies on every host. On web, the menu starts at **y=12**, but padding expands its name to **y=0**, exempting unrelated pixels above it. An in-memory difference at **(200,5)** is incorrectly accepted. Apply the padding only on iOS, retaining unpadded names elsewhere.

A documented pad representing actual platform paint is compatible with condition (b). Video host filtering works through shot records and `namedReport`; new-chat’s keyboard handling checks out. **Six focused tests passed**, plus read-only reproduction checks.

VERDICT: CHANGES
