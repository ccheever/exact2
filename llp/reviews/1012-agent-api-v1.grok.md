# Review: LLP 1012 Agent API v1 (grok)

- **Family:** grok (xAI): `grok -p <prompt> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <capsule> --output-format plain`; the prompt embedded the brief and every capsule file; the CLI has file tools and its working directory was the capsule (a repository export) — it did not run anything. (A first launch with `--always-approve` was stopped after three minutes for that flag's write hazard; nothing was written; no output from it is kept.)
- **Method:** llp-review, one round at Charlie's request, 2026-08-29; mutually blind to the other family. Brief sha256 eb4593d6df58863ce8fe7feeb1b446bcd3d8800da8473e8d63b5277d97c975e7; target r1 sha256 a20460f47130478a3f335833fcf77934e3f78688264fd8eaebea019d4fd9fb62; capsule sha256 02281bb288f90c9e12e07308471751add0647902953f5d0ba406bd3e5969acee. Verdict (round 1): NOT READY — binds to the r1 hash.
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T12:55:06Z, verbatim)

> Provenance note: the CLI's leading status lines ("I'll start by…") are its own narration while it read the offloaded prompt and the capsule; preserved as received.

I'll start by reading the review skill, the full prompt, and the spec under review.I'll read LLP 0005, the spec, and the code it claims to transcribe so the review is grounded.I'll read the spec, rules, and implementation files in parallel so claims can be checked against the code.Next I’ll check host glue, ABIs, and the clock/journal paths against the spec’s claims.I have enough to judge r1 against the code. Next I’ll write the review artifact and the LLP 0005 verdict.**Document:** `llp/1012-agent-api-v1.spec.md` r1
**sha256:** `a20460f47130478a3f335833fcf77934e3f78688264fd8eaebea019d4fd9fb62`
**Family:** grok · **Provider:** xAI Grok 4.6 · **Date:** 2026-08-29 · **Redacted:** no
**Method:** independent read of r1 against the capsule; did not author the draft; did not see the other review; did not run the smoke.

## Overall assessment

The architecture is transcribed well: eight named operations, three answered once in the runner, five in host glue, one driver, time as a seek. §5's decisions (real input, no session envelope, no tiers, `clock` not `wait`, `layout` as what the host renders) match the code and `rules/NOT-DOING.md` §Agent API.

It is not yet the reference r1 claims to be. A Linux author cannot implement `layout` / `tap` / `type` / `clock` / `screenshot` from §1 without reading `Agent.swift` and `glue.js`. §4 titles itself "What the smoke holds" and then states magnitudes and a cross-host constant the smoke does not assert. Those two gaps — implementer contract, honesty of the check — are why this revision is not READY.

## Strengths

- **§1 split is the right one.** `tree` / `state` / `logs` from `runner/src/agent.rs`; the other five out of the wasm. `host/web/src/host.rs:150–158` and `host/apple/src/host.rs:155–167` match: `settle` from the host's engine, the rest forwarded to `handle`.
- **§1 target resolution.** "The driver resolves it through `tree` before the host sees it" is `scripts/agent.mjs:254–266`. Both hosts then see a view id. Linux can copy that.
- **§2 journal shape.** `JOURNAL_RING = 4096`, `journal_start` as dropped-count, `{"next","from","lines"}` with `from = since.max(start)` is exactly `runner.rs:359–366` and `agent.rs:162–183`. The boot / dispatch / refuse / poison / command / advance line shapes are the ones `log_outcome` and `advance` emit (`runner.rs:380–393`, `555–601`, `665–681`). The ring test in `host/web/tests/agent.rs:97–128` is the check.
- **§3 map is usable.** `exact_agent(len) → len`, JSON not a batch, same buffer discipline as `exact_dispatch` — `host/apple/include/exact.h:76–79`, both ABIs. Agent mode: `?agent=1` / `EXACT_AGENT=1`, `agentClock` as `now()`, ticker skipped. `--plan` is `exact.reload` / `EXACT_PLAN`. `EXACT_SMOKE=1` kept for metrics stamps (`main.swift:157–167`).
- **§3 clock on the web, in outline.** Registration on `send`, `finish` / `pause`+`currentTime`, `settle` as max of engine, animations, now — this is `glue.js:125–128` and `141–188`. See concern on what that outline leaves out.
- **§5 decisions are the document.** One delivery path for agent `tap`/`type`; no `provenance`/`delivery` tokens; no capability flags; `layout` from the DOM on the web and from views on macOS; no views layer / MCP / generated skill. The nested-scroll claim (only real events show it) is why `Agent.swift:80–91` goes through `scrollWheel` and the web driver through CDP `mouseWheel`.
- **§7 rendering is almost a fixture contract.** One `render` in `agent.mjs:306–323`, pinned by `scripts/fixtures/transcript.{json,txt}`, checked first in `smoke.mjs:40`. The example `"Say \"hi\"\n"` is the fixture. `(nothing new)` is the empty case.
- **§8 is honest** that a `transition` under `clock` is not in the smoke, and that macOS `screenshot` without `window` misses Metal.

