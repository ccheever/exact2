# Tennis — exact2, with Jev as the opponent

Brief (Charlie, 2026-09-23, lane `tennis`): a second game the template was not
shaped around — a tennis game with forehands and backhands against an AI opponent
whose decisions come from Jev through the Vercel AI Gateway. Start the way an author
would (`game/README.md`, `bun game/new.mjs`), change the engine only when truly
blocked, and keep this diary: time, line counts, workarounds, what was missing,
what was pleasant. Builder: Claude (Opus 5.5), one agent, lane worktree
`lanes-0923/tennis/exact2` on `lane/tennis` from `origin/main` @ 4eca9a09.

The game is `game/games/tennis/`. How to play it is in its README.

## Time log (UTC, 2026-09-23)

The first clock receipt is 16:03:15; reading the lane rules, `CLAUDE.md`, the rules,
`game/README.md`, LLP 1046.003 and the Beacons diaries began about five minutes
earlier and is not in the totals.

- **16:03:19** `bun game/new.mjs tennis`: wrote only `game/games/tennis/`. The
  scaffold names the Rust type `SmallGame` (the template substitutes `small-game`,
  `small_game` and `Small game`, not `SmallGame`) — renamed by hand in `lib.rs` and
  `app.json`.
- **16:03:28** First build refused: Homebrew's Bun 1.4.0 is older than the 1.4.2
  `package.json` pins. Downloaded 1.4.2 into the lane's `tmp/` (another lane had
  done the same). Not an engine issue; a machine one.
- **16:03:57** First build refused again: *the template's captured `Cargo.lock` is
  stale on main* — the core grew `exact-bake`, `exact-markdown`, `exact-raster` and
  `exact-textflow` since it was captured, so a freshly generated game cannot bake
  `--locked`. The error names the fix (`bun game/app/shells.mjs <game>
  --update-lock`), which worked. Beacons' lock is stale the same way (its
  `prove.mjs` fails at the Cargo graph on this checkout). Owned by the loop lane; left alone.
- **16:04–16:06** Cold Linux build of the untouched starter: 103.8 s (`UNVERIFIED`,
  build only). **16:08–16:09** cold web build: 79.0 s.
- **16:09** Probed Jev with curl (key from the environment, never printed): 200 in
  0.56 s and 0.33 s, ~650 input tokens. Finding: a `score` answer with five labels
  is the expected label *index* (2.34 of 0–4), not 0–1 as the brief's example
  suggested. The world normalizes by `levels − 1`.
- **16:10–16:30** Wrote the game: ball physics, scoring, the Jev brain, both
  players, the HUD, the data source and the Contract (about 1,700 lines before
  `rustfmt`). No engine source read beyond the READMEs, `scene.rs`, `input.rs`,
  `audio.rs`, `math.rs`, `environment.rs` and `sim.rs`'s method list.
- **16:31:53** All logic and data tests green after four fixes of my own: an
  inverted service-box test, `Value::str` taking `&str`, a late-answer test that
  assumed decision 1 (a netted first serve made it 2), and — found by a test — the
  launch solver choosing arcs that clip the tape. The solver now treats a tape touch
  as short and keeps the bound that clears.
- **16:33** Linux host build: the Contract compiler reports one error per bake
  (`text has no attribute flex-grow`); 15 s per error round.
