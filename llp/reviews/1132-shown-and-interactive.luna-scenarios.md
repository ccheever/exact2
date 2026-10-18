# LLP 1132 — scenario test with Luna 6 (gpt-6-luna, medium), 2026-10-17

The owner asked for 20 scenarios that test different parts of the spec, answered by Luna 6 from the RFC alone (it was told to read only the RFC and to say where the text does not decide), graded against the author's expected answers, and for the spec to be improved until Luna answers all of them correctly.

## Runs

| Run | Score | Misses and where the fault was | Change made |
|---|---|---|---|
| 1 | 18/20 | 2: the spec did not define "hidden"; 16: Luna misread the scenario | D1 defines a hidden route; D2 gives the cancel time of a visit that starts after a terminal failure |
| 2 | 18/20 | 3: the spec's "hidden while waiting for a frame" was ambiguous, and Luna also ignored the rule that the expression must be true (it answered 200, before the resource landed); 2: the scenario did not say Compose was a first visit | D2 separates the two hidden cases; scenarios 2 and 5 state first visits |
| 3 | 18/20 | 15: the spec did not say a `replace` keeps the route element, nor what a screen is in Observe's history; 9: a slip | D1 and D7 say both |
| 4 | 19/20 | 15: the scenario did not limit the question to what follows the `replace`, and Luna invented `/t/1`'s start (100) to give it a `cold_ttr` of 0 | scenario 15 scoped |
| 5 | 19/20 | 15: the scenario did not say `/t/2` was new | scenario 15 states it |
| 6 | 19/20 | 9: the same slip as run 3 (816 for 916, the number of scenario 19) | scenario 9 given distinct numbers |
| 7 | 20/20 | none | — |
| 8 | 20/20 | none; on 12, Luna noted that the scenario gives no separate time at which UIKit begins the back transition, which the question does not ask | — |
| 9 | 16/20 | 1, 5, 15: D7 did not say which way the screen history decides between cold and warm, although Luna called Compose cold in 2 under the same text; 7: Luna misread the nesting and refused the container | D7 states that a navigation is warm when its screen was shown before; scenario 7 describes the container before the list |
| 10 | 20/20 on the rules; 16/20 with every value asked for | 1, 2, 5 and 15 name `cold_ttr` without its value, and 15 gives no time for `/t/1`'s cancellation (410) | — |

The author's expected answer to scenario 15 was itself wrong in the first draft: a `replace` keeps the route element, so `/t/2` carries `/t/1`'s mark. Luna saw this; the expected answer was corrected before run 3.

Between runs 7 and 8, Sol's second consensus check (`1132-shown-and-interactive.cons2.sol-max.md`) found that a `replace` keeps the route element only when the template keeps it mounted, and that the definition of a hidden route contradicted itself for a fullscreen route over a sheet. D1 now defines a hidden route by what its host stops drawing, and says that a `replace`'s new visit reads the mark on the element then in place. Scenario 15 now states that the element stays mounted, and scenario 3 also asks for the time on iOS, where UIKit keeps drawing beneath a fullscreen route.

Between runs 8 and 9, Sol's third consensus check (`1132-shown-and-interactive.cons3.sol-max.md`) found that UIKit stops drawing beneath a sheet that it adapts to fullscreen. D1 now takes a hidden route from each host's own drawing state, and scenario 2 also asks about a sheet that UIKit adapts to fullscreen.

## Limits of this test

Sol reviewed the test itself (`1132-shown-and-interactive.evals.sol-max.md`). The author wrote the spec, the scenarios, the expected answers and the grades, revised scenarios between runs, and stopped at the first full score, so the runs are feedback for the text, not a measurement. The final score counts the right rule in each answer; four answers leave out a value that the question asked for. Scenarios 2, 3 and 15 state facts that the spec should decide (that the web hides A, what the frame beneath the sheet shows, that the element stays mounted), so they test the timing rules that follow and not those facts. No scenario tests a split view, a visit hidden while it waits for a frame, a warm navigation, Observe's one `tti` per screen, the reasons `closed`, `unmounted`, `ended` and `activation_failed`, D6's refused commits, Linux, or the web's launch. Web scenarios also assume that timing runs, which D7 limits to an app with a launch module that has a web service.

## Scenarios (as in run 10)

All times are milliseconds on one clock. "Frame at t" means a frame presented at t. Unless a scenario says otherwise, the app uses a router, nothing else is loading, the runtime is ready (activation done, no input module needed), and every entrance has ended by the first frame.