## Concerns

### **HIGH** · §1 · Linux cannot implement the five host ops from the table

The table names the ops, the JSON, and which side answers. It does not state the contract a native host has to match. `Agent.swift` is 155 lines of that contract. Missing:

1. **Coordinate space.** Origin is the viewport's top-left, y down (macOS is a flipped clip view, `Presenter.swift:6–8`, `173`; `box` is `convert` into the clip then subtract `clip.bounds.origin`, `Agent.swift:52–56`). Units are points / CSS pixels, not device pixels. Spec says "clip view's space" and "getBoundingClientRect" without origin, direction, or units.
2. **Who is in `layout`.** Web: `el.isConnected` (`glue.js:157`). macOS: `v.window != nil` (`Agent.swift:61`). Not "on-screen": a node scrolled out of a container is still listed, possibly with a box outside the viewport. Order is unspecified (web: `Map` insertion; macOS: sorted by id).
3. **Rounding.** macOS rounds every number to two decimals (`r2`, `Agent.swift:48`). Web returns raw `getBoundingClientRect` floats. A Linux author has no rule. The claimed `652` cross-host equality only survives because both happened to agree on an integer scroll offset.
4. **Transforms.** Web `getBoundingClientRect` includes CSS transforms. macOS `convert(v.bounds, to: clip)` does not include `layer` affine transforms (`applyTransform` writes `layer?.setAffineTransform`, `Presenter.swift:240–245`). Under a spring, the two `layout`s are different quantities. Unstated.
5. **`clock` is monotonic and composed differently per host.** Both hosts refuse a backwards seek (`glue.js:184`, `Agent.swift:128`) — not in §1. Native `settle` is `max(now, engine.settle_time())` only (`Agent.swift:125–126`). Web `settle` is `max(agentClock, springs.settle, starts+endTime for each Animation)` (`glue.js:175–182`). Linux follows native, not the §1 sentence that leads with "the browser's animations". Order is `exact_advance(to)` then `exact_tick(to)` (macOS) / `send(exact_advance)` then `seek` (web). Runner `advance` on a backwards time is a no-op (`runner.rs:561–563`), not an error — the refusal is host glue.
6. **`tap` wheel.** Spec does say "phase-less pixel-unit wheel to the hit view" and `dy > 0` scrolls down. It does not say: CGEvent signs are inverted (`wheel1: -dy`, `wheel2: -dx`, `Agent.swift:86`); delivery is `hitTest(p).scrollWheel(with:)` not `NSWindow.sendEvent` (press uses `sendEvent`; wheel does not); the target's center is only the hit point. Web relies on `--disable-smooth-scrolling` (`agent.mjs:87`), unstated in §1.
7. **`type`.** Non-input is an error on macOS (`"view \(id) is not an input"`, `Agent.swift:106`). Web `focus` will `focus()` any element (`glue.js:165–170`). Web `type` is not an `exact.agent` op: the driver calls `{op:"focus"}` then CDP `Input.insertText` (`agent.mjs:144–148`). Reply shape disagrees with the table (`typed`, `value`): web library returns `{at, typed, target}` with no `value`; macOS returns `{typed, value}` and the library adds `target`.
8. **`screenshot`.** Table reply is `screenshot, w, h`. macOS `window: true` returns `{screenshot, window: true}` and no `w`/`h` (`Agent.swift:146`). Web returns the emulation size, not the PNG's (`agent.mjs:156`). `screencapture` flags are `-x -o -l <windowNumber>` (`Agent.swift:143`).
9. **Errors** are `{"error":"…"}`. Not in the table. Linux needs the string to fail the same way the driver does (`agent.mjs:234`).
10. **Where the five live.** On the web, `layout`/`clock` are in `glue.js` and `tap`/`type`/`screenshot` are in the driver (CDP). On macOS all five are in `Agent.swift`. Linux follows macOS. §3 says this; §1's "answered by host" can be read as "put them in `exact_agent`". They are not in the wasm.

