# LLP 1116: What the bake-off asks of Exact2 — round 1

**Type:** RFC (a decision brief; per Charlie's standing instruction for long programs, the author decides and logs, and each decision lands as its own commit so it can be vetoed)
**Status:** Draft r2, 2026-10-10. Both reviewers found r1 ready with changes, reviewing blind: GPT-6 Astra (xhigh) and Grok 4.7 (xhigh), direction and architecture only (`llp/reviews/rfc-2026-10-10-1116-r1.{astra,grok}.md`). r2 folds in their changes (§R); there are no further rounds.
**Systems:** the agent-facing docs (`docs/start-here.md`, `exact new`'s `AGENTS.md`, a new `docs/recipes/`), the app CLI (`scripts/exact.mjs`, the generated `exact.mjs`, `scripts/agent*.mjs`), Contract lowering (`contract/lower`: headings, header projection checks, a warning channel), the web host's stylesheet (`host/web/index.html`, `host/web/src/element.rs`, `host/web-js`), the iOS host (`host/apple/Sources/ExactKit/IOS/`: navigation covers, header projection, keyboard avoidance, accessibility identifiers), the runner's store (persisted state), the stdlib roster (`formatNumber` styles), the kernel's `progress` control
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-10
**Implementer:** Claude (Opus 5.5) lanes, one per decision
**Related:** LLP 1115 (write the web, ship the platform; D7 the one required read); LLP 1102 (the authoring bench; §3.8 `storage.kv`); LLP 1109 (the app farm); LLP 1104 and LLP 1069.011 (the web's default button); LLP 1069.001 (web control sizes); LLP 1075.003 (native controls: the header and its bar, §9.10); LLP 1084 (grouped lists); LLP 1071 (the JS target); the bake-off's own records, `~/bakeoff/rounds/r1/` (`records/`, `grades/`, `analysis/batch-{a,b,c}.md`, `findings.md`)

## R. Reviews, and what r2 changed

Both reviewers: **READY WITH CHANGES**. No decision was marked WRONG DIRECTION. Where both
families raised a change, r2 adopts it. A finding from one family is adopted where the
evidence supports it.

| Change | Raised by | Where |
|---|---|---|
| D1 stays presentation only: no automatic centred column, and `facts.js` keeps reporting the window | both | §4 |
| Paint the grouped route background (white cards on a white page in three apps), and stop a `column` in a grouped row going horizontal | both | §5 |
| Verification observes the shipped app: on iOS, an `expect` or `tap` on a node the bar does not present fails and names why; the tests recipe ends with a bounded real-clock drive | both | §5, §6 |
| The required core is about 150 lines; recipes replace the long-guide and Shelf reads for the questions builders asked | both | §6 |
| Backend tests get per-run isolation (the REST recipe: a fresh database per test run) | both | §6 |
| Deliberate design is allowed: a numeric readout may set its size, `tabular-nums` by default; `no-tells` does not flag it | both | §6 |
| D5 is bounded to small settings and lands in this round | both (Astra: bounded; Grok: this round) | §8 |
| The fallbacks D6 warned about are removed instead: system Back works without `navigationBack`, and a segment's `aria-label` no longer turns off the native control. Warnings remain for what cannot be presented | Astra | §9 |
| The web button keeps its min-content floor ("End session" squashed to "E… s…") | Grok | §4 |
| The `Math.random` refusal names the randomness recipe | Grok | §9 |
| Quality lanes run in parallel with the docs lanes; D7 moves into the D1 lane | Grok (Astra on order) | §11 |
| Round 2 keeps each spec's round-1 model and stack, so a change is measured pair against pair; the backend change is reported separately | Astra | §10 |

Not adopted: a numeric keypad without an authored `inputmode` (Grok). A field's numeric use
cannot be inferred reliably from Contract. Instead the core and the `number-field` recipe
both say it.

## 0. Summary

Charlie's goal: Exact2 should take fewer tokens and less wall-clock time than Expo, React
(web) or SwiftUI to produce a higher-quality app. To measure that, the bake-off builds the
same 24 app specs twice, with the same model each time: once with Exact2 and once with one
comparison stack. A coin flip chose the model per spec: GPT-6.1 Sol at high effort or Muse
Spark 1.3 Contributor at xhigh. The comparison stack was balanced, 8 specs each. The harness
records wall clock and tokens for each build, and a blind pairwise grader (Claude Opus 5.5,
A/B order randomized) drives both apps and scores them.

Round 1 (Exact2 `20da6c87d`, 24 pairs; final numbers in §1):

- **Time: Exact2 is already competitive.** Over the pairs it took 0.79× the comparison's
  minutes (geometric mean).
- **Tokens: Exact2 costs more.** Total tokens were 1.19× the comparison's (fresh tokens, the
  input not served from cache, 1.15×), and 1.78× against React on the web. The difference is reading, not writing. Exact2
  builders took in 100–350 KB of tool output, against 15–42 KB for React and SwiftUI builders,
  who "already knew" their stack. Of that:
  - `start-here.md` (30 KB), read in full by every builder;
  - lookups in the long guides;
  - **about 280 KB of the CLI's own source** (`scripts/exact.mjs`, `agent*.mjs`,
    `build.mjs`), because `--help` works on no verb.
  Everything read early is carried in context through every later turn.
- **Quality: Exact2 loses most pairs.** The grader preferred the comparison app in 18 of 24:
  React 8 of 8, SwiftUI 7 of 8, Expo 3 of 8 (see §1). Almost every app on both sides passed every feature, so the losses are about how
  the app looks and how it behaves at the edges:
  - **On the web, Exact2 lost every pair on looks.** Its platform score was 5–6 against
    React's 7–9. Unsaid presentation is the browser's own (grey UA buttons, regular-weight
    headings, a 13 px checkbox for a switch, no header bar, unstyled dialogs and tabs). That
    is what LLP 1115 §3 asks for, and graders read it as "an unstyled prototype".
  - **On iOS, Exact2 won on native chrome and dark mode** (book-club, mood-journal,
    password-generator, notes). It lost on host bugs and traps:
    - a white band over the bottom safe area in 5 apps;
    - a header's extra text silently dropped from the navigation bar;
    - keyboard avoidance off by default;
    - `performanceNow()` persisted as a timestamp;
    - a shuffle seeded with 0 because randomness is refused;
    - no grouped or currency number format.
  - **Exact2's Snapback client caused the worst losses**, on 4 backend apps (writes that hang,
    stale reads, a stalled feed). Charlie has since moved the bake-off to a plain Bun REST
    server. The client bugs go to their own track (§9).

