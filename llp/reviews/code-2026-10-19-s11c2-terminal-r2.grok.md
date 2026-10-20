# Code review: Stage 11 C2: the terminal held to its column (exact-terminal layout and plan), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `6b9d8def04`.
- **Method:** one brief (sha256 `ca36a05ab97e4010f76aea45b4c8ed908d2441131638a3f3dbe5a96b015c26a4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; one P2 (clamp corners at 0)
- **Disposition:** taken in 61ef26cdd3; its note on a step short of its words taken in the next commit

**Nothing blocking.** The round-1 fixes hold for the terminal column. One ordinary defect:

1. **[P2]** Clamp each resolved corner at zero before reporting it — `host/terminal/src/cli.rs:181`. `layout` reports raw `at_basis`. A host (`resolve_radii`, and button geometry) then applies `.max(0.0)`. A negative literal radius is refused at parse, but a `calc()` or comparison that is still a `Calc` or `Compare` can resolve below zero on one or both axes. The reply then shows a negative length, or a nested pair when only one axis is negative, where the host paints 0. `scale-check` treats a negative number as a real length and a pair as unmeasured, so that corner cannot match the host. The scale fixture's terminal radii are `0`, and the 50% specimen is a square 32 px box, so this does not fail the current gate.

Checked and sound:

- An elliptical corner is valid JSON, a pair inside the radius array (`[[across,down], …]`). `scale-check` rejects it on purpose (`Number.isFinite` is false for an array; the test locks "not four lengths"). Terminal scale radii are lengths, so both axes agree and the reply stays four numbers. A percentage on a non-square box fails closed rather than being scored as one length.
- `at_basis` already resolves `Compare` with the box basis (a percentage with no basis stays a comparison, then `wide` gets the basis) and turns `env()` into points before that match, the same on both axes.
- `terminalVerb`: a step that is only a verb returns that one word (a following `layout` can be swallowed, and the drive errors). Extra spaces stay in the text tail (`type field hello  world`). A verb not in the table stays one word each (`wheel list 3`).
- The even-check message is right for the drift it names (`1/0` on the half-row box). Missing or non-integer cells fail closed; the wording says "not in the reply" for both. The terminal's cells are integers. The QUEUE line matches the painter: 8 px down can drift with the child's height, 16 px down does not.