1. **A pushed screen.** On iOS, the user taps a row at 0. The tap's handler commits a push to the Detail screen at 10. Detail's route element carries `markinteractive=(item.loaded)`, and `item` shows its placeholder. UIKit begins to show Detail, and its first frame is at 40. The item's answer lands at 300, and the frame that shows it is at 316. Detail has not been visited before in this session. Give Detail's visit cause, start, shown time and interactive time, and what Observe sends for this navigation.

2. **A sheet over a pending screen.** As in scenario 1, but at 200 a timer's action (no input) pushes a sheet route, Compose (`navigationPresentation="modal"`, not marked), over Detail. Compose's first frame is at 230. The item lands at 300, and the frame at 316 shows it in Detail beneath the sheet. Does Detail's visit end? What is Detail's interactive time? What does Exact journal for Compose? What does Observe send for Detail's `tti`, and for Compose, which has not been visited before in this session? If UIKit had instead adapted Compose's sheet to fullscreen, so that Detail's view controller disappeared at 260, what would Detail's interactive time be?

3. **A fullscreen cover.** On the web, screen A was shown at 100; its mark is false. At 190 the user taps, and at 200 a fullscreen route B (`navigationPresentation="fullscreen"`) is pushed; the web now hides A. At 300 A's resource lands, which makes A's mark true and changes A's text. B stays open until 5,000. What is A's interactive time? What would it be on iOS, where the frame that presents A's change is at 316?

4. **A push away.** Screen A (marked) was shown at 50, and its mark is still false. At 120 the user taps, and the handler pushes screen B into the same stack, with no sheet; B's first frame is at 140. What does Exact journal for A?

5. **No mark.** The user taps at 0, and screen C is pushed and shown at 40. C carries no `markinteractive`, nothing inside it does, and C has not been visited before in this session. What does Exact journal for C, and what does Observe send?

6. **A section opened by a tap.** Screen D was shown and interactive long ago. At 1,000 the user taps a button. Its handler works for 40 ms and commits at 1,040, which mounts `section id="comments" markinteractive=(comments.loaded)` inside D. The comments land at 1,200, and the frame that shows them is at 1,216. Give the section's start, cause and interactive time. Does D's own interactive time change?

7. **Marks in loops.** A router app declares its screens as `for e in stack(nav) key=e.id` with each route element `column navigationKey=\`${e.id}\` markinteractive=(feed.loaded)`. Inside the screen, `column id="posts" markinteractive=(feed.loaded)` contains a list, `for post in feed.posts key=post.id`, that renders `article markinteractive=(post.ready)` for each post. What does the compiler do with each of the three marks?

8. **A section in a tab panel.** The root has a `tablist` and panels. Inside the unselected "Stats" panel, outside every route, is `section id="stats" markinteractive=true`, mounted since launch. At 5,000 the user taps the Stats tab, and the first frame that shows the panel is at 5,016. Give the section's measurement start, cause and interactive time. What would Exact journal if the user had switched away from Stats at 5,005, before that frame?

9. **A marked root.** On iOS, the root carries `markinteractive=(config.loaded)`. The process starts at 0, and the first frame is at 400. At 600 the user taps to navigate to Settings. The config lands at 1,200, and the frame that shows the change is at 1,216. Give the launch's shown time and interactive time, and say whether the navigation cancels anything. If the root carried no mark, what would the launch's interactive time be, and where would Observe's startup `tti` come from?

10. **Background before the first frame.** An iOS app launches; the process starts at 0. At 150, before any frame, the app goes to the background. It returns at 10,000, and the first frame after the return is at 10,040. Which visit is the launch, what does Exact journal for it, and is the app visit that begins at 10,000 the launch?

11. **Return beneath a cover.** On the web, screen A was shown, and its mark is false. A fullscreen route B covers it. At 15,000 the app goes to the background, and at 20,000 it returns; the first frame after the return is at 20,016, with B still covering A. At 20,300 A's mark becomes true. What does Exact journal for A at the background, and for A after the return?

12. **Poison.** Screen E (marked, still false) is shown. At 500 the runner is poisoned. At 600 the user swipes back (a transition UIKit makes itself) to screen F, which is marked, and F's first frame is at 620. What does Exact journal for E and for F?

13. **Web input readiness.** On the web, screen G carries `markinteractive=true` and contains a `tablist` of native buttons. Its commit is at 0 and its first frame at 30. The deferred input module finishes loading at 250, which changes nothing on screen. In another app, screen H carries `markinteractive=true` and contains only plain native buttons; its commit is at 0 and its first frame at 30. Give the interactive times of G and of H.