This RFC decides what to change before round 2. In order of expected effect:

| # | Decision | Moves |
|---|---|---|
| D1 | The web's unsaid presentation becomes a designed default look; LLP 1115 §3's web bullet is amended | web quality (8/24 pairs) |
| D2 | iOS host fixes: the bottom band, the grouped background, the header's subtitle, keyboard avoidance, the checklist mark, test ids on bar items, and tests that observe the presented app | iOS quality |
| D3 | A smaller required read, a single-stack skeleton, and `docs/recipes/` for the questions builders actually asked | tokens, time, quality |
| D4 | Every CLI verb answers `--help` with its flags, ports and an example | tokens |
| D5 | `state … persist`: a remembered setting in one line | tokens, time, quality (9/24 specs) |
| D6 | Remove two silent iOS fallbacks; a warning channel in `contract build` for the drops that remain | tokens, quality |
| D7 | Headings take the emphasized weight of their text style | quality, both hosts |
| D8 | Small gaps: `formatNumber` styles, a randomness recipe, `performanceNow()` wording and its persistence warning, determinate `progress`, `symbol:sf/` in the refusal, a favicon | quality |

The bake-off's own process changes are listed in §10.

## 1. Round 1 numbers

Per pair, from `~/bakeoff/rounds/r1/records/*.json`. Minutes are wall clock measured by the
harness, from launch to the builder's exit. Tokens are the model's input plus output tokens
summed over every model call, subagents included.