- **16:36** The Linux host boots the title and HUD but the world is
  `unavailable`: `logs` says *GPU module has no baked identity*. Root cause below
  (**blocked**). A scratch starter (`bun game/new.mjs` into the lane's `tmp/`)
  fails the same way on its first key (`view 10 cannot take focus`), so it is
  main, not the data source.
- **16:38–16:41** The same starter passes under `EXACT_GAME_PROOF_PROFILE=release`
  (0 failures; 6 min 19 s, a cold release build), which embeds the product digest
  instead — so only the gpu-dev path is broken.
- **16:41** The one engine-side change, its own commit (`4e4dd81f`,
  `bake/src/receipt.rs`, +6 lines). The starter's gpu-dev Linux proof: 1 failure →
  0 failures, 9.2 s.
- **16:42** `aria-hidden` is not a `text` attribute either (a decorative glyph
  cannot be hidden from assistive technology). Removed.
- **16:43** First tennis proof on Linux: 31 of 34 checks. Mine: the bot checked the
  serve two ticks after pressing (contact is 19 ticks later); the paused overlay was
  declared after the top bar, so it covered Resume (`view 36 is covered`); and
  `logs` answers only what is new since the previous read, which my helper had
  treated as the whole journal.
- **16:45** A key-perfect bot and the fallback policy rallied for 59 shots. The
  far player now takes a 0.2 s split step before it runs, every stroke has a
  launch-angle wobble, and scatter grows with aggression and with the incoming
  ball's pace, height and the stretch. A unit test measures it: a calm drive misses
  6.2%, an all-out flat drive 38%, a hurried drive 24%. Players now brake at
  45 m/s² (22 to accelerate). Offline, 90 s of play: 7 points, longest rally 9.
- **16:49:46** Linux proof: **0 failures, 34 checks, 7.0 s** (`UNVERIFIED`: pins
  are still empty).
- **16:50–16:53** First web proof: 0 failures, 174.6 s including the first web
  build with audio and the data source (45.7 s for the host wasm alone). The
  scripted Jev answered the browser's CORS preflight and the page's POST; the whole
  resource → HTTP → plan path is the same on Linux and in Chrome. First look at the
  pixels: the camera was too low (the near player filled a sixth of the frame),
  the fog washed the backstops pale and the net read as a grey wall. Camera to
  9.5 m up and 24.5 m back at 40°, fog thinned, walls and net darkened, legs added
  under the torso. One more web run: 69.4 s warm.
- **16:55:23** First live check against real Jev (Linux, world clock paced to the
  wall clock, 90 s): 32 questions, 31 answers, 0 parse failures, latency p50 541 ms,
  p90 883 ms, max 1,241 ms — and **18.8% of committed decisions late**: the first
  request pays for TLS (1.2 s), and at the net the far player's volleys leave
  25–416 ms between your stroke and its commit point.
- **16:57** Redesign: the rally question now goes out when the **far player
  strikes**, for its *next* shot (it sees where its ball is going and where you
  will return it from); your toss still asks for the return, the start of the point
  for its serve. The HUD keeps the decision secret until the swing, then shows it
  until the next one; a status beside it reads `Jev thinking…`, `plan ready in
  0.40 s` or `late`.
- **17:01:21** Live check again: **44 questions, 44 answers, all before their
  deadline, 0 failures, 0 fallbacks for lateness**; latency min 258 ms, p50 400 ms,
  p90 783 ms, max 1,033 ms; 29 of Jev's intents played; 20 points in 90.4 s. No
  artifact contains the key (checked by the script and by a `grep -rlF` over the
  artifacts afterwards).
- **17:02** Linux proof: 0 failures, 36 checks, 15.0 s. First baseline started:
  `bun game/prove.mjs game/games/tennis`.

## Design: Jev plans, the engine executes

- The world asks by publishing one numbered question in its HUD record
  (`hud.ask`: `{"id":7,"kind":"rally","state":{…}}`), Contract's resource
  `plan = jev(hud.ask)` carries it to a linked Rust data source, and the source's
  answer comes back as the world's `#[live] plan: String` argument. Nothing in the
  world knows about HTTP; nothing in the data source knows about tennis physics.
- Questions are fired early: the return-of-serve plan at your **toss**, each rally
  plan the moment **the far player strikes** (for its next shot: two flights of
  the ball, ~2.4 s, before it is needed), Jev's serve at the **start of the point**.
  The far player's commit point is the first tick of its swing (19 ticks before
  contact); for its serve, the toss, after a 0.6–1.2 s routine that ends early when
  the plan is in. Asking when *you* strike left ~1.1 s and was 19% late live.
- The HUD reveals a plan when it is played, not when it arrives (seeing
  "drop shot" a second early would give the point away), and keeps it on screen
  until the next one; the status beside it says whether Jev is thinking, ready
  (with its latency) or late.
- Applying an answer copies every probability into the saved `Brain` resource and
  samples shot, target and net approach with the world's RNG; aggression is Jev's
  expected score. An answer with another id is stale (journaled); one that arrives
  after its commit point is counted as a late arrival with its latency and never
  applied. The fallback policy (attack the weaker side, lob a net rusher, slice
  when stretched) is deterministic and named on the HUD: `Jev late → fallback: …`.
- The world keeps the near player's own contact hint on `world:near` (`goal`,
  `contact`, `hand`) computed by the same predictor the far player runs to. The HUD
  never shows it; agents and tests drive real keys with it.
- Your tendencies (per-side shots and errors, direction, net approaches, first-serve
  percentage, the last four points) are counted from rally outcomes, never from
  input timing, and go to Jev with each question.

## Where the HTTP call lives

- **Found:** a game can link a Rust data source (`game.data` in `app.json`) whose
  `answer` returns `Answer::Later(Request::post_json(..).header(..).independent_http(n))`
  and whose `parse` turns the `Outcome` into the resource's value. Grants
  (`net.fetch <origin>`) compare origins only. `pending(x)` exists but the world is
  authoritative for "thinking…", so the HUD reads the world.
- **Chosen:** native hosts with `AI_GATEWAY_API_KEY` in their environment call the
  gateway directly (the key is read at request time and rides only in the header);
  everything else — every web build, and a native host without the key — posts to a
  dev proxy on `127.0.0.1:47913` (`jev-proxy.mjs`, 49 lines) that holds the key and
  answers CORS preflights. The origin is fixed because grants are baked; a
  configurable base URL would have to be one of the granted origins anyway.
- No game had used `game.data` before (only a shell-generation test). It linked
  cleanly into all three host shells.