14. **A failed input load.** As screen G in scenario 13, but the input module fails to load at 250. G also contains `section id="actions" markinteractive=true` that holds only plain native buttons and was mounted with G. What does Exact journal for G and for the section?

15. **A replace.** Screen `/t/1` (marked, still false) was shown at 100. At 400 the user taps, and the handler commits `replace` to `/t/2` at 410; the router keeps the entry and changes its URL, and the route template keys its element by the entry's ID without depending on the route's name, so the same element stays mounted. The first frame of `/t/2` is at 430; `/t/2` has not been shown before in this session. Considering only what happens from 400 on (`/t/1`'s own navigation was measured earlier), what does Exact journal for `/t/1` and for `/t/2`, and what does Observe send?

16. **Navigation without a router.** An app has no router. Its root is `column navigationKey=screen` with `state screen = "a"`, and it has two routes, A and B; B carries `markinteractive=(b.loaded)`. A is shown. At 0 the user taps, and the handler sets `screen = "b"` at 5. B's first frame is at 30. `b` lands at 200, and the frame that shows it is at 216. Give B's cause, start, shown time and interactive time. Does Observe send a navigation metric?

17. **A mark that flips.** Screen J was shown at 50. At 300 a change makes J's expression true and changes the screen; the next frame would come at 316. At 305 another change makes it false. At 500 it becomes true again with a visible change, and the frame that shows it is at 516. What is J's interactive time?

18. **An overdue timer inside a tap.** Screen K is shown and interactive. The user taps at 1,000, and the host dispatches the tap at 1,030. A timer that was due at 990 runs first, at 1,030, and its action mounts `section id="banner" markinteractive=(promo.loaded)` inside K. The tap's own action changes nothing. The promo lands at 1,100, and the frame that shows it is at 1,116. Give the banner's start, cause and interactive time.

19. **Services.** On iOS, didFinishLaunching is at 120. The app has a stream that never sends its first message. (a) The root carries `markinteractive=(ready)`, which becomes true at 800, and the frame that shows the change is at 816. When do the launch modules' services load? (b) The same app without a root mark: when do they load?

20. **A macOS sheet.** On macOS, the user clicks at 0, and the handler commits `showModal("prefs")` at 5. The `dialog id="prefs"` carries `markinteractive=true`. The host calls `beginSheet`; the sheet's first frame is at 20, and `beginSheet` returns at 260, when the entrance animation ends. Give the dialog's cause, start, shown time and interactive time.

## Expected answers

1. Cause: input. Start: 0 (the tap's timestamp). Shown: 40. Interactive: 316. Observe: `expo.navigation.cold_ttr` (Detail's first visit), from start to shown (40 ms), and `tti` from start to interactive (316 ms).

2. Detail's visit does not end: a sheet neither pauses nor ends the visit beneath. Detail's interactive time: 316. Compose: a screen visit, cause `program`, start 200 (the commit), shown 230; no interactive event, because it is not marked. Observe: no `tti` for Detail, because the newer navigation (Compose) ended Observe's measurement of Detail, although Exact journals Detail's interactive; Compose gets `cold_ttr` (30 ms) and no `tti`. (Detail's `cold_ttr` 40 ms was already sent.) With the sheet adapted to fullscreen: 300. UIKit stopped drawing Detail at 260, so Detail is hidden when its conditions come to hold, and its interactive time is that moment.

3. 300: the web stops drawing A, so A is hidden, and the interactive time is the moment the conditions came to hold; it does not wait for B to close. On iOS: 316. UIKit keeps drawing the routes beneath its fullscreen routes, so A is not hidden, and the frame rule applies.

4. A's measurement is cancelled with the reason `left` when the push commits and puts B above A in its presentation. No interactive time. (The first draft said "at 120", but 120 is the tap; the scenario gives no commit time.)

5. A `shown` event at 40 (cause input, start 0), which says that C is not marked. No `interactive` and no `cancelled`. Observe sends `cold_ttr` (40 ms) and no `tti`.

6. Start: 1,000 (the tap's timestamp). Cause: input. Interactive: 1,216. D's own interactive time does not change; a section never holds its visit.

7. The route element's mark: accepted (a screen visit; the stack loop is above its owner). The `column id="posts"` mark: accepted (a section with no `for` row between it and its owner). The `article` mark: refused, because a `for` row lies between it and its owner; the error says to put `markinteractive` on the containing element.

8. Start: 5,000 (the selection's tap). Cause: input. Interactive: 5,016 (the first frame that shows the change). If the user switched away at 5,005: the measurement is cancelled with the reason `left`.

9. Shown: 400. Interactive: 1,216, from the root's mark. The navigation does not cancel the launch's measurement: only the background, the end of the session or the process, poison, a failed activation and a failed input load the root needs cancel the launch's interactive time. Without a root mark: the launch has no interactive time; Observe's module takes its startup `tti` from the first screen `interactive` in the launch session before the app went to the background.

10. The launch is the session's first app visit, which began with the session; it ended unshown at 150 (background) and leaves no events. The app visit that begins at 10,000 has the cause `foreground` and is not the launch.

11. At the background (15,000): A's pending measurement is cancelled with the reason `background`. After the return: a new visit of A, cause `foreground`, start 20,000, shown at 20,016 (it was shown before, so it counts as shown at the first frame after the return although B hides it), interactive at 20,300 (hidden, so the moment the conditions hold).

12. E: cancelled with the reason `poisoned` at 500. F: shown at 620, then cancelled at once with the reason `poisoned`.

13. G: 250 (it waits for the input module because its `tablist`'s arrow keys respond through it; nothing on screen changes then, so it is that moment). H: 30 (its conditions held before it was shown; the interactive time cannot be before the shown time).

14. G: shown at 30, then cancelled with the reason `input_failed` at 250. The section does not need the input module, so it goes on: interactive at 30.

15. `/t/1`: cancelled with the reason `left` at 410 (the `replace` changed its entry's URL). `/t/2`: a new visit, cause input, start 400, shown 430. The template keeps the same route element mounted, so the visit reads the same mark, which nothing in the scenario makes true: no interactive time yet. Observe: a navigation for `/t/2` (a router change), `cold_ttr` 30 ms (a new screen); no `tti` yet for `/t/2`; no `tti` for `/t/1`.

16. B: cause input, start 0, shown 30, interactive 216. Observe sends no navigation metric, because no router change started the visit.

17. 516. The expression must be true at the moment; the true at 300 turned false before the frame that would have shown it.

18. Start: 1,030 (the timer's commit, not the tap's timestamp). Cause: `program`. Interactive: 1,116.

19. (a) At 816, the launch's interactive time (the root is marked). (b) At 30,120: settled never comes because the stream never sends, so the 30 s bound after the anchor (didFinishLaunching at 120) applies.

20. Cause: input. Start: 0. Shown: 20. Interactive: 260 (the entrance must end; nothing changes on screen then, so it is that moment).

## Luna's answers in run 10

1. **A pushed screen.** Detail’s visit has cause `input` (the tap), starts at **0 ms**, is shown at **40 ms**, and becomes interactive at **316 ms**: the mark turns true at 300, and the visible change appears in the frame at 316. Observe sends a **cold navigation `ttr`** and a navigation **`tti`** for Detail. **D1 (Cause, Start, Shown); D2; D7.**

2. **A sheet over a pending screen.** Detail’s visit does **not** end. Its mark becomes true while Detail is still drawn beneath the sheet, so Detail becomes interactive at **316 ms**. Compose is a visit caused by `program`, starting at **200 ms** and shown at **230 ms**; Exact journals its router change and a `shown` event indicating it is unmarked, but no `interactive` event. Observe sends Compose’s **cold navigation `ttr`**, but no `tti` for Compose. The newer navigation supersedes Detail’s still-pending Observe measurement, so Observe sends **no Detail `tti`**, even though Exact later journals Detail’s interactive time. If the adapted sheet hides Detail at 260, Detail becomes interactive at **300 ms**, when its mark turns true while hidden. **D1 (Visits, Cause, Start, Shown, End); D2; D7.**

3. **A fullscreen cover.** On the web, A becomes interactive at **300 ms**: it is hidden when its mark becomes true, so it does not wait for a frame. On iOS, the fullscreen route does not hide A; A becomes interactive at **316 ms**, when the frame shows the change. **D1 (Visits); D2.**

4. **A push away.** Exact journals A’s pending measurement as `cancelled` with reason **`left`** when the push takes A out of place. **D1 (End); D2.**

5. **No mark.** Exact journals C’s `shown` event at **40 ms**, indicating it is unmarked; there is no interactive time. Observe sends a **cold navigation `ttr`** and no `tti`. **D3 (No mark); D7.**

6. **A section opened by a tap.** The section’s measurement starts at **1,000 ms**, with cause **the tap**; it becomes interactive at **1,216 ms**, when the frame shows the comments. D’s own interactive time does not change: each mark is measured separately. **D1 (Sections); D2; D3 (Several marks); D6.**

7. **Marks in loops.** The route element’s mark is accepted: the router’s `for e in stack(nav)` row is the route itself. The `posts` container’s mark is also accepted: it is a section, and no list row lies between it and its route owner. The `article` mark is refused: it is a section inside a list row, between the article and its route owner. **D1 (Sections); D3 (Compiler checks).**

8. **A section in a tab panel.** The section’s measurement starts at **5,000 ms**, with cause **the tab tap**. It becomes interactive at **5,016 ms**, when the frame shows the selected panel. If Stats is deselected at 5,005 while the measurement is pending, Exact journals it as `cancelled` with reason **`left`**; it has no interactive time. **D1 (Sections); D2; D3 (Compiler checks); Tests (Sections).**

9. **A marked root.** The launch is shown at **400 ms** and becomes interactive at **1,216 ms**, when the frame shows the config change. Navigating to Settings does not cancel the launch’s root measurement. With no root mark, the launch has **no interactive time of its own**. Observe then looks for the first screen `interactive` event in the launch session before background; this scenario does not say whether any screen has a mark that becomes interactive, so a startup `tti` cannot be determined. **D4; D7.**

10. **Background before the first frame.** The launch is the app visit that began with the process at **0 ms**. Because it ends unshown when the app backgrounds at 150, Exact records **no times and no `cancelled` event** for it. The visit beginning at **10,000 ms** is a foreground visit, not the launch. **D1 (App, End); D4.**

11. **Return beneath a cover.** At background, Exact journals A’s visit as `cancelled` with reason **`background`**. After return, A gets a new visit with cause `foreground`, start **20,000 ms**, and shown time **20,016 ms**, even though B still covers it. A’s mark turns true while A is hidden, so its new visit becomes interactive at **20,300 ms**. **D1 (App, Start, Shown, End); D2.**

12. **Poison.** Exact journals E’s pending measurement as `cancelled` with reason **`poisoned`** at **500 ms**. F starts after poisoning, so Exact journals it as shown at **620 ms** and cancelled at that shown time, also with reason **`poisoned`**; it never becomes interactive. **D1 (End); D2.**

13. **Web input readiness.** G becomes interactive at **250 ms**: its `tablist` needs the deferred input module for arrow-key handling. H becomes interactive at **30 ms**, its first frame: plain native buttons need neither deferred input nor motion handling. **D2 (Runtime is ready); Tests (The web).**

14. **A failed input load.** Exact journals G as shown, then cancelled with reason **`input_failed`** at **250 ms**. The `actions` section needs neither deferred input nor motion handling; its literal-true mark can become interactive at **30 ms**, the first frame. **D2 (Runtime is ready, failures); Tests (Failures and the launch; The web).**

15. **A replace.** Exact journals `/t/1` as `cancelled` with reason **`left`** when the replace changes the entry’s URL. `/t/2` starts at **400 ms** (the input timestamp), is shown at **430 ms**, and has no interactive time yet: the reused element’s mark is still false. Observe sends a **cold navigation `ttr`** for the new entry URL, and no `tti`. **D1 (End, Start); D2; D7.**

16. **Navigation without a router.** B’s cause is **the tap**; its visit starts at **0 ms**, is shown at **30 ms**, and becomes interactive at **216 ms**, when the frame shows the change. Observe sends **no navigation metric** for this state-based navigation, since its navigation metrics come only from router changes. **D1 (Screen, Cause, Start); D2; D7.**

17. **A mark that flips.** J becomes interactive at **516 ms**. The true value at 300 does not count because it turns false before the frame that would show the change; the later true value remains true through the frame at 516. **D2.**

18. **An overdue timer inside a tap.** The banner measurement starts at **1,030 ms**, with cause **`program`** (the timer’s commit), not the tap: the timer’s action, rather than the input’s own action, mounted it. The banner becomes interactive at **1,116 ms**, when the frame shows the promo. **D1 (Sections, Start); D2; D6.**

19. **Services.**  
   (a) The root mark becomes true at 800 and its visible change is shown at 816, so launch services load at **816 ms**, the root’s interactive time, despite the stream that never sends.  
   (b) Without a root mark, services load at the settled-time bound: **30,120 ms** (30 seconds after `didFinishLaunching` at 120). **D4 (Services); D5.**

20. **A macOS sheet.** The dialog visit’s cause is **the click**, and it starts at **0 ms**. It is shown at **20 ms**, the first frame during sheet presentation. Its literal-true mark cannot become interactive until the entrance ends, when `beginSheet` returns at **260 ms**; that is its interactive time. **D1 (Dialog, Cause, Start, Shown); D2 (Entrance has ended).**