| Spec | Model | Stack | Exact2 min | Other min | Exact2 Mtok | Other Mtok | Exact2 overall | Other overall | Better |
|---|---|---|---|---|---|---|---|---|---|
| Unit Converter | GPT | SwiftUI | 9.4 | 7.9 | 3.6 | 1.6 | 7 | 8 | SwiftUI |
| Household Shopping List (backend) | Muse | Expo | 23.8 | 49.8 | 15.3 | 30.2 | 6 | 6 | Expo |
| Water Tracker | Muse | React | 8.7 | 6.4 | 3.6 | 2.7 | 6 | 8 | React |
| Weather | GPT | Expo | 7.4 | 26.0 | 2.3 | 9.3 | 7 | 8 | Expo |
| Recipe Finder | GPT | React | 6.1 | 6.2 | 1.5 | 1.0 | 6 | 8 | React |
| Book Club (backend) | GPT | Expo | 18.1 | 15.8 | 9.3 | 6.8 | 7 | 7 | Exact2 |
| Password Generator | GPT | Expo | 13.1 | 16.9 | 5.6 | 4.9 | 8 | 7 | Exact2 |
| Flashcards | GPT | React | 5.2 | 6.8 | 1.6 | 0.9 | 6 | 8 | React |
| Chat Rooms (backend) | Muse | SwiftUI | 40.3 | 19.9 | 21.7 | 7.5 | 4 | 7 | SwiftUI |
| Stopwatch | Muse | Expo | 8.3 | 15.6 | 2.6 | 4.0 | 4 | 8 | Expo |
| Polls (backend) | Muse | SwiftUI | 19.2 | 15.0 | 12.5 | 6.7 | 7 | 8 | SwiftUI |
| Habit Tracker | GPT | React | 5.7 | 6.9 | 1.4 | 0.8 | 6 | 8 | React |
| Team Kanban (backend) | Muse | SwiftUI | 22.8 | 20.0 | 13.4 | 6.8 | 2 | 7 | SwiftUI |
| Mood Journal | GPT | SwiftUI | 6.2 | 6.3 | 1.9 | 1.5 | 7 | 7 | Exact2 |
| Tip Splitter | GPT | React | 3.8 | 6.7 | 0.9 | 1.0 | 6 | 8 | React |
| Workout Log | Muse | SwiftUI | 12.9 | 90.2 (cap) | 4.3 | 17.8 | 6 | 7 | SwiftUI |
| Notes | GPT | Expo | 11.0 | 17.7 | 3.3 | 5.6 | 7 | 6 | Exact2 |
| Trivia Quiz | Muse | SwiftUI | 8.9 | 17.6 | 3.3 | 2.4 | 6 | 7 | SwiftUI |
| Grocery List | Muse | SwiftUI | 10.5 | 27.4 | 3.8 | 6.2 | 6 | 8 | SwiftUI |
| Event RSVP (backend) | GPT | React | 11.2 | 9.5 | 5.8 | 2.7 | 3 | 8 | React |
| Expense Tracker | GPT | Expo | 9.8 | 9.5 | 3.7 | 2.7 | 8 | 6 | Exact2 |
| Contacts Directory | Muse | React | 6.5 | 6.1 | 4.2 | 2.9 | 5 | 7 | React |
| Focus Timer | GPT | React | 11.7 | 6.4 | 3.1 | 0.5 | 6 | 8 | React |
| Countdowns | GPT | Expo | 10.6 | 13.2 | 4.2 | 4.5 | 8 | 6 | Exact2 |

Geometric means, Exact2 over the comparison: minutes **0.79×**, total tokens **1.19×**, fresh tokens **1.15×**. Better: Exact2 **6**, comparison **18**. By stack: React 0 of 8; Expo 5 of 8; SwiftUI 1 of 8.

How to read it:

- **The comparison builders know their stacks from training.** Their diaries say "no docs
  needed". They read a 30-line README at most. Exact2 builders must learn Contract from the
  docs.
- **The grader drove every app**: Maestro on the simulator, Playwright on the web. It checked
  every feature, relaunched for persistence, and tried the edge cases the spec names. Scores
  are 1–10, and `better` is its pairwise verdict.
- **Diary minutes are self-reported and inflated.** One backend diary claims 52 minutes inside
  a 22.8-minute run. They are used only as relative weights.

## 2. Where the tokens go

`~/bakeoff/anatomy.py` classifies every command's output by what it read. Here are the bytes
of tool output taken in, summed over the Exact2 arms of the first 10 pairs:

| Read | KB |
|---|---|
| `docs/start-here.md` (read whole, sometimes twice) | 356 |
| `docs/contract-grammar.md` (lookups) | 123 |
| `scripts/exact.mjs` | 112 |
| `docs/contract-for-agents.md` (lookups) | 73 |
| `scripts/agent.mjs`, `agent-launch.mjs`, `scripts/agent`, `agent-test.mjs` | 124 |
| `host/apple/build.mjs`, `host/web/serve.mjs` | 48 |
| `apps/shelf/app.contract` | 37 |
| iOS host Swift (to explain silent fallbacks) | 19 |

The comparison arms read 0–7 KB of documentation per run. Exact2 builders wrote *less* than
React builders in three of four GPT pairs, so the whole gap is input. Three things drive it:

1. **The required read is about 11k tokens:** `start-here.md` at about 8k plus `AGENTS.md` at
   about 3k. About half of `AGENTS.md` describes the authoring diary, which this program and
   any `feedback never` project skip.
2. **The questions builders searched for:**
   - test `expect` forms;
   - date and day keys;
   - timers that survive a relaunch;
   - modulo and padding;
   - a number keypad;
   - clipboard;
   - currency;
   - the agent's `scroll`, `--size` and `--port`;
   - SF Symbols.
   Each is a grep over a guide of 10–20k words, followed by reading 100-line windows.
3. **The CLI documents none of its flags**, so builders read its source. In part the
   bake-off's prompt caused this, because it told builders which ports their servers may use
   (fixed for round 2, §10). But no help text says which ports `test` and `agent` open,
   either.

## 3. What the graders marked down

The grader verdicts and the diary analyses (`analysis/batch-*.md`) agree on these causes,
ranked by quality impact:

1. **The web look** (D1): every web pair. Unsaid and `bordered` buttons are Chrome's UA
   button (`host/web/src/element.rs:1054-1058`, `index.html:68`). Content tablists show no
   selected state. A `switch` is a 13 px checkbox in Chrome. The `header` is a line of text.
   `destructive` is unstyled. The alert dialog keeps the UA border. Headings are regular
   weight. Graders: "most secondary buttons are unstyled browser defaults next to the styled
   blue ones", "flat typography", "looks like an unstyled prototype".
2. **The iOS bottom band** (D2): unit-converter, book-club, weather, password-generator,
   mood-journal. `NavigationBarIOS.swift:789-793` pads every route by the bottom safe area,
   so the route's scroller ends 34 pt above the screen's edge, and the strip shows white over
   a grey grouped list.
3. **Traps that compile and behave wrong:**
   - stopwatch, 4 vs 8: a persisted `performanceNow()` (D8). The docs say "since boot", and
     the builder read it as the device's boot.
   - trivia-quiz: a shuffle seeded with 0 (D8).
   - household-list: an "N items left" text in the `header`, dropped by the iOS bar (D2).
     The driver paints the authored header, so the builder's own screenshot showed it.
   - two iOS fallbacks with no diagnostic: a segmented control whose tab label differs from
     its text, and a route with no `navigationBack` (D6).
4. **Copied recipes and their flaws** (D3):
   - start-here's skeleton is tab-based, so single-screen apps shipped a one-tab tab bar
     (chat-rooms, polls).
   - Snapback's README status lines became UI text in 4 of 5 backend apps.
5. **Missing pieces** (D5, D8):
   - remembered settings: a resource, a mutation, a source and grants per setting;
   - money formatting: `$1481.47`, where React printed `$1,481.47`;
   - determinate progress: refused, in 3 of 5 pairs that wanted it;
   - the decimal keypad: supported by the host (`inputmode`), but undocumented in start-here.

## 4. D1 — the web's unsaid presentation is a designed default look

**Decision.** On the web, what the author leaves unsaid draws as a well-made web app would,
not as the browser's bare controls. LLP 1115 §3's web bullet is amended:

- **Before:** "the web host keeps resolving roles to the browser's own values".
- **After:** "the web host resolves roles to a designed default look that a web developer
  would not notice as unstyled".

LLP 1115's own test settles it. Someone who knows the web notices a grey UA button and a
13 px checkbox switch as unstyled. So those are tells, and the platform's answer wins. On the
web, the platform's answer is the look of a carefully built web app.

The amendment reaches the rulings that pinned the UA look: LLP 1069.011 D2 (the `ua` look),
LLP 1104 D8 (the UA button), LLP 1069.001 D1/D5 (the switch) and LLP 1084 §143.

