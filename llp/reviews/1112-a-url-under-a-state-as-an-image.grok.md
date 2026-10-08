# Review: LLP 1106: A URL under a state, as an image (r1, grok)

> **Renumbered 2026-10-08.** These RFCs were drafted as LLP 1105, 1106 and 1107 in a checkout 542 commits behind origin, where those numbers had since been allocated to other documents. They were committed as LLP 1111 (apps as tools), 1112 (a URL under a state) and 1113 (settled and the two clocks); LLP 1101.003 kept its number. The reviews below are transcribed as returned and keep the numbers they were written against: read 1105 as 1111, 1106 as 1112, 1107 as 1113.

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` the same detached worktree at `2ce193976`; 2026-10-07T04:16:30Z to 2026-10-07T04:26:24Z.
- **Method:** one brief per RFC (sha256 `20fb57e3539e62e9bb3f747ca716cd8144e7c5d2ca28f657dd98cce9f80f621a`), shared verbatim with the other reviewer; round 1, blind to the other review. Requested by Charlie ("have astra max and grok 4.7 xhigh review the RFCs"). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted: the brief and documents hold no secrets.
- **Transcription:** the review as returned, unedited. The sentences before the assessment are the CLI's progress narration, kept as returned.
- **Verdict:** NOT READY.
- **Disposition:** r2 (2026-10-06) takes every concern: D4/D8's two pictures become one definition (D1a: data-settled, then motion sought with no timer fired); a production launch record that never enters agent mode (D9, `EXACT_LAUNCH`); a named anonymous mode, with the private-address refusal listed as new work and the release `<app>-linux` as the child (D10); parsing examples and a precedence table (D1); the URL, encoding, handler order and `s-maxage` cache table (D7, D8); the pinned set grown and the refusal list corrected (D2, D3); the threshold metric named and the parity step marked as apparatus needing approval; stale Hermes and PDF-trait claims corrected (D5, D11); both takes said to reverse recorded rulings (§5); the minor measurements and citations fixed.

---

I'll review the draft RFC against the binding rules and the code it cites, without changing anything. First I'm reading the rules, the RFC, and the sibling documents.The rules and deferred list are in. Next I’ll read the RFC and the two sibling drafts so the review can check consistency against both.The RFC makes many code-level claims. I’ll check those citations and the sibling documents against the tree at this commit.The direction is right and belongs in this repository: a public card is a settled picture of a location, the Linux CPU painter is the machine that can make one, and Chrome stays the oracle. The draft is not safe to implement. D4 and D8 disagree about what "settled" means, the launch facts a picture needs exist only in agent mode and a production bake strips them, and spawning today's one-frame binary does not produce the anonymous environment D8 claims.

## Strengths

- **D1** defines a state as the inputs that lead to a picture, not a runner snapshot. That matches LLP 1069.009's rule that an input is a runner call, and it is what makes a picture reproducible across plans. LLP 1101.003 D6 uses the same split: `exact render` flags are not an installed command's flags, and place and time there come from `LANG` and `TZ`.
- **D1** is right to keep the state off the app's query string. The router owns that string (`launch_location` goes through `exact_route::location_of`, `host/linux/src/app.rs:46-49`).
- **D2** picks the painter LLP 1015 already calls the deterministic pixel oracle (`host/linux/src/raster.rs:1-3`) and leaves Chrome as the standard. The missing comparison is real: `host/web-js/conform.mjs:30-31` compares `// linux: layout` boxes to 0.5 px and does not compare pixels, which is what LLP 1015 §7 says.
- **D8**'s refusal of request-chosen size, `prefer`, store, steps, and answers is the right bound for a URL an `<img>` fetches. "Not personal" is the right consent rule, and it matches LLP 1048.002's signed-in pages staying off this path. Refusing golden-image tests in §6 matches the Agent API refusal of behavior diff.
- **D5** does not smuggle paged media back in. `rules/DEFERRED.md` still keeps `@page` out under the LLP 1093 admission, and the draft says a print consumer has to bring that admission itself.
- The order in §4 is right: `answer fetch` is independently useful, and it fits LLP 1103's table (longest prefix wins, `llp/1103-making-a-source-fail-in-a-test.rfc.md` D1). It stays a launch line, so it does not become an eleventh operation.
- Several "today" claims check out. `EXACT_SHOT` boots, paints one frame, and prints `wrote {path} (WxH)` (`host/linux/src/app.rs:9`, `567-573`). `MAX_FRAME_SIDE` is 16,384 (`host/linux/src/raster.rs:383`). `DEADLINE` is 2 seconds (`host/render/src/lib.rs:59`). The server has a worker pool, a bounded queue, and an immediate 503 (`host/render/src/serve.rs:9-10`, `51`, `326`). `/.exact/health` is reserved (`serve.rs:750`). `Document::page_head` writes `og:image` only from `head.image`, made absolute against `app.origin` (`host/web/src/page.rs:87-103`, `138-150`). `exact new` writes `web/` and `apple/` and no `linux/` (`docs/contract-for-humans.md:61-62`, `game/new.mjs:188-255`). The launch-line list at `contract/syntax/src/parser/steps.rs:716-724` is the one the draft cites. Declared faces are loaded (`host/linux/src/text/catalog.rs:191-196`). `scripts/fixtures/fonts/` holds the DejaVu Sans subset.

## Concerns