**Resolve:** a short native-host subsection (or extra columns) covering origin/y-down/units, inclusion, monotonic `clock`, native vs web `settle`, wheel hit-test + sign, `type` requires an input, error object, screenshot two paths and their replies. Point at `Agent.swift` as the worked example, not as the spec.

### **HIGH** · §4 · claims the smoke does not hold

Section title: "What the smoke holds, on both hosts."

| Spec (§4 lines 123–130) | `scripts/smoke.mjs` |
|---|---|
| "a wheel of 300 over the content **moves it by 300**" | `moved > 0` (line 98). Magnitude is printed, not asserted (line 119). |
| "**The limit is 652 on both hosts** — the first number `layout` has compared across hosts." | `limit > 100` (line 143). `limit` is printed (line 147). No `=== 652`. The two hosts are not compared in one run. |
| "`clock +60000` decrements **every** countdown by one" | The **first** countdown only (lines 63–67). |

Those are not paraphrases of the check. They are stronger than the check. The 652 sentence sits in the smoke-holds section and is a measurement, if it is anything.

Also unstated in a section that is supposed to be the hold-list:

- Nested fixture: if the page has not moved after twelve wheels, **wait 300 ms wall-clock and wheel again** (`smoke.mjs:144`). That is a settle-poll. It contradicts Summary / §5 "`clock` replaces `wait`" and is not in §8.
- Logo: up to 40 × 50 ms polls (line 58) — §4 does mention polling, correctly.
- GPU: up to 60 × 50 ms polls (lines 105–106) — §4 says only "the GPU module loads (web)".

**Resolve:** write only what `check(...)` asserts. Move 652 / 2.2 s / wasm bytes to a "measured" paragraph that is not the hold-list. Name the 300 ms retry in §8 (or delete it from the smoke).

### **MEDIUM** · §3 / `glue.js` · `clock` on the web is almost as specified, with unstated edges

True of `host/web/glue.js`:

- Agent mode: `agentClock` starts at 0, is `now()`, 250 ms ticker never starts (`18–21`, `220`).
- Every `send` registers unseen animations at current `agentClock` (`125–128`, `WeakMap`).
- `seek`: `t = to - (starts.get(a) ?? to)`; `finish()` if `t >= timing.endTime`, else `pause()` + `currentTime = t` (`141–148`).
- `settle`: `to = agentClock`, then `max` with `ask({op:"settle"}).settle`, then `max` with `(starts.get(a) ?? agentClock) + timing.endTime` (`175–182`).
- Springs (`Element.animate`) and CSS transitions share `document.getAnimations()`.

Not in the spec, and the code does them:

- Backwards seek errors; missing `to`/`settle` errors as a backwards seek (`184`).
- After `settle` computes `to`, `agentClock = to`, **then** `send(exact_advance(to))` (new animations registered at `to`), **then** `seek(to)` (those new ones sit at local time 0). Correct; unstated.
- Unregistered animation: `seek` uses `starts.get(a) ?? to` (local time 0); `settle` uses `?? agentClock`. Different defaults.
- CSS transitions are registered only if they already appear in `getAnimations()` at the end of `send`. Style is applied in the same turn (`apply` → `getAnimations`). §8 admits there is no contract fixture for `transition` under `clock`. The "handled alike" sentence is therefore a claim the smoke does not hold.