## What was missing (engine and tooling)

1. **Every game's gpu-dev Linux proof is broken on main.** `034d2ac3` (2026-09-22)
   made the bake read the GPU development identity's trust from the resolved
   cohort (`compat.inputs.trust`) instead of the environment. Games are
   binary-only (`deploy.store` is `"0"` everywhere), and a binary-only cohort omits
   trust (`null`), so `EXACT_GPU_DEVELOPMENT` is never honoured and the Linux host
   refuses the module ("no baked identity"). The Linux host's own verifier already
   expects the opposite ("No-delivery apps omit trust from the cohort; their baked
   GPU card still records the explicit development build trust"). The starter
   reproduces it in a scratch copy (`view 10 cannot take focus` on the first key).
   **Changed** (`4e4dd81f`, `bake/src/receipt.rs`): when the cohort omits trust,
   the card follows the build trust `compat.rs` resolved. The only change outside
   `game/games/tennis/`; `cargo test -p exact-bake`: 19 passed.
2. **The template's and Beacons' captured `Cargo.lock` are stale** against the
   core's current crates, so `bun game/new.mjs` produces a game that cannot bake
   until `--update-lock`, and `bun game/prove.mjs beacons` fails at the Cargo graph.
   Left for the loop lane.
3. **`game.data` had never been built for a real game.** It works, but on wasm
   `exact-data-host` warns (`unused import: Work` — the name is only used by the
   native branch), and nothing documents the HTTP pattern: publish a numbered
   question, key a resource on it, return the answer through a `#[live]` string.
   A game→Contract→world round trip for one value costs a shape field, a resource,
   a canvas argument, an id check and a "seen" guard in the tick.
4. **No input channel from Contract into the world except arguments.** The answer
   is a live `String` re-parsed each tick until its id is seen; a message into the
   world (the reverse of `message=`) would say what is meant. Live arguments also
   bind positionally in `Sim::bind`: a test that sets `plan` must restate `seed`,
   `offline`, `paused` and `restart` in order.
5. **A proof cannot wait for a resource without moving the world.** `clock
   settle` waits for in-flight requests but also plays the world to rest (whole
   points, here). To land a scripted answer on a known tick, the Contract shows
   `pending(plan)` as a glyph and the proof polls `tree` with the clock stopped.
6. **`logs` returns only what is new since the last read.** A helper that treats
   it as the journal silently loses lines; the proof keeps its own copy per session.
   Resources (the `Match`, the `Brain`) are not readable through the agent at all;
   the proof reads components and HUD text instead.
7. **Contract, one error per bake, 15 s each.** `text` has no `flex-grow` (CSS
   gives every flex item one) and no `aria-hidden`; each cost a Linux bake to find.
8. **Rust and Contract spell fields differently.** Contract's convention is
   camelCase, Rust's snake_case, and the HUD record's names are the Rust names;
   the shape uses `you_games` rather than fight `non_snake_case`.
9. **The data source has no clock on wasm** (`std::time::Instant` panics), so
   latency is measured in world time by the tick that applies the answer. Under
   the live check the world clock is paced to the wall clock (ratio 1.000).
10. **Machine friction, not the engine's:** Homebrew Bun 1.4.0 below the 1.4.2 pin;
    the key file is `AI Gateway:` then an indented `AI_GATEWAY_API_KEY=…`, not
    shell-sourceable (`sed -n 's/^[[:space:]]*AI_GATEWAY_API_KEY=//p'`); Jev's
    `score` is a label index (0–4), not 0–1.

## What was pleasant

- **Saves were free.** Ball flight, both players, the match, Jev's copied answers
  and the scouting report are `Data` structs and enums with fields, `BTreeMap`s and
  `Option`s; a mid-rally save restored in a fresh process and continued to
  byte-identical saves on the first run, with nothing written for it.
- **Determinism held without effort**: glam on libm, `exact_game::math` for the
  few transcendentals, the RNG for scatter and sampling. The launch solver and the
  far player's predictor use the tick's own integrator, so a prediction *is* the
  future: the far player reaches the exact planned contact tick every time.
- **The HTTP path worked the first time it compiled**, on Linux and in Chrome,
  including CORS, grants by origin and the runner dropping superseded tickets
  (newest wins) — the world's id check is a second line, not the only one.
- **The host-less Linux proof is fast**: 36 checks, a scripted Jev over real HTTP,
  a fresh-process restore and a whole match in 7–15 s. `state world:near` gave a
  key-driving bot everything it needed.
- **`Sim` tests read like tennis**: bind a scripted answer, play ticks with a
  key-only bot, read the journal. A unit test measures the error model's miss
  rates, which is how the balance was tuned.
- **Contract made the HUD cheap**: 129 lines for the title, scoreboard, Jev pill
  with live status, call banner, prompts, touch stick and two stroke buttons,
  pause, restart, menu and match-over, all with accessible names.