**What changes.** Everything is in the web host's stylesheet (`host/web/index.html`, which
both web targets ship). Each rule is written in `:where()` or as a custom property, so an
author's row still wins: author > platform > CSS default.

| Element | New default |
|---|---|
| Unsaid and `bordered`/`gray` buttons | a neutral fill, `var(--exact-control-radius, 8px)`, `.5em 1em` padding, weight 500, hover and pressed states |
| `destructive` | the system red |
| Content `tablist` | a segmented track; the `aria-selected` tab is raised |
| Root tab bar (tabs with `aria-controls`) | a top separator; the accent colour on the selected tab |
| `switch` | a drawn track and thumb where the browser has none (`@supports not selector(::thumb)`); Safari keeps its own |
| A route's `header` | an app bar: material background, bottom separator, minimum height, sticky |
| `dialog role="alertdialog"` | rounded, borderless, shadowed, a darker `::backdrop` |
| The button's label | keeps its min-content floor: no `overflow: hidden` with `minmax(0, auto)` squashing a label to "E… s…" |
| An alert's actions | in a row, with `destructive` in red |
| Fields and `select` | a real tap size (at least 36 px tall), the same look as text fields |

A centred readable column for wide screens is a recipe (`web-look`), not a default. It would
change layout and the facts an app reads (r2, both reviewers).

**Constraints checked.**

- **Layout parity holds.** The kernel never lays out on the web (LLP 1071). The JS target and
  the wasm reference ship the same stylesheet, so conformance moves both sides together.
- **The Apple hosts never read `index.html`.**
- **The `switch` box is recorded in the Chrome tables** (`kernel/tests/it/browser_controls.rs`)
  and in the kernel's fallback sizes (`kernel/src/control.rs:72`). The drawn switch keeps
  Chrome's box size, and draws a larger track inside it only if those tables are re-recorded
  in the same commit.
- **Linux's painted button padding** (`host/linux/src/paint/button.rs`) is checked against any
  layout plan that measures a native button's width.

**Cost.** One lane, about a day, plus the LLP amendments and updated press and focus tests.