**Resolve:** add monotonicity, the advance-then-seek order, and that CSS-transition registration is untested (already in §8 — say it next to the "handled alike" line).

### **MEDIUM** · §4 · numbers are not in the capsule; wasm KiB is sloppy

Traceability:

- Smoke **prints** `boot`, node count, journal lines, `moved`, fixture `limit`, total seconds (`smoke.mjs:119, 147, 154`). It does not print or pin 2.2–2.3 s, 12.7–13.3 ms, 0.8–1.2 s, 126–169 ms, or 0.46 s.
- Wasm 418 002 → 431 782 is not in any file in the capsule. The method ("HEAD built in a scratch worktree") is stated and is the right method. 431 782 − 418 002 = 13 780 bytes = **13.46 KiB** (1024), not 13.8 KiB. 13.8 is 13 780 / 1000.

**Resolve:** cite `scripts/metrics.mjs` (or a dated run) for the times; write 13.5 KiB or 13.8 kB; or drop the sizes until a check prints them.

### **MEDIUM** · §5 · "one delivery path" has real exceptions; two are holes

Holds for agent `tap`/`type`: they do not call `Runner::dispatch` / `act`. Web goes CDP → DOM listeners → `exact_dispatch`. macOS goes `sendEvent` / field editor → `onPress`/`onChange` → `exact_dispatch`.

Honest exceptions (spec already names them in §3, not in §5):

- **`focus`.** Page-side helper for web `type` (`glue.js:165–170`). Not one of the eight. macOS folds it into `type`. Linux should too.
- **`settle`.** Internal `{op:"settle"}` on `exact_agent`, consumed by `clock`. Not a ninth operation.
- **`exact.reload` / `EXACT_PLAN`.** Session boot, not a drive operation. `--plan` in §3.

Holes:

- **`Runner::act` still exists** (`runner.rs:537–553`) and LLP 1005 §6 still says `act(name, args)` is "tests, agents". §5 reads as if agents never take that path. Tests still do. Either this spec says "tests may `act`; agents must not" or 1005 is stale.
- **`quit`** on macOS (`Agent.swift:33`) is a ninth stdin op, unlisted.
- Image load and GPU load move the world **without** a `clock` call (the smoke polls). §4 is honest about the image; §5's "nothing moves between two calls unless a call moved it" is not.

**Resolve:** a four-line exceptions list under §5. Update 1005's "tests, agents" or explicitly retire `act` for agents here.

### **MEDIUM** · §2 · the ring's drop is implied; what a late reader sees is not

The ring drops the **oldest** line (`pop_front`, `journal_start += 1`, `runner.rs:363–366`). Spec says "a ring of the last 4,096" and "`from` above `since` when the ring has moved past it". It does not say: there is no gap marker, no error, and the omitted prefix is gone. A reader who only looks at `lines` cannot tell they missed anything.

§1's table lists `next`, `lines[]` and **omits `from`**. The JS library drops it too (`agent.mjs:242–246` returns `{lines, host}` and advances `logCursor` to `next`). The only signal that the cursor was passed is the wire field the driver does not expose.

Hosts "may append through `Runner::log`": the method exists; no host in this capsule calls it. Permission, not a fact.

**Resolve:** state FIFO drop; state that a passed cursor returns a silent suffix with `from > since`; put `from` in the §1 table; say whether the library should surface it.

### **MEDIUM** · §7 · fixture-tight except empty `label`, and "never parsed" is a discipline

The grammar matches `render()` (`agent.mjs:306–323`) and the fixture, with these holes:

- "Every bracketed part appears only when its field is present" — `label=` is gated on truthiness (`p.accessibilityLabel ?`, line 312), so `""` is present and omitted. `text`/`value` use `!= null` and keep `""` (the fixture's `value=""`).
- `placeholder` is in the JSON fixture and not in the text. That is the advertised lossiness; say so, or the next person will think it is a bug.
- No `state` sample in the fixture (the one op rendered as indented JSON).
- CLI grammar `tap <target> [wheel dx dy]` omits the literal word `wheel` the CLI requires (`agent.mjs:347`).
- `smoke.mjs:19` and `agent.mjs:294` cite "LLP 1012 §8" for the transcript; in this document it is **§7**.

The two rules: **one rendering** is enforced (single `render`, smoke equality, `--record`). **Never parsed** is not: nothing in the capsule parses the outline (the library returns objects; the CLI splits argv). That is a convention. A later `tree.txt` snapshot test that greps the outline is the way it breaks, and no check would catch it.

**Resolve:** `!= null` for `label`; add a `state` fixture row; fix the §7/§8 citations; say "never parsed" is a review rule, not a check.

### **MEDIUM** · Related / §1 / §5 / §7 · exact1 claims the capsule cannot support

The capsule does not contain exact1. These cannot be checked here and are not needed to implement v1:

- six primitives, 80 wire names (Related, §5)
- `provenance: real|synthetic` and `delivery: host|semantic` (§5)
- `rootId` + `snapshotId` + "seven-token identity envelope" (§5)
- `waitForAppSettle` every 50 ms, 500 ms quiet, 10 s timeout that still returned; LLP 0442 "live flake" (§5)
- five transcript forms `full | compact | accessibility | yaml | dense` (§7)
- `exact_tap` / `exact_scroll` / `exact_gesture` (§1)

Citing 0495 §4.1 as the defect class ("a projection that is a parallel reconstruction") is fine as research. Using exact1's numeric inventory as load-bearing contrast is not, in a spec whose authority is this capsule.

**Resolve:** one sentence of contrast, tagged research, or cut. Keep 0495's sentence; drop the 80 / five / 50–500–10 lists.

### **LOW** · §1 · "a ninth costs one" is originated here, slightly mis-cited

NOT-DOING §Agent API lists the same eight, says `clock` replaces `wait`, and does not mention a wheel. The wheel-as-`tap` decision (Charlie, "A") is correctly this spec's. Good.

"A ninth operation costs one of these eight (the trade shape of `rules/RULES.md`'s five checks)" — RULES.md's trade is about **blocking checks**, not operations. NOT-DOING already binds the count ("8 operations, not 90"). The trade sentence should live in NOT-DOING if it is meant to bind the next PR; this spec should cite it, not invent it by analogy. Do not write it in both as two laws.

### **LOW** · §3 · line-count approximations

`runner/src/agent.rs` is 378 lines, not "~330". `Agent.swift` is 155, not "~60 lines in each host's glue". The web CDP carrier is ~130 lines of `Cdp` + `openWeb`, not "~80". Harmless if cut.

## Suggestions

Cut, if the document is meant to be sparse:

- The exact1 inventory (Related, §1 last sentence, §5 identity/wait paragraphs, §7 "five forms").
- §6 (gpu observer, `wall()` vs `now()`). True, already in the code comments; not API.
- Approximate line counts and the wasm-size narrative (or pin them to a printer).
- Charlie asides that do not bind ("seems worth it."). Keep "A" only if NOT-DOING does not take the wheel sentence.
- Duplicate restatement of the eight names already in the table.

Add, minimally:

- Native host contract (concern 1).
- §4 walked back to `check(...)`.
- Journal FIFO + passed-cursor behavior; `from` in the table.
- Exceptions list: `focus`, `settle`, `act` (tests), `quit`, `reload`.
- `clock` will not run backwards.

Keep the table, the journal, the web `clock` outline, §5's decision list, the transcript grammar, §8.

## Open questions

- Should native `layout` include presentation transforms, matching the web's `getBoundingClientRect`, or exclude them, matching macOS `convert(bounds)`? §5 says both answers are "the structure the host renders from." Under motion they disagree.
- Is the fixture's 300 ms overscroll retry a known host bug (async chaining) or leftover wait? If the former, it belongs in §8; if the latter, delete it.
- Default viewport: web driver `420×900` (`agent.mjs:66`), macOS window `420×860` (`main.swift:85`). Which is the parity size?
- Does `logs.from` belong on the library object, or only on the wire?

## On the ten questions

1. **No.** Missing origin/y-down/units, inclusion, rounding, transforms, monotonic `clock`, native vs web `settle`, wheel sign and `scrollWheel` vs `sendEvent`, `type` on a non-input, reply shapes, `{"error"}`, and that Linux follows `Agent.swift` not the CDP half of the driver. See HIGH §1.
2. **Mostly true.** Registration / `pause`/`currentTime`/`finish` / `settle` composition match `glue.js`. Unstated: backwards refusal, advance-then-seek order, different `??` defaults in `seek` vs `settle`, CSS-transition registration timing untested (§8).
3. **Times and 652 are prints, not pins.** Wasm method is honest; 13.8 KiB is 13 780 bytes counted as 1000-byte kB. Numbers are not in the capsule.
4. **Overclaim:** move-by-300, 652, every countdown. **Undermention:** 300 ms fixture retry, GPU poll. Transcript check and logo poll are mentioned.
5. **Almost.** Empty `label` uses truthiness; no `state` row; CLI omits the word `wheel`; comments say §8. One rendering is enforced; never-parsed is not.
6. **`focus` / `settle` / `reload` are honest.** `Runner::act` and `quit` are holes. Polling image/GPU (and the 300 ms retry) punches the "nothing moves" line.
7. **Oldest dropped; `journal_start` is the drop count.** A passed cursor returns the suffix with `from > since`, no error. Table and library hide `from`.
8. **Eight and `clock`-replaces-`wait` match NOT-DOING.** Wheel-as-`tap` is this spec's, correctly. Put "a ninth costs one" in NOT-DOING if it binds; cite it here. Not both as independent rules. Not RULES.md (that's checks).
9. **Yes — the exact1 lists.** Flagged above. 0495's one-sentence defect class can stay as research.
10. **Cut** the exact1 catalog, §6, line-count guesses, unverified size/time numbers, duplicate eight-op prose.