**BLOCKING — D4 and D8 describe two different pictures.** D4 says the default picture is after `clock settle`, with every transition settled. D8 says no timer fires and the picture is "the settled first frame, as a rendered page is." Those are different clocks in the code. `clock settle` waits for replies, then advances the runner clock and fires each timer at its due time (`host/linux/src/agent.rs:700-709`, `803-809`). The render host does the opposite: "No action runs, no timer fires and the clock stays where boot put it" (`host/render/src/lib.rs:13-14`). Today's `EXACT_SHOT` path does neither. It waits up to 500 ms for images, paints one frame, and exits (`host/linux/src/app.rs:547-578`). It never pumps fetches and never enters the agent loop. §3's change to `app.rs` adds facts, a store snapshot, answers, and a flat-paint report. It does not add a settle loop. An implementer following D4 on the public path would run app timers for a stranger; one following D8 would paint transitions at t=0 and call that "settled." Resolve it by naming two operations that already exist: data lands at the current clock with no timer fired (the render host, and the agent's `clock data`), and motion is a seek of the engine to its settle time with no `advance_timed`. Say which of those the public picture uses, and which bound wins when `clock settle`'s request wait is 20 seconds (`agent.rs:791`) and the server's deadline is 2 seconds.

**BLOCKING — The facts D1 requires are agent-mode inputs, and a production bake drops them.** Locale, zone, epoch, and seed are read only when `EXACT_AGENT=1` (`host/linux/src/zone.rs:32-33`, `85-103`). Otherwise the process uses the machine locale, `TZ`, and a random seed. `prefer` is an agent operation (`host/linux/src/agent.rs:459`), not an environment variable. A production compat file removes `EXACT_AGENT` and every `EXACT_AGENT_*` variable before boot (`host/linux/src/app.rs:146-151`), which is LLP 1069.007 D2. If both `EXACT_AGENT` and `EXACT_SHOT` are set, the agent server returns first and the shot is never written (`app.rs:561-563`). D8's named state, including `prefers-color-scheme`, therefore has no channel on the binary a server would ship. "Grow `EXACT_SHOT`" does not say how a production child receives facts without turning agent mode on. Specify a non-agent launch record (facts, answers, snapshot) that a production binary accepts, and state that `EXACT_AGENT` stays off.

**BLOCKING — D8's environment is not what the child process is.** Anonymous narrows grants to `net.fetch` lines and starts from an empty store (`host/render/src/source.rs:1-29`). The Linux one-frame binary boots the app's own data source. Store writes stay in that runner unless `--storage` names a scratch tree (`host/linux/src/host.rs:1251-1258`). The draft says the child has "the render host's" environment, including "no private addresses unless granted." That private-address rule is a sentence in LLP 1048.000 D10 (`llp/1048.000-pages-at-build.spec.md:654`). The executor enforces grant origins and redirect hops (`host/apple/src/executor_core.rs:975-980`, and the as-built note at `1048.000:714-716`). I found no private-address check in the executor or in ibex2. §3 does not list an anonymous wrapper, an empty cookie jar, or that check. Spawning `caltrain-linux` as specified would paint with the app's real grants and whatever the binary keeps locally. Specify the child as a named mode: empty store, `net.fetch` grants only, no cookies, the same redirect rule as the render host, and a private-address refusal that has to be built because it is not there now. A child panic has to be a 500 that does not take the server down, which the render server already does for its own renders (`1048.000:660-661`).

**MAJOR — The worked examples do not parse, and the three spellings are not subsets.** `size` is `1200x800`, not `1200 630` (`contract/syntax/src/parser/steps.rs:134-151`, `468-474`). Contract interpolation is a template in backticks, not `"…${id}"` (`contract/syntax/src/parser/expr.rs:364-368`, `docs/contract-for-agents.md:317`). The D7 line would be a literal URL ending in `${id}`. D1 says each spelling is a subset of the one before. Scale is in the `app.json` state and in `EXACT_SCALE`, and it is not a launch line. Steps exist only in the test file. The store is only `--storage`, which is a named scratch store kept between drives (`scripts/agent.mjs` usage, `host/linux/src/host.rs:1251-1253`), not an LLP 1018 snapshot handed in at boot. LLP 1105 D8 depends on "a store snapshot of three notes" inside a test. This draft gives that no syntax. `--online`-style launch flags do not exist. `scripts/agent-launch.mjs:126-152` parses `--size`, `--url`, `--locale`, `--time-zone`, `--epoch`, `--seed`, `--storage`, and `--fail-fetch`. `online` is a `prefer` operation.

**MAJOR — D7 and D8 underspecify the URL the server already has rules for.** `page_head` absolutizes `head.image` before it is emitted (`host/web/src/page.rs:87-91`). A recognizer looking for the literal prefix `/.exact/` will miss `https://origin/.exact/picture/...`. A location that itself has a query (`/talks/42?tab=replies`, the D1 example) cannot be placed raw in `?location=` without encoding. The file sketch `dist/.exact/picture/card/talks/42.png` does not say to run the location through `location_of`, which already pops `..` (`route/src/location.rs:27-37`, `79-80`), and to refuse a result that is not a relative path under that directory. `/.exact/picture/card.png` is asset-shaped (`host/render/src/files.rs:8-14`). An unknown asset-shaped path is a 404 before any document render (`host/render/src/serve.rs:845-850`), and a file that does exist under `/.exact/` is served as a static file with `Cache-Control: no-cache` (`files.rs:33-48`, `serve.rs:798-804`). The picture handler has to run beside `/.exact/health`, before both of those. D8's "page's public lifetime" is not the header pages use. A cached document is `public, max-age=0, s-maxage={lifetime}` (`serve.rs:1313-1314`). A deadline page is `s-maxage=60` and `no-store` when busy (`serve.rs:1300-1310`). "max-age=60" would cache a placeholder in browsers, which pages deliberately do not. The origin cache key has to be the canonical location, and any shared cache has to vary on the query string. Missing status rules: unknown state, location the router sends to not-found, painter failure, and a picture larger than the server's 16 MiB page bound (`serve.rs:31-32`).

**MAJOR — D3's pinned set is smaller than the sentence, and "left flat" is partly stale.** `scripts/fixtures/fonts/` contains subset DejaVu Sans Book and Bold only. D3 says that directory covers `sans-serif` and the other generic families. It does not. `EXACT_FONTS` adds a directory to the system scan (`host/linux/src/text/catalog.rs:25-27`, `host/linux/src/app.rs:25`). Skipping the scan without a serif, a monospace, and a missing-glyph rule means those families paint nothing, and the draft does not say whether that is "left flat," tofu, or a hard failure. Portable `symbol:` images are drawn (`host/linux/src/paint/svg.rs:71-91`). A Canvas 2D node is drawn when its bitmap is present (`host/linux/src/paint.rs:995-1003`). LLP 1015 §7 and D7 are older than that code. `sf/` names stay empty boxes (`svg.rs:76`). The refusal list should name the GPU-module canvas and `sf/` symbols, not every `symbol:` image.

**MAJOR — D2's comparison has no threshold and no place to run.** "A perceptual threshold" does not say ΔE, a fraction of pixels, or the band LLP 1015 §7 already uses for the two painters (mean delta and percent of pixels past 32/255, asserted at 5 and 6%). A Chrome comparison is not a sixth blocking check. `rules/RULES.md` keeps the gate at five checks and 60 seconds. The draft adds "a painter-parity script" and says to run it before anyone relies on a picture. Say it is async-lane apparatus, and that adding it needs the human approval `rules/RULES.md` requires for a new script.

**MAJOR — The Hermes justification is stale, and the PDF trait claim is the GPU backend's internals.** LLP 1048.000 D10's as-built text says a JavaScript call is interrupted with Hermes' `asyncTriggerTimeout`; a Rust source still runs to the end (`llp/1048.000-pages-at-build.spec.md:743-748`). A separate process is still justified for Rust sources, for isolation, and because the render binary was split so the painter is not on the server's startup path (`1048.000:634-638`). It is not justified by the current JavaScript budget. D5 says the walk already emits text runs with font bytes and glyph ids. That is what the GPU backend puts into a vello scene (`llp/1015-linux-host-v1.spec.md:141-142`). The trait method is `text(&mut TextEngine, &Paragraph, …)` and `finish` returns a `Pixmap` (`host/linux/src/paint/backend.rs:77-84`, `118-119`). A PDF backend is a trait change. `krilla` inside `exact-linux` would also put a PDF library on every Linux app. Optional capability is a separate artifact, not a dependency of a core host (`CLAUDE.md`).

**MINOR — A few measurements and citations are off, not load-bearing.**

- The 787-face scan is 25 ms on the Mac in LLP 1015 §3, and 3.7 ms is 43 faces on the builder, not the fleet's cost for the same scan. The 1.8 s first run was measured by the author on a debug binary built 2026-09-21, older than these sources. I did not re-run it. Attributing most of that 1.8 s to the scan contradicts the 25 ms figure. The ~4–7 ms frame is §7's CPU time at 420×860 (3.8–6.9 ms), not §6, and not the 2400×1260 picture.
- The summary's 20 ms is process start through first frame painted, fonts and paint included (`llp/1015` Summary). "20 ms boot plus the painting" counts the paint twice.
- `EXACT_DISPLAY=headless` is redundant when `EXACT_SHOT` is set (`app.rs:272-277`). On a Mac the binary is headless anyway.
- "One paint away from what the render already computed" is not the architecture. The render host does not lay out (`host/render/src/lib.rs:42-43`). A picture is a fresh boot, layout, and paint.
- The test grammar's steps are not the ten operations. `pick`, `resize`, `reload`, `close`, and `expect` are steps (`steps.rs:184`, `399`). `pick` is not one of the ten.
- The 503-when-full behavior to cite is LLP 1048.000 D10, as built in `serve.rs`. LLP 1041 is the overload work that admitted the idea.
- `declared_faces` at `catalog.rs:17` is the map. The loader is `catalog.rs:191-196`.
- `lib.rs:13-17` is the comment that names SQLite, secrets, and surface work. The mechanism is the grant filter and the empty store in `source.rs`.

## Suggestions

- Split D4 into data settlement and motion settlement, and say the public path does the first plus an engine seek. Keep `clock settle`, including timers, for the author-run test and the command line.
- Make the production launch record an explicit input in §3, next to the `app.rs` row, and say it is not `EXACT_AGENT`.
- Add the anonymous child mode to the change table. Do not describe it as already true of the Linux executable.
- Fix the three examples to `size 1200x630` and `` `/.exact/picture/card.png?location=${encodeURIComponent(location)}` ``, and define one canonical location function for the test line, the flag, the head, the file path, and the cache key.
- Give `answer fetch` the same inheritance rule as `fail fetch` (file-level lines, several prefixes, longest prefix across both kinds, same prefix replaces). Name the outcome the source parses: status 200, `Content-Type` from the fixture file's extension using `files.rs`'s `content_type`, body bytes, missing file fails by name. Say it is a form of `prefer` if it ever becomes a step, as `fail fetch` is (`scripts/agent.mjs:822`).
- Point the page cache headers at the existing `s-maxage` spelling. Reuse `MAX_PAGE` or name a different byte cap. List 400, 404, and 500.
- In D5, one sentence is enough for LLP 1101.003 D6: a tall screen page stays `exactViewport.media = "screen"` until paged media exists.
- For the §5 takes, say that dropping the JS render option reverses Charlie's 2026-09-28 ruling to keep both, Rust primary (`llp/1071-exact2-web-target.rfc.md:526-532`), and that moving LLP 1069.009's development ring behind this takes back a ring that document already records as ruled in.

## Open questions for the author

- For a public card, do app timers that the boot already scheduled run, or only an engine seek? A fade-in at t=0 and a fade-in at its end are different images, and a timer that fetches is a drive.
- Which binary does the server spawn in production: `<app>-linux` or `<app>-render` plus a CPU painter? The render binary was created so the painter stays off that process (`1048.000:634-638`). `exact render` says `host-dev`. A public server cannot be a development bake.
- LLP 1048.000 still binds the render server to loopback and waits on hosting for a public front door (`1048.000:639-640`, `771-773`). Interview's per-request cards are only fetchable by a crawler if that hosting exists. Is D8 a local generator whose files a later host publishes, or a public route?
- What is the store-snapshot syntax LLP 1105's Fieldnotes test is supposed to write?
- What number is the perceptual threshold, and which existing fixture pages are the corpus?
- If `fail fetch` and `answer fetch` name the same prefix, which wins?
- Q1 and Q2 still gate the admission. The header is right that this specifies nothing until there is an implementer. `rules/RULES.md` and `rules/DEFERRED.md` both refuse a spec nobody is assigned to build, and the JS-render take needs Charlie's words because it reverses a recorded ruling.

Verdict: NOT READY

## Round 2 (2026-10-07), on r2

- **Family:** xAI. `grok-4.7` via the grok CLI (`--prompt-file`), reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` a detached worktree at `2ce193976` holding the three r2 drafts and only Grok's own r1 reviews; 2026-10-07T20:14:49Z to 2026-10-07T20:31:43Z.
- **Method:** one round-2 brief per RFC (sha256 `36d3ad14341b707711e9450de12681013143216d6272832bb5988616c948dd8e`), shared verbatim with the other reviewer; blind to the other family's reviews, though r2's dispositions section names concerns both raised. Requested by Charlie ("ok sounds good" to a second round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited; the sentences before the assessment are the CLI's progress narration.
- **Verdict:** NOT READY.
- **Disposition:** r3 (2026-10-07) took every concern. D1a is now a procedure in ms: activation, a data fixed point (`pump` with announcements, `land_then`, `sync_surfaces`, background, the boolean `accessibilityBusy` walk), a motion fixed point over `settle()`, images, the deadline. The consumer is Caltrain with the sky toggled off (`tap sky-toggle`); the sky-on picture is the negative case. Slice 1 names the three answer sites and the `fetchEarly` guard, the 64 MiB cap, whole-URL prefixes and duplicates as parse errors. D9 has one omitted-field rule per launch mode. `exact render` enforces the hard deadline (exit 124). Per-request status codes, built-file query matching and the private-address ranges moved to §5 as direction.

I'll review LLP 1106 round 2 read-only: the binding rules, my round-1 notes, the revised RFC, and the code and sibling drafts it depends on.The r2 draft rewrites settlement, the launch record, and the anonymous mode. I'll check those decisions against the code and the two sibling RFCs.The audit is done. I am checking the r2 decisions and the code they cite so the review cites the text, not the notes.Round 2 gives each of the three r1 holes a real decision, and most of the new citations match the tree. It is still not safe to build. D1a is the term both siblings import in this same revision, and it is not an algorithm: two of its sentences contradict the calls it names, and the calls it omits are the ones a picture of Caltrain would need. The consumer §5 names fails D2 on the current app. Slice 1 (`answer fetch`) is not guess-free either.

## 1. Overall assessment

The direction is right and belongs here. A public card is a fresh boot of a location under fixed facts, painted by the Linux CPU painter, with Chrome as the oracle. D9's `EXACT_LAUNCH` is a production fact channel that leaves `EXACT_AGENT` off. D10 names an anonymous mode and moves `Anonymous` to `exact-data`, which the dependency graph allows. D11 separates a cooperative deadline from a hard kill and refuses a placeholder on the public path. The worked `size 1200x630` line parses, the store snapshot is gone, and the font and parity claims now match the code and LLP 1015.

What is not ready is the shared definition. D1a tells an implementer to seek with `Host::tick` and `Engine::settle_time`, then says the infinite animation is sampled at the data-settled clock. LLP 1055 D10 says seeking still moves it, and `advance` does. One seek is not the fixed point `clock settle` already runs. `settle_time` is seconds and `group_settles_at` is milliseconds. `clock data` and `EXACT_SHOT` also wait on module activation and call `sync_surfaces`; D1a names neither, so a Canvas 2D bitmap is absent and `native()` paints nothing. Q1's consumer is Caltrain's home card, whose default sky is a full-viewport GPU canvas, which D2's always-on `--strict` refuses.

Slice 1 is the grammar plus "substitute after the grant admits the URL." On the native path that place does not exist: LLP 1103 D1 admits an origin inside ibex, and the fault runs before bindings (`executor_core.rs:850-857`). On the JS target, `fetchEarly` (`host/web/module-glue.js:51-56`) starts a live GET when the grant admits and no fault matches, and an answer is not a fault.

## 2. r1 concerns

- **BLOCKING, two "settled" definitions — partly resolved.** D1a is one named definition, `clock settle` stays an authored step, and the citations of `agent.rs:699-709` and `:800-880`, `lib.rs:13-14`, `has_pending`, `land_then`, `background_operations`, and `tags.rs:532` are real. The definition is still not executable; see the new D1a concern.
- **BLOCKING, facts only in agent mode — resolved.** D9 takes `EXACT_LAUNCH`, keeps `EXACT_AGENT` off, and the zone and bake citations (`zone.rs:32-33`, `:85-103`, `app.rs:146-151`, `:561-563`) hold. Omitted epoch and seed, and the claim that LLP 1069.007 D2 already covers this channel, are new.
- **BLOCKING, the child is not anonymous — resolved.** D10 names the mode, moves `Anonymous`, and lists the private-address refusal as new work. The address classes, the check target, and redirect hops are new.
- **MAJOR, examples and subsets — partly resolved.** `size 1200x630` parses, the spellings are no longer called subsets, `online` is a `prefer` fact, and the snapshot is withdrawn. `steps.rs:747` is `same_launch`, and a duplicate launch line is an error (`steps.rs:109-113`), not a replace-merge.
- **MAJOR, URL, cache, and handler — partly resolved.** Absolutized heads, handler-before-static, the `s-maxage` table, and `MAX_PAGE` match `serve.rs:1299-1314` and `:32`. `location_of` pops `..` before any error can see it (`location.rs:79-83`). A built file does not require an empty query. D8's non-zero exit (500) fights D11's killed child (503).
- **MAJOR, fonts and the refusal inventory — resolved.** The fixtures are Sans Book and Bold, portable `symbol:` is drawn (`svg.rs:71-91`), `sf/` is not, and Canvas 2D is drawn only when the bitmap is present (`paint.rs:995-1003`).
- **MAJOR, parity threshold and apparatus — resolved.** The metric matches LLP 1015 §7, the number is deferred to measuring the corpus, and the step is async-lane apparatus that names Charlie's approval.
- **MAJOR, stale Hermes and PDF-in-core — resolved.** Hermes does interrupt: `exact_js_interrupt` calls `asyncTriggerTimeout` (`js/src/shim.cc:348-349`). Rust runs to the end. PDF is a separate crate and a trait change. The disposition attributes "checked only after return" to r1; that was the other review. D11 does answer the stale JS-budget point.
- **MINOR, measurements and citations — resolved.** Timings are marked historical, `EXACT_SHOT` implies headless (`app.rs:272-277`), and the ten operations are no longer miscounted. A few line numbers drifted; they are in the new minor list.

## 3. New concerns

**BLOCKING — D1a is not a settle algorithm.** Section D1a.

`Engine::settle_time` (`motion/src/engine.rs:672-678`) and `animations_settle_time` (`motion/src/engine/animate.rs:360-369`) return the end of finite motion and omit infinite and paused animations. `Host::tick` then seeks that time (`host.rs:1175-1178`, `engine.advance(now_ms / 1000)`). LLP 1055 D10 (`llp/1055-svg-shapes-and-css-animations.rfc.md:237`) says an infinite animation never settles and that seeking still moves it. D1a's sentence "sampled at the data-settled clock" describes a different picture from the seek it prescribes.

The agent's `settle` converts the engine time with `t * 1000` and then takes `max` with `group_settles_at` (`host.rs:654-661`, `agent.rs:693-697`). `group_settles_at` is already milliseconds (`presenter/group.rs:424-435`, `started + rest * 1000`). D1a says to add the ghost time and names no unit.

`clock settle` is a loop because a seek can start more motion (`agent.rs:818-880`). One call to `tick(settle_time)` leaves a layout transition that the seek itself started at local time 0. `tick` also assigns `host.now_ms`, which later commits read, so the runner clock stays put and the host clock does not.

`clock data` waits on `data_activating` and calls `frame` while it waits (`agent.rs:730-766`). `EXACT_SHOT` calls `sync_surfaces` before it paints (`app.rs:555`). D1a names neither. Without the sync, a Canvas 2D node falls through to `native()`, whose default is a no-op (`paint.rs:995-1003`, `paint/backend.rs:67-74`). `aria-busy` lowers to the boolean prop `accessibilityBusy` (`tags.rs:532`), not the string `"true"`. A loop that sees busy and does not pump `frame` never observes the module finish. `land_then` also runs a due queue `next` with timers off (`commit.rs:339-345`).

Resolve it by specifying picture-mode settle as one procedure: `clock data`'s loop (activation, replies, `land_then`, `sync_surfaces`, `background_operations`, boolean `accessibilityBusy`, pumping `frame` while waiting), then a motion fixed point in milliseconds, as `settle()` already computes, with no `advance_timed`. Infinite animations are sampled at the sought time and reported `motion: "infinite"`. Then `wait_images`, then paint.

**MAJOR — the named consumer fails D2.** Sections D2, Q1, §5.

Q1 and the proposed `DEFERRED.md` entry name Caltrain's home card at build, with `--strict` always on. At boot, `screen` is `"home"`, `sky` is `some("#05081a")`, and `skyOn` is true (`apps/caltrain/app.contract:55-63`, `:117`). The sky is `canvas surface=glass(...)` at `width="100%"` and `height="100%"` (`:188`), a GPU-module canvas, which D2 lists as unsupported. The deck canvas is inside `when deck` and `deck` is false (`:59`, `:317-318`), so that one is off. The line map is Canvas 2D (`:302`) and would draw only after `sync_surfaces`. `task ticker` is `every(1000, tick)` (`:137-138`); D1a does not fire it, so `nowMs` stays the literal `1787915400000`. That is reproducible. It is not a picture D2 will accept.

Resolve it by naming a route or fixture with no visible GPU surface before that consumer is the admission, or by stating an exception for this card.

**MAJOR — slice 1 has no insertion point, and `fetchEarly` still sends the GET.** Section D1, order of work §4.

D1 correctly says the web checks the grant before the fault (`admission.js:38-42`) and the native core runs fault work before bindings (`executor_core.rs:850-857`). LLP 1103 D1 says native admission sees only an origin, and the table sits at the host-side caller that still holds the URL (`llp/1103-making-a-source-fail-in-a-test.rfc.md:37-38`, `:211`). "After the grant admits the URL" does not name that caller. `fetchEarly` returns early only for a grant error or `faultMatches` (`module-glue.js:51-56`). An answer is not a fault, so a matched GET is already in flight before the runner can substitute.

Also unstated: an oversize body (the cap is `MAX_BODY`, `64 << 20`, at `executor_core.rs:26`) — truncate, fail the fetch, or fail the launch; whether the prefix includes the query; and whether a same-prefix line replaces (D1) or errors (`steps.rs:109-113`). `content_type` for the extension is specified.

Resolve it by naming the host-side function on each executor, consulting the answer table inside `fetchEarly` before `fetch()`, and stating the body, query, and duplicate-line rules.

**MAJOR — omitted epoch, seed, and prefer mean three different launches.** Sections D1, D9, against LLP 1101.003 D6 and LLP 1106's own header.

D1's Default column pins epoch `1767225600000` and seed `1`. The named-state column says epoch is "none: the server's clock" and seed is "none." D9 says an unknown field or an out-of-range value is a boot error, and does not say what an omitted field means. LLP 1101.003 D6 fills locale from `LC_ALL` then `LANG`, time zone from `TZ`, and sets no epoch or seed because they are the system's (`1101.003-the-command-line.rfc.md:349-356`). D9 says that sibling "fills the same record from `LANG`, `TZ`." The header says both siblings use settled exactly as D1a defines it. LLP 1101.003 D2 uses D1a "with one difference" (timers fire on the wall clock) and adds "no armed `after`" (`:149-164`). LLP 1105 waits for data-settled only and maps the hard deadline to `unknown`, not 503.

D8's cache key includes the state's resolved facts and the query. A wall-clock epoch in that key never hits. An omitted `prefer` "platform default" (D1) follows the machine, so two machines do not produce D2's "same bytes."

Resolve it with one omitted-field rule per column, `LC_ALL` then `LANG` in both documents, agent `LAUNCH_MEDIA` as the picture default for prefer, and a sentence that LLP 1069.007 D2 still ignores `EXACT_AGENT` while production accepts `EXACT_LAUNCH`. Say in the header that print mode and tool calls use D1a with the differences those RFCs name.

**MAJOR — a killed child is both 500 and 503, and `exact render` can hang.** Sections D8, D11.

D8: a non-zero exit, a missing report, or a crash is 500. The outcome table and D11: deadline or busy is 503, and a killed child is 503 with no picture. A kill is a non-zero exit. The cooperative path says the child reports `settled: "deadline"` and, when anonymous, exits without a picture. It does not say the exit code. Local `exact render` has no parent to apply the hard deadline, and a Rust source runs to the end of the call, so a stuck Rust source hangs the command.

Resolve it as: exit 0 with `settled: "deadline"` is 503; kill or timeout is 503 even though the exit is non-zero; a crash with no such report is 500. `exact render` uses the same hard deadline, enforced by its parent.

**MAJOR — the private-address refusal is only a name.** Section D10.

D10 and LLP 1105 D8 both depend on this check, and neither ranges, nor "resolved address rather than hostname," nor "every redirect hop" is stated. LLP 1048.000's "unless granted" is dropped with no replacement. Grant denial and redirect-to-origin denial exist (`executor_core.rs:973-980`); a private-address check does not.

Resolve it by listing the ranges (loopback, link-local, RFC 1918, CGNAT, IPv6 ULA, IPv4-mapped, and the cloud metadata addresses), checking the addresses the resolver returned, applying the check on every hop, and saying whether a grant can opt in.

**MAJOR — a built PNG is served for a query it was not rendered with.** Section D8.

"A built picture already in `dist` is served from the file" has no empty-query condition. The cache key includes the query. `GET /.exact/picture/card/talks/42.png?tab=replies` can return the build-time PNG of the path without that query.

Resolve it by serving the file only when the query is empty, and rendering any other query.

**MINOR — smaller gaps.**

- `EXACT_LAUNCH` with no `picture` object is not defined as picture mode, and `EXACT_SHOT` is not explicitly left on the old one-frame path. `EXACT_LAUNCH_URL` already exists (`scripts/agent.mjs:878`, `host.rs:828`) and is a different variable.
- A refused JPEG or an unloaded `http(s):` image is not classified against the "visible unsupported node" predicate. Visible should say computed `visibility` (it inherits) and accumulated opacity.
- `app.json` `pictures` has no shape and no state-name alphabet. LLP 1086 has no such key, and the name is a path segment.
- Line drift: the interrupt that fires is `js/src/shim.cc:348-349` (`engine.rs:94` is the comment); `know_roots` is `host.rs:223`, not `:221`; `build.mjs:554-555` looks in both `linux/src/bin` and `web/src/bin`.

## 4. Suggestions

Write D1a as the procedure above, in milliseconds, and point LLP 1101.003 and LLP 1105 at that procedure plus their named differences. Pin picture defaults to the agent's launch media so an omitted prefer cannot follow the machine. State that `picture` in the record is what enters picture mode. For slice 1, name the three executor sites and the `fetchEarly` guard in the same paragraph as the grammar. Copy the 2 s deadline into `exact-linux` rather than taking a dependency on `exact-render`.

## 5. Open questions for the author

- For a named picture state, is the epoch the agent default, the server's wall clock, or omitted-means-the-machine only for non-picture launches? If it is the wall clock, is it inside the cache key?
- Which Caltrain route, or which fixture, is the first public picture that passes D2 with the sky canvas in the tree?
- Is a killed child 503 because D11 says so, and is every other non-zero exit 500? What exit code does the cooperative deadline use?
- Which address ranges does the private-address refusal cover, and can a grant opt out of it?
- Does `answer fetch` replace a same-prefix `fail fetch` in one test, or is that a duplicate-launch error as `same_launch` is today?

## 6. Verdict

Verdict: NOT READY

## Round 3 (2026-10-07), on r3

- **Family:** xAI. `grok-4.7` via the grok CLI (`--prompt-file`), reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` a detached worktree at `2ce193976` holding the three r3 drafts and only Grok's own r1 and r2 reviews; 2026-10-07T20:54:22Z to 2026-10-07T21:25:08Z.
- **Method:** one round-3 brief per RFC (sha256 `e5d544eee109275270836d540637fe449e2a1501293520259f3395e3a13d079d`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("sounds good, try that" to the narrowed r3 and a third round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited; the sentences before the assessment are the CLI's progress narration.
- **Verdict:** NOT READY.
- **Disposition:** pending; the loop stops here after three rounds; not revised against this review.

I'll review r3 of LLP 1106 read-only: governing rules, my earlier reviews, the draft and its siblings, then the code those claims cite.The review file and the three drafts are next, along with CLAUDE.md, so I can judge r3 against the earlier rounds and the siblings.r2's blockers were the settle procedure, the consumer, and slice 1's insertion point. I'll check those claims, and the sibling wording, against the code.The r3 text is in. I'll check each cited behavior against the tree, then the sibling drafts' r3 wording for the same terms.The review is not written yet. I am checking the two claims that would block the first slice against the code, then I will write the verdict.## 1. Overall assessment

r3 is a real narrowing. Slice 1 and slice 2 are the only specified work, §5 is marked direction and lists the open problems from round 2, and D1a is now a procedure with units, activation, and a publishable predicate that is separate from settled. The Caltrain acceptance case matches the contract: sky on is the negative picture, sky off after `tap sky-toggle` is the strict one, the ticker is `every(1000)`, and `nowMs` is the literal `1787915400000`.

An implementer still cannot build that slice from the text. Two specified claims disagree with the code. D1a (c) says the capture moves the session clock and that due timers wait for a later `clock` step. `Presenter::tick` moves only the host clock. The next `tap` fires those timers itself, which is the failure the acceptance test forbids if anyone "fixes" the post-condition by syncing the runner. Slice 1 says there is one runner fault table and three sites. The web and the JS target answer fetches from `host/web/faults.js`, and the wasm request that actually counts is `host/web/http-body.js:184`, which the document never names. Both tables stay empty unless agent mode is on, while picture mode keeps `EXACT_AGENT` off and the D9 example still carries `answers`.

§5 does not affect the verdict. It is labeled unspecified, and each item states the open problem.

## 2. Round-2 concerns

- **D1a is not a settle algorithm — partly.** (a)–(e) now exist, in milliseconds, with activation, `sync_surfaces`, boolean `accessibilityBusy`, `land_then`'s queued `next`, a pump every round, and infinite animations sampled at the sought engine time. The clock post-condition is false, and the loop still has no defined stop. See the new blocking and major items.
- **The named consumer fails D2 — resolved.** §4 makes `render caltrain /` the non-strict case (`flat: ["sky"]`, `publishable: false`) and `tap sky-toggle` then `screenshot` the strict case. The lines match: GPU sky at `app.contract:187-188`, plain scroll at `:192`, ticker at `:137-138`, `nowMs` at `:54`, `deck` false at `:59` and `:317-318`, Canvas 2D map at `apps/caltrain/data/src/lib.rs:253` and `:264`.
- **Slice 1 has no insertion point; `fetchEarly` sends the GET; cap, query, duplicates — partly.** §3 names native `fault_dispatch`, `fetchWith`, and `fetchEarly`, the substitution after `bindings` and `timeout_refusal` (`executor_core.rs:854-860`), a 64 MiB launch failure, the whole URL including the query, and a same-prefix duplicate as a grammar error. The web table and the wasm answer site are still wrong.
- **Omitted epoch, seed, and prefer are three launches; sibling locale order — resolved** for the picture, test, and command-line columns. Locale order is `LC_ALL`, then `LC_MESSAGES`, then `LANG` (`zone.rs:111-114`), which matches LLP 1101.003. The named-state column is marked direction. How picture mode installs those facts is a new concern.
- **A killed child is both 500 and 503, and `exact render` can hang — resolved** for slice 2. D11: the parent kills at the cooperative deadline plus 500 ms, exits 124, and writes nothing. The Hermes interrupt is `js/src/shim.cc:348-349`. Server 500 versus 503 moved to §5.
- **Private-address refusal is only a name — moved.** §5 lists the ranges, every redirect hop, the resolved address, grant opt-in, and that no such check exists.
- **A built PNG is served for a query it was not rendered with — moved.** §5 requires the exact location, query included, and leaves the codec open.
- **`EXACT_LAUNCH` with no `picture` object, `EXACT_SHOT`, `EXACT_LAUNCH_URL` — resolved.** D9: `picture` is what enters picture mode, `EXACT_SHOT` stays the one-frame path, and `EXACT_LAUNCH_URL` is a different variable (`host.rs:828`).
- **JPEG and `http(s):` images are unclassified; visibility is vague — partly.** The refusal record is now inside the paint walk (`paint.rs:855-931`; `paints` at `:1362` is computed visibility). JPEG off Android is `Refusal::DecodeFailed` (`jpeg.rs:213`). An `http(s):` URL fails `relative()` because of `:` (`assets.rs:55-63`) and `open` maps that to `DecodeFailed` (`workers.rs:319`). The publishable list still omits most `Refusal` variants.
- **`app.json` `pictures` has no shape — moved.** Slices 1 and 2 define no `app.json` key.
- **Line drift on the interrupt — resolved.** D11 cites `js/src/shim.cc:348-349`. Leftover citation misses are new minors.

## 3. New concerns

**BLOCKING — D1a (c) 's clock post-condition is false, and the repair fires Caltrain's ticker.** Section D1a (c); §4 acceptance.

The procedure forbids `advance_timed` (`host.rs:972-975` is `advance_effects`, which calls it at `:973`) and then says `Host::tick` sets `now_ms` (`host.rs:1176`), later commits read that clock, the next `--test` step sees the sought time, and timers due by then fire at the next `clock` step.

`Host::tick` (`host.rs:1175-1178`) and `Presenter::tick` (`presenter.rs:1399-1409`) write `host.now_ms` and seek the engine. They do not write `Runner::now_ms`. Every agent reply reports `runner.now_ms()` (`runner/src/agent.rs:89-94`). A following `state` or `tree` therefore still shows the boot clock. A following `tap` does not wait for `clock`: `Host::dispatch_at` (`host.rs:851-873`) passes `host.now()` into `Runner::dispatch_at`, which calls `advance_timed` before the event (`commit.rs:183-189`) and fires every timer with `next_ms <=` that time (`commit.rs:319-328`).

Caltrain's acceptance depends on `every(1000, tick)` (`app.contract:137-138`) not running, so `nowMs` stays `1787915400000`. That holds only while the seek does not advance the runner and the screenshot is the last step. Making the written post-condition true by advancing the runner during (c) fires that ticker whenever the sought time is at least 1000 ms. `advance_within(T, timers: false)` (`commit.rs:285-287`, `:339-345`) still runs a due `then` and a queued `next`. After the engine has been seeked, `commit_effects` does `engine.advance(host.now_ms / 1000)` and debug-asserts that the engine does not go backwards (`host.rs:1269-1286`, `engine.rs:549-555`).

The procedure has to name which clock moves, state that reply `clock` stays on the runner until a specified call, and state whether a later `tap` is allowed to fire timers that became due during the seek. Those are three different sessions.

**BLOCKING — Slice 1's "one table" is the native table, and the required web test does not read it.** Section §3; §6; D9.

§3 says an answer is a second kind of entry in `runner/src/runner/faults.rs`, and §6 lists `admission.js` and `module-glue.js` as the web edits. The JS target and the wasm host consult `host/web/faults.js` (`admission.js:2` imports `takeFault`; `module-glue.js:7` imports `faultMatches`). `fetchWith` fails a match by throwing (`admission.js:42`). `fetchEarly` only declines (`module-glue.js:54`, `faults.js:58-59`). The request that counts, and the only wasm place that can return a body, is `takeFault` in `host/web/http-body.js:184`, which today returns `failed(1, faultMessage(url))`. Neither file is named.

Both loaders are agent-gated. `Faults::from_env` returns an empty table unless `EXACT_AGENT=1` (`faults.rs:44-49`). `launchTable` returns `[]` unless `AGENT_ADMITTED` and the page URL has `agent` (`faults.js:32-36`). D9 says picture mode never turns agent mode on, and its development example includes `answers`. That example has no install path on Linux or on the web.

The native half is specified well: a new dispatch substituted after `bindings` and `timeout_refusal` (`executor_core.rs:850-860`), because `Dispatch::Run` at `:852` would still run before the grant. Longest prefix, whole URL, no `times`, 64 MiB, and same-prefix duplicates match `same_launch` (`steps.rs:745-751`) and `MAX_BODY`. The cross-target test in §3 cannot be built from the files §3 names.

**MAJOR — The data and motion loops have two exit conditions and several unnamed results.** Section D1a (b), (c), (d).

(b) says repeat at most 16 rounds until a round changes nothing, and step 5 is a different check: nothing pending, `background_operations() == 0` (`background.rs:126-130`), and no `accessibilityBusy`. `has_pending` (`commit.rs:1129-1135`) is a third predicate. A round can change nothing while background work is still queued, and `wait_for_replies` (`agent.rs:934-945`) returns immediately when `pending()` is false, with the 20 ms sleep inside that wait. Sixteen tight rounds can end before storage drains. The document does not say what the 17th round is: `settled: "deadline"`, a distinct reason, or a complete settle that fails publishable.

`settle()` is `Option<f64>` (`agent.rs:693-698`). `None` means no in-flight transition. (c) says "let `t` be `settle()`" and "while `t` exceeds `host.now()`", which does not cover `None`.

(d) returns to (b) when `set_intrinsics` changes layout. It does not say whether (c) runs again, or whether the 16-round caps reset. `set_intrinsics` returns `Option<String>` and lays out only when an intrinsic changed (`host.rs:1005-1024`). It does not queue collections. `apply_reports` does (`presenter/images.rs:18-33`). `Presenter::tick` queues collections (`presenter.rs:1400-1402`) and does not call `settle_collections`, which the agent's clock settle does (`agent.rs:828`). `pump`'s `now_ms` is unnamed. `wait_for_replies` passes `host.now()`, and after a seek that stamps reply commits at the sought host time (`host.rs:799-806`) while the runner clock stays put.

(b)3 cites `Host::land_then` (`host.rs:966`). The `land_data` model calls `Presenter::land_then` (`presenter/clock.rs:21-30`), which also `Host::tick`s at the current time. Timers stay off either way. The engine sample does not.

**MAJOR — Publishable's image failures are a subset of `Refusal`, and `flat: ["sky"]` has no identifier rule.** Sections D1a publishable; D2; §4.

`Refusal` is `Overflow`, `InvalidDimensions`, `EncodedLimit`, `HeaderLimit`, `SourcePixels`, `TooLarge`, `Budget`, `QueueFull`, `SubscriberLimit`, `ConflictingMetadata`, `Paused`, `Shutdown`, `Stale`, `ActualExceedsReservation`, and `DecodeFailed` (`host/raster/src/types.rs:147-163`). The predicate names `DecodeFailed`, `QueueFull`, `Budget`, and `Prepared::Failed`. `open` returns `EncodedLimit` for an oversized body (`workers.rs:323-324`) and `Stale` when the asset owner is gone (`workers.rs:316`). Those pictures stay publishable. `image.rs:311` is the `Prepared::Failed` arm, not `Budget`. `Budget` and `QueueFull` appear in `pending()` at `image.rs:433-436`.

§4 requires `flat: ["sky"]`. D2 says the walk records the node. The sky node's string in the tree is `testId="sky"` (`app.contract:188`). The identifier rule is only that example. `row_refuse()` runs for every canvas before a bitmap is drawn (`paint.rs:997-1003`), so the record has to stay on the `native()` and `spatial` paths, as the prose says. `spatial` returns at `:867`, before `paints` at `:910`, so the new record has to add that test on the early return. That part is specified.

**MAJOR — Picture mode's pinned facts have no boot call.** Section D9.

The picture column pins `en-US`, `UTC`, epoch `1767225600000` (`zone.rs:35`), seed `1`, and `LAUNCH_MEDIA`. The only readers are the `EXACT_AGENT=1` branch of `launch_place` (`zone.rs:85-92`) and `agent_time` (`zone.rs:32-33`). With the variable unset, the process takes `LC_*`, `TZ` or the system zone, and `getrandom` (`zone.rs:93-103`). The document does not name the call that installs the record, or say that it happens before the runner is built and before the first frame. `Preferences::NONE` already matches `LAUNCH_MEDIA` (`runner/src/viewport.rs` defaults; Linux reads no system color setting, `presenter/preferences.rs`). Locale, zone, epoch, and seed do not.

**MINOR — D1a's sibling paragraph omits differences LLP 1101.003 actually specifies, and LLP 1105 still cites a server 503.** Sections D1a "The siblings' differences"; LLP 1101.003 D2; LLP 1105 §4.

1106 says 1101 runs (b) on the wall clock, fires timers as they come due, and leaves (c) empty. 1101 D2 also skips the image wait, holds only an armed one-shot (`after`, `then`, queue `next`; `every` and `frame` do not), tests `aria-busy="true"`, and uses a watchdog at timeout plus 2 seconds with a 120 s default (`1101.003` D2, about lines 128-167). 1106's busy check is the boolean prop (`tags.rs:532`, `schema.json:320`), which is the lowered form of that attribute, so the checks can be the same. The omitted differences are real. 1105's own-invocation sentence matches 1106. Its direction section still says a hard deadline is "LLP 1106 D11's 503 with no MCP body" (`1105` line 533). Specified D11 for this slice is exit 124.

**MINOR — Citations that point at the neighboring lines.**

- `pump` at `presenter.rs:1316-1324` is the return taken when there is nothing to apply. Announced topics fall through to `fulfill_all` at `:1323`. The prose is the right behavior.
- `assets.rs:55` is `relative()`, which rejects any name containing `:`. The `http(s):` rejection is `image_input` returning `None` at `:101-102`.
- `jpeg.rs:206` is the comment. The `Err(Refusal::DecodeFailed)` is `:213`.
- `host/render/src/lib.rs:80-92` is `Rendered::status`, which forces 404, 410, or 503 before it reads `head.status`. A picture that copies it would report 503 for a deadline settle, which D1a already reports as `settled: "deadline"`.
- §5's "512 MiB at `image.rs:100`" is the comment about the 32–192 MiB clamp. The arithmetic is `fit` at `image.rs:110`: `pixels * 4 * viewports`, which is 512 MiB at 4096² with 8 viewports. Direction only; the number matches the formula.
- `content_type` is `pub(crate)` in `exact-render` (`files.rs:82`). The runner and the JS driver cannot call it. The match has to be copied.
- The font-set digest, the slice-1 channel for `live`, and "scrolled to the top" (`D4`) are unnamed. The acceptance test has no scroll step.

## 4. Suggestions

State the two clocks in (c) as a table: who writes `host.now_ms`, who writes `Runner::now_ms`, what a reply's `clock` field is, and which later operations may call `advance_timed`. Keep the acceptance sentence "the ticker does not fire" as a consequence of that table.

Name `host/web/faults.js` and `http-body.js:184` next to the runner table. Say picture mode and the agent driver both install answer entries, and that `from_env` and `launchTable` stay empty unless that install happens. Say whether streams, native requests, auth, and asset URLs are excluded the way `fault_dispatch` (`faults.rs:207-208`) and the `!asset` guards already exclude faults.

Give (b) and (c) one stop condition. Say what 16 unsuccessful rounds produce, what `settle() == None` means, that (d) runs (c) again, and that collection settlement is `settle_collections`. Name `pump(host.now())`.

Make publishable "any `Refusal`, or `Prepared::Failed`". Define `flat` entries as the node's `testId`.

In the sibling paragraph, add 1101's image wait, one-shot rule, and watchdog, and point 1105's direction line at §5 instead of "D11's 503".

## 5. Open questions for the author

- After a screenshot inside `exact render --test`, does the next `tap` fire timers that came due during the seek, or only an explicit `clock` step? What does the next `state` reply's `clock` field contain?
- When `settle()` is `None`, is (c) a no-op at the current host time?
- If round 16 ends with `background_operations > 0` or `accessibilityBusy` still set, is that `settled: "deadline"`, a third reason, or a complete settle that fails publishable?
- Does `answer fetch` apply to streams, native requests, auth, and `/assets/` URLs?
- For `render caltrain /` with the sky mounted, does `sync_surfaces` (`surfaces.rs:1057-1083`) fail the settle when no GPU library is loaded, or does it still write the PNG and report `flat: ["sky"]`?

## 6. Verdict

Verdict: NOT READY
