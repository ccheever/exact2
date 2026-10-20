# Code review: LLP 1122 T4 step 1, the Hold control, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `025317cbf9`.
- **Method:** one brief (sha256 `6116c28a0e52dd8d21b43ec5dfe1329d4c00357f74db9566f068559112a73b7a`), shared by both reviewers, blind to the other review; commits 1f9f9766a 493b45ac2 ab3c809bed. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium; smaller points).
- **Disposition:** taken in `d736c267b4`, with the server's half in `d058feff01`: the server forgets a build's `held` report when it sends another hold, so a listing's `held` is its hold's and the tool takes it as given (successive listings and a resync both right); a new hold, anyone's, ends the last word, a let-go only a listing says (its event missed) gives its reason, and a refused hold keeps it; the held rows are asked of the page whenever the state held here is not the one the selection reports (none, or an earlier hold's), once a hold, the ask forgotten when the hold goes or the server restarts, and 'Its page is asked for the held look's rows.' is said until a selection answers (then 'Provenance incomplete' if it still has none); the press's own node selected is where it is held (its report's `site`), so its rows show and Let go works there; the owner line reads only this hold's report. The stub's refresh keeps inspect mode and answers with a held look a test sets; the test drives a live replacement (Pressed to Hovered, its rows through a refresh), a listing-only let-go and another client's hold. Not taken: a hold ended because its client went says nothing (the server sends no event and the listing no reason).

The refresh bug blocks this step. The round 1 confirmation, wording, and stale-selection fixes hold; a later hold on the same selection never loads that hold's rows.

## Blocks

**[P1] A later hold never re-fetches the held rows — `apps/design-tool/app.ts:1348`**

`loadSelection` asks the page to report the selection again only when `view.hold.held && !selectionNow.e.held`, and `asked` remembers that hold for the life of the page (`app.ts:402`). The first Pressed on a selection that has no `held` works: the host confirms, the tool sends `inspect {refresh:true}`, and the joined rows show. The selection then carries `held`. Press Hovered on that same copy: the old `:active` rows are hidden (`inspector.ts:89` requires `e.held.state === shown.state`), the panel says "Held hovered.", and the ask is skipped because `e.held` is already set. The page is never asked, so the hover part's rows never appear. Let go and press Pressed again: `asked` still has that hold, so there is still no ask; if the leftover `held.state` is `active`, the previous pressed rows come back without the page reporting them.

The same set survives a new server run. `app.ts:1036` clears `holdNotes` and the selection on a new epoch and leaves `asked`. After a restart, Pressed on the same build, site, and state is treated as already asked, so the rows never load and the panel stays on "Provenance incomplete".

That same leftover `held` is what the ancestor line reads (`holds.ts:53`). Once stage 9 E sends `held.site`, Hovered while the selection still has the pressed answer says the hover is "on the node its press resolves to, #…, above this one."

## Also wrong

**[P2] A later listing puts the previous acknowledgement back when the state name matches — `apps/design-tool/holds.ts:22`**

`holdingOf` drops `held` only on the listing where `hold` changes. The server does not clear `b.held` when it replaces a hold (`scripts/tamber-builds.mjs` `sendHold` clears it only for `hold: null`), and every `build-status` sends that `held` inside `shown(b)`. Hold pressed on site 3 until the host has confirmed. Hold pressed on site 5. The `builds` event correctly says "Holding pressed." A status report before the host answers the new frame has the new hold and `held.state: "active"`. The hold now matches what the tool stored, so the old report is taken and the panel says "Held pressed." on site 5. Round 1's different-state case stays "Holding", because `holds.ts:51` requires the state strings to match. The stub still writes `held: null` on every hold (`stub/tamber-stub.ts:127`), so the test never sends this listing.

**[P2] Why a hold ended still ignores the listing — `apps/design-tool/app.ts:1073`**

Notes are written only from a `held` or `hold-cleared` event that has a reason, and a null hold displays that note (`holds.ts:61`). Two cases from round 1 remain:

- Another client disconnects. The server clears the hold in the `builds` listing and does not emit `hold-cleared`. This client's previous sentence (for example the older "real input" line) is shown as why this hold ended. `sendHold` deletes the note only when this client sends the hold.
- The poll misses the clear. Hello's build has `hold: null` and `held: {state: null, reason: "input"}`. `holdingOf` throws that report away because the hold changed, and nothing copies `reason` into the note. The panel keeps the pre-gap sentence, or says nothing.

A live clear still works: the `held` event follows the listing and sets the note, and a reasonless echo no longer replaces it. A new server run clears the notes.

## Smaller

- While the first refresh is in flight the selection has no rows yet, and `inspector.ts:90` already says "Provenance incomplete… read-only." That sentence is for a host that reported none.
- `sendHold` deletes the note before the server answers (`app.ts:1357`). A refused hold drops the previous reason and leaves the error on `hold-error`.
- The stub's `inspect` has no refresh path (`stub/tamber-stub.ts:137`). The new ask omits `on`, so the stub treats it as off and emits `inspect` off, which marks the selection stale until the next `selection` event. The real server refreshes and leaves inspect on.

## Round 1, settled

The `held` event updates the build without a new listing, and "Held" is only when that report's state is the current hold. "Holding" versus "Held" is right for another node of the build. A place-shaped `source` renders as `file:line`. A stale selection still shows the end sentence, with the buttons and the rows withheld (`app.contract:1200`). Let go from here says "Let go." The test does drive a `held` event, a joined held row, and Let go. Naming the press's node from `held.site` when the answer is for this hold matches the stage 9 E split; until that field exists the line stays empty, which is the recorded limit.

`~/x2voice/SIGNOFFS.md`, `to-dtool2.md`, and `x2p4-s9e-plan.md` are not on this machine. The state-rung default in the 16:18 correction is the other step. I did not run the contract test.