**Expected effect.** +2 platform points per web pair (the analyses' estimate), with no change
to what the author writes.

## 5. D2 — iOS host fixes

1. **The bottom band.** In `reportCovers` (`NavigationBarIOS.swift:789-793`), the bottom cover
   becomes 0 when the route's last in-flow child is its scroller, as the top cover does when
   the scroller is under the bar. UIKit then extends the scroller or grouped list to the
   screen's edge and insets its content (`adjustedContentInset.bottom`), as a hand-built app
   does.
   - A footer below the scroll keeps its padding (native-fixture's Detail route).
   - A route without `navigationScroll` gets the same rule when its last child is a scroller.
   - Test: `NavigationBasicsIOSTests.testAnInlineTitlesScrollerGoesUnderTheBar`. The list's
     `maxY` equals the view's `maxY`, and the inset equals the safe area.
2. **A header's second text is the bar's subtitle.** `HeaderTitle.init`
   (`NavigationTitleIOS.swift:31-104`) falls back to the header itself as the heading's group.
   So the first text after a heading that is a direct child of `header` becomes
   `navigationItem.subtitle` on iOS 26, and the drawn title view before it. This amends
   LLP 1075.003 §9.10 ("a heading the header holds directly has none").
   - The driver's `tree` and `screenshot` mark any header node the iOS bar does not show,
     under the authored chrome too, as `TouchIOS.swift:93-99` already explains taps.
   - D6's warning names the case at compile time.
3. **Keyboard avoidance on by default.** A focused field inside a scroller is kept above the
   keyboard, and the scroller's bottom inset follows the keyboard, as UIKit's
   `keyboardLayoutGuide` and SwiftUI do. The existing opt-in mechanism becomes the default.
4. **A `testId` on a bar item or tab is its `accessibilityIdentifier`**, so XCUITest and
   Maestro can address it, as they already can every other node.
5. **A checkbox is the platform's checklist mark.** iOS has no square checkbox. An
   `input type="checkbox"` without `switch` draws the system's checklist circle, an empty
   circle that fills with a checkmark as in Reminders. Today it draws a square box, which
   grocery-list's grader read as "a web form on iOS". The web and macOS keep their own
   checkboxes.
6. **The keyboard reappearing over a menu after a create** (polls). Reproduce it; fix it if the
   cause is the host's focus restoration.

7. **The grouped route's background is painted.** In light mode, three apps (weather,
   password-generator, mood-journal) set `-exact-grouped-background` as the docs say, and
   still showed a white page, so their white cards disappeared. The route's authored
   background paints the whole route, under the home indicator too. Author > platform.
8. **A `column` in a grouped row lays out as a column.** Two apps saw a grouped row's
   vertical content laid out as a horizontal strip, while their tests passed. The row gives
   an authored `column` its own direction, or the compiler refuses the form with its repair.
   The implementing lane chooses which, by what the grouped-list sheet allows.
9. **Verification observes the shipped app.** The default drive keeps the authored chrome
   (every test addresses nodes as on the web). But on iOS, an `expect` or `tap` on a node the
   platform bar would not present fails, saying why ("the iOS navigation bar does not show
   this node; in the app it is not visible"). A screenshot marks such nodes. Household-list
   lost its verdict, with every sub-score in Exact2's favour, on exactly this.

**Cost.** One lane, one to two days, with XCTests for each item.

## 6. D3 — a smaller required read, a single-stack skeleton, and recipes

1. **`AGENTS.md` sheds the authoring diary.** `exact new` writes one line: "run
   `bun exact.mjs feedback status` first; it prints what to keep". `feedback status` prints
   the diary instructions when the answer is `ask`, `local` or `always`, and nothing for
   `never`. The commands table stays.
   - Target: `AGENTS.md` from 11.5 KB to 5 KB or less.
2. **`start-here.md` stays the only required read, cut to a core of about 150 lines.**
   - **The skeleton is a single stack** (a list route, a pushed detail, a sheet, an alert).
     Tabs become a ten-line add-on, with the rule "a tab bar has at least two tabs".
   - **The native patterns stay** (they are why Exact2 wins on iOS), compacted.
   - **What builders searched for gets one line each**, pointing to a recipe:
     - `inputmode="decimal"` for numbers;
     - `font-variant-numeric="tabular-nums"` for a readout;
     - `scrollFollowEnd` for a chat;
     - `symbol:sf/<name>`;
     - `formatNumber` styles;
     - persisted state (D5);
     - real-clock time for anything that outlives a launch.
   - Target: about 150 lines (from 702). The long guides and `apps/shelf` leave the first
     page. The lookup table points at recipes first, and at the guides only for what no
     recipe covers. Reading is measured as everything a build reads, lookups included, not
     only the core's size.
   - The core starts with one screen. Navigation, sheets and alerts are recipes added when
     the app needs them, so builders don't copy a stack skeleton in place of a tab skeleton.
3. **`docs/recipes/`**: one short, compiled, tested file per question builders asked.
   - **Recipes:**
     - `persisted-setting`;
     - `number-field`;
     - `money`;
     - `dates-and-days`;
     - `timer-that-survives-relaunch`;
     - `random-and-shuffle`;
     - `chat-thread`;
     - `responsive-columns`;
     - `rest-backend` (fetch, polling, bearer tokens, errors as data, and a fresh database
       per test run so tests never meet another run's rows);
     - `tests` (every `expect` form, and a bounded real-clock drive before calling the app
       done: write, refresh, relaunch);
     - `numeric-readout` (a timer's or total's big number: a literal `font-size` with
       `tabular-nums` is design, not a tell);
     - `web-look` (what D1 gives, and the few attributes that matter).
   - **Format:** each recipe is under 80 lines: a code block that compiles under the
     `<!-- check -->` harness, then the three or four rules that matter.
   - **Index:** start-here's lookup table lists them first.
4. **Deliberate design is allowed.** "Leave colours, fonts and metrics unsaid" means the
   platform answers what the author omits. It does not forbid the few things an app
   designs, such as a numeric readout, a chart, or a progress ring. The core says so, and
   `no-tells` does not flag a size on a node with `tabular-nums`.
5. **Recipes are ship quality.** A recipe that shows a status line, an error code or a single
   tab is a tell that builders copy. The `check` harness compiles every recipe, and the
   `no-tells` script runs over them.

**Expected effect.** About 4k fewer tokens in the required read, carried through every turn,
and fewer grep-and-read cycles. The round-1 analyses estimate 20–30% fewer total tokens per
build, plus fewer copied tells.

## 7. D4 — the CLI answers `--help`

- **Help on every verb.** Each verb of the generated `exact.mjs` (`web`, `web-build`, `test`,
  `agent`, `ios`, `mac`, `linux`, `android`, `contract`, `hatch`, `update`, `feedback`)
  answers `--help` and `-h`, anywhere in its arguments, with:
  - one line of purpose;
  - its flags and what each takes;
  - the ports it opens, and how to choose one (`web --port`);
  - whether it runs in the foreground;
  - one example.
- **The texts live in exact2**, in a `scripts/help.mjs` table keyed by verb. The generated file
  forwards `--help` to it, so `exact.mjs update` never has to carry help text.
- **The index:** `bun exact.mjs help` and `bun exact.mjs` with no verb list the verbs with
  their one-liners.
- **The dispatched scripts refuse an unknown flag**, naming the known ones. Today
  `agent web --help` is "unknown op: --help", and `web --help` starts a server.
- **Test:** a unit test runs every verb with `--help` and checks that it exits 0 within a
  second and starts nothing.

## 8. D5 — `state … persist`

**Problem.** "Remembered across relaunch" settings appear in 9 of the 24 specs: a tip %, units,
°C/°F, timer durations, the selected persona, a high score, a water goal, and the last
category. Today each one needs:

- a resource, a mutation and an `app.ts` source;
- grants;
- JSON parsing and validation;
- a test that reloads.

SwiftUI writes `@AppStorage("tip") var tip = 18`, and React writes
`useState(() => localStorage…)`. LLP 1102 §3.8 proposed a `storage.kv` data-module API and
kept "the first frame" out of it.

**Decision.** A root component's `state` may say `persist`:

```contract
component App
  state tipPercent = 18 persist
  state units = "metric" persist
```

**Semantics.**

- **Before first paint**, the runner reads the app's persisted values from its store snapshot,
  which is already read synchronously before boot (LLP 1018). A stored value that matches the
  state's type replaces the declared initial value. A missing value, or one of the wrong
  shape, keeps the initial value, and a mismatch is logged.
- **After each commit** that changes a persisted state, the runner writes it through the same
  store path that `secret.keep` uses, under a non-secret scope:
  - the web: `localStorage`;
  - Apple: the app's `UserDefaults` domain;
  - Linux: a file in the app's data directory.
- **The key** is the state's name. A rename loses the old value, as an `@AppStorage` key change
  does.
- **What may be persisted:** small settings only, which is the evidence: numbers, strings,
  bools, options of those, and lists of those, at most 16 KB a value. The compiler refuses
  `persist` on records, on lists of records, and on anything holding an action or a host
  handle. Collections of app data stay in a source (r2, Astra).
- **The agent and tests:**
  - a drive with `--storage <name>` keeps persisted state in that store;
  - `reload` reads it back;
  - each authored test starts empty, as storage does today.

The Lean semantics gains an initial-store input for persisted slots, and difftest's generator
covers it.

**Cost.** The largest item: compiler, runner, three hosts' stores, semantics and difftest, docs.
Two to three lane-days. It lands in this round, after D1–D4.

## 9. D6 — loud fallbacks, and D7, D8

**D6. Remove the fallbacks first, then warn for what is left.** Two silent iOS fallbacks are
removed rather than warned about (r2, Astra):

- **System Back without `navigationBack`.** A pushed route gets the system Back button and
  edge swipe even when no control is named. The host applies the router's own `back` to the
  root routes table, as LLP 1115 D5 promises.
- **A segment's `aria-label`.** A segmented tab whose `aria-label` differs from its visible
  text keeps the native control: the text is the segment's title, and the label is its
  accessibility label.

Then the warning channel:

- **The channel.** `contract build --json` gains `"severity": "warning"` diagnostics. They do
  not fail the build or a test, and the CLI prints them after errors. Lowering today can only
  refuse (`lint.rs:180`); this adds a non-fatal list beside it.
- **First warnings:**
  - `lower-header-unplaced`: a node in a route's `header` that no host's bar places (after D2,
    anything besides a heading, its subtitle text, one search field, one tablist and pressable
    buttons);
  - `lower-single-tab`: a root tab bar with one tab;
  - `type-performance-now-persisted`: a value computed from `performanceNow()` without
    `epochAtZero`, sent to a source or stored in a persisted state.
- Each warning names its repair, as the refusals do.

**D7. Headings take the emphasized weight of their text style.** `heading_style`
(`contract/lower/src/tags.rs:1278`) writes weight 700 for `title1`–`title3`, and keeps 600 for
`headline`.

- On iOS, a navigation bar's large title is UIKit's and does not read this row.
- Content headings get the weight apps actually use: Apple's emphasized text styles, which
  are bold.
- On the web, a bold heading is the web's own default.
- A written `font-weight` still wins.

**D8. Small gaps:**

- **`formatNumber` styles:**
  - `formatNumber(n, "decimal")`: grouped, as `Intl.NumberFormat("en-US")`;
  - `formatNumber(n, "currency", "USD")`: an ISO 4217 code;
  - `formatNumber(n, "percent")`.
  They are deterministic en-US forms, as `formatDate` is, added to the roster, the runner, the
  JS target and the semantics.
- **Randomness:** the `random-and-shuffle` recipe draws a seed from
  `crypto.getRandomValues` in a source, so a shuffle differs on every launch. The data
  module's refusal of `Math.random` names the recipe.
- **`performanceNow()`:** its docs say "since this launch" (not "since boot") in the grammar
  and the guide, and point to `time.epochAtZero + performanceNow()` for anything stored. The
  D6 warning backs it.
- **A determinate `progress`:** `progress value= max=` draws the platform's bar
  (`UIProgressView`, `NSProgressIndicator`, the web's `<progress>`, Linux's painted bar), with
  `aria-valuenow`/`aria-valuemax` carried to accessibility. The refusal at
  `contract/lower/src/controls.rs:422` is removed.
- **`symbol:sf/<name>`:** the unknown-symbol refusal names the `sf/` form after the role list.
- **A favicon:** the web host declares the app's icon, or a default (no `/favicon.ico` 404 on
  every load).

**Not in scope: Snapback.** The Snapback client's iOS and web bugs go to `QUEUE.md` as their
own track, with reproductions from `analysis/batch-c.md`:

- one partition per native device, against the answer model;
- a cold sync over the 100 ms source budget;
- a write that hangs until relaunch;
- reads that stay stale after a write;
- recipe status lines that become UI.

## 10. The bake-off's own changes for round 2

- **The backend:** a builder-written Bun REST + JSON server (`Bun.serve`, `bun:sqlite`).
  Personas are bearer tokens, and live updates are polling. This is Charlie's call of
  2026-10-10, replacing Snapback.
- **Ports:** the Exact2 prompt says that Exact's own commands pick their own ports.
- **Prompts go in on stdin**, never in argv. A grader's `pkill -f` matched four other graders'
  prompts and killed them in round 1.
- **Simulators are reset by uninstalling apps, never erased.** An erase put 16 simulators
  through first boot, and the load reached 170.
- **The specs and grader are unchanged.** Round 2 re-runs the same 24 specs, against the
  Exact2 commit that lands this RFC.
- **Each spec keeps round 1's model and comparison stack**, so a change is measured pair
  against pair (r2, Astra). This is a deliberate change from a fresh coin flip per round.
- **Both arms are rebuilt**, so their wall clock is taken under the same load.
- **The six backend specs change two things at once** (the REST server and this RFC), so they
  are reported separately from the eighteen others.

## 11. Order of work

All in parallel, one lane each (r2: quality is the binding gap, so it does not wait on docs):

1. **D4.** CLI help.
2. **D3.** The core, the recipes, `AGENTS.md`.
3. **D1 + D7.** Web look and heading weight. Both are what the web shows, and D7 is a
   lowering change.
4. **D2.** The iOS host, including the presented-tree check.
5. **D6 + D8.** The two removed fallbacks, the warning channel, the small gaps.
6. **D5.** Persisted state. It merges last.

Each lane runs the five checks, plus the async lane's suites for what it touches (the web JS
target's conformance for D1, `build.mjs --test --ios` for D2). Each lands as its own commit,
naming its D.