## Recommended next step

Revise r1: tighten §1 for a native implementer, walk §4 back to `smoke.mjs`'s `check`s, name the journal-gap and the helper ops, cut unverifiable exact1 inventory. Stay `Draft`. Do not accept this hash. The other family's review should land on the revised text if the author still wants two families on one revision.

## Verdict: NOT READY

---

## Disposition (orchestrator, 2026-08-29)

Folded into r2 (sha256 7e0f6d7d3cfd345275efbc4493ddd8634bf976119123c775918a0b6af9d2d1b1, unreviewed): the clock made true on both hosts (animations frozen at every batch; `Runner::advance_timed` with per-commit times, web `at` markers and a `clock` trailer, Apple per-commit engine seeks; the clock lands where the runner says, refusal and all; `settle` as a bounded fixed point; the GPU module on the agent's clock); §1 rewritten as the contract a host implements (space, membership, order, rounding, transforms, monotonic clock, wheel sign and delivery, `type` on inputs only, replies, errors, private messages); §5 walked back to what `check(...)` asserts, with the smoke made exact (300, 652, every countdown) and its wall-clock retry removed; the journal's FIFO drop and passed-cursor behavior stated and `from`/`dropped` surfaced by the driver; §7 tightened (predicates, quoting, the outer fixture grammar, `empty`/`dropped`); exact1 inventory cut to one research sentence; line counts and §6 cut; the wasm delta restated as 13 780 B = 13.5 KiB with commit and method; LLP 1005's "tests, agents" reworded. Not folded: a separate normal/agent wasm artifact (open for Charlie, §8); the "ninth replaces one" line (proposed for NOT-DOING, not written here as law); a transition fixture under `clock` in the smoke (§8, not in v1). The verdict above binds to r1; r2 is unreviewed.
