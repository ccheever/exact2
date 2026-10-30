# Code review: the crossing skip's Excord half (render.ts frozen rows), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh (run by x2-perf4, the Excord half's reviewer).
- **Method:** one brief (sha256 `a13b4683aa950a6bb8a3ef17aa10a7100b9dc515a1923391eac55e6dce7d7598`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review only commit `aed74f34cd` — the Excord half of the runtime crossing skip — against the approved design, without editing anything. First I'll read the review criteria and the commit itself.The request is a defect-first review of one commit with a fixed checklist, so I'll follow that review procedure and read the commit against the design.HEAD is `aed74f34cd` and touches only Excord's renderer and its test. I'll read that diff, the exact-js contract it has to meet, and every place a rendered row is written.`aed74f34cd` does the Excord half of the crossing skip correctly. A rendered row is deep-frozen once, the objects that repeat across pages qualify for the runtime's keep, and nothing writes one after that.

**1. What is frozen.** `frozen` (`render.ts:380`) freezes the row and every object and array under it. `renderMessage` builds that graph from fresh literals: blocks, runs, embeds (description, fields, field values, image, thumbnail), media and files (what attachments become), stickers, reactions, poll and answers, reply, and note. `author` is the display-name string, so there is no author object on the row. The store user stays unfrozen, and the cache still keys on that object's identity (`render.ts:395`, `render.ts:402`). A row carrying all of those parts passes the runtime's keep rule in `js/src/binary.js` (`frozenData`: plain prototype, data properties only, no symbols, no functions, strings under 64 KiB, depth far under 64). `JSON.stringify` of that row is identical before and after the freeze.

The `{...row, isNew}` copy (`render.ts:432`) is a new object, so it stays unfrozen and the runtime will not keep it. Its nested arrays are the cached row's frozen objects, and its JSON is that row with `isNew: true`.

**2. Writes after render.** Blocks and runs are mutated only while `renderMessage` is still building the row (`render.ts:78`, `render.ts:83`, `render.ts:138`, `render.ts:140`, `render.ts:318`, `render.ts:325`), before `frozen`. After the row is returned, no Excord code assigns to it or its children. Edits, reactions, and poll votes replace the store message (`discord/store.ts:344`, `discord/store.ts:630`, `discord/store.ts:656`), so the renderer cache misses and a new row is frozen. Spoiler reveal, jump highlight, translation, poll picks, and read state live in separate view state (`app.contract`, `translate.contract`). Image boxes use the width and height baked at render time. The cache replaces its map entry (`render.ts:406`) and does not write row fields.

**3. Cost.** Freeze runs once per rendered row. A cache hit returns at `render.ts:402` before `frozen`. Measured here: 50 plain rows take 0.07 ms (1.4 µs each, 13 objects). 50 heavy rows with 328 highlight runs take 5.3 ms (105 µs each, 378 objects), paid on first render beside the parse, not on later pages. Freezing an already frozen row again is a single `isFrozen` check (0.009 ms for those 50).

**4. The test.** `render.test.ts:69` walks every object in a 20-row window, including runs, an embed with fields, media, and reactions, and expects a write to the row and a `push` on `reactions` to throw. The row marked New is a different object and is not frozen. The other tests in that file assert rendered fields through `createRenderer`. That file passes, 35 of 35. The rest of the Excord suite that loads here also passes (585). Four files do not load in this checkout because `fflate` is missing. That import is outside this diff.

**5. RFC 0001.** The diff is `render.ts` and `render.test.ts` only. No REST call, gateway frame, or contract change.

VERDICT: APPROVE

No findings.
