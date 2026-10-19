# Code review: LLP 1121 stage 9 G4, an ancestor's press release and a descendant's press, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change and rounds 1-2.
- **Method:** one brief (sha256 `a2a935f226c37f0d276713b6a540c5d2bdfce7d2706b87673101e96268eb300a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: an obsolete lender on release; a background-only press; re-ink against the old style; recycling; duplicates).
- **Disposition:** fixed in round 3's commit.

---

Four blocking issues remain. Below, `A → B → C` denotes ancestry; resting text is white.

1. **blocking — Release restores from an obsolete lender.** [PressPaint.swift:245](/tmp/rv-g4-3/host/apple/Sources/ExactKit/PressPaint.swift:245)  
   Press A yellow, C blue, then B green; release C. C restores white: its remembered lender is A, whose `inkAgain()` stops at B’s own colour. C should inherit green. The two-level order “B down, A down, B up” also fails because B never acquired a lender. Changing A’s colour while B is pressed can similarly erase B’s lender through `endLent`. **Fix:** resolve the currently applicable ancestor when own colour ends, respecting written-colour boundaries. Extend the three-level test at [PressPaintTests.swift:215](/tmp/rv-g4-3/host/apple/tests/ExactKitTests/PressPaintTests.swift:215) through release, and cover reversed order and held releases.

2. **blocking — A background-only press prevents legitimate colour inheritance.** [PressPaint.swift:218](/tmp/rv-g4-3/host/apple/Sources/ExactKit/PressPaint.swift:218)  
   Press A yellow, press C with only `press_background_color`, then press B green. C remains yellow instead of inheriting green. Its saved resting colour matches, but `!node.pressPaint.down` rejects it despite `showsOwnTextColor` being false. **Fix:** permit replacing borrowed text colour on a pressed node that has no own text colour; preserve its background/opacity independently. Add this variant to the three-level tests.

3. **blocking — Colour changes re-ink against the previous style.** [PressPaint.swift:401](/tmp/rv-g4-3/host/apple/Sources/ExactKit/PressPaint.swift:401)  
   With A yellow and B’s background-only press down, add green `press_color` to B through an engine style update. B initially inks C green, but `endInked([])` calls A’s `inkAgain()` before B receives its new style. A therefore passes through B and takes C back to yellow; B finishes green. Conversely, removing B’s press colour leaves B and C white while A remains yellow. **Fix:** reconcile lending after installing the incoming style and updated overlay/rest state. Test adding and removing `press_color` under a down or release-held ancestor.

4. **blocking — Recycling can reapply an old ancestor’s ink to a new node.** [PressPaint.swift:438](/tmp/rv-g4-3/host/apple/Sources/ExactKit/PressPaint.swift:438)  
   Let pressed list A lend yellow to parked row B and its child C. Rebinding B resets its paint and installs its new white style. Rebinding C next calls `endInked`, which invokes A’s `inkAgain()` and paints the already-rebound B yellow again. Moving that reused row to another unpressed list leaves B carrying A’s old colour and lender. This follows the reset-before-create ordering in [NodePoolIOS.swift:549](/tmp/rv-g4-3/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:549). **Fix:** separate retirement cleanup from release restoration: dropping paint should selectively end its loans without re-inking its lender. Add a cross-list reuse regression.

5. **should-fix — `inkAgain()` accumulates duplicate weak entries.** [PressPaint.swift:219](/tmp/rv-g4-3/host/apple/Sources/ExactKit/PressPaint.swift:219)  
   Hold A and repeatedly press/release B. Every release appends A’s eligible descendants again; with two descendants, five cycles grow `inked` from 3 entries to 13. Storage and eventual cleanup grow with interaction history. The ownership guard makes duplicate cleanup idempotent, so duplicates alone do not cause another colour wipe. **Fix:** maintain unique weak entries and test that repeated cycles keep bookkeeping bounded.

The `atRest` predicate itself correctly describes a pristine record; it does not prevent finding 4 from contaminating a newly reset record. Review was read-only, using source traces and an in-memory model; no builds or XCTest runs.
