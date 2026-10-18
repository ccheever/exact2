# LLP 1132 — held-out scenario round with Luna 6 (gpt-6-luna, medium), 2026-10-18

Sol's review of the first Luna test (`1132-shown-and-interactive.evals.sol-max.md`) proposed a cheaper and more trustworthy method, which this round follows:

1. The 20 scenarios were revised: scenario 4 gives its commit time; scenarios 2, 3 and 15 no longer state the facts the spec must decide; scenario 6 now tests that a visit does not wait for its sections; scenario 19 adds services that load before a late interactive time; and the shared premises state the web's timing precondition. Nine new scenarios (21 to 29) cover what the first set did not: a split view and the `layout` cause, a visit hidden while it waits for a frame, activation at launch, Observe's screen history, the reasons `closed`, `unmounted` and `ended`, failed activation, compiler checks and a bad first value, a served web page with a redirect and prerendering, and Linux.
2. Every scenario lists the fields its answer must give.
3. The author wrote the expected answers before the run. The RFC (tag `llp-1126-v1`), the scenarios, the expected answers and the prompt were hashed before the run, and the author's grades before the Sol grader's arrived:

```
cf91b13a33d6c57d21ebfc6f907456b760c1285288a2982bff915f90f1b50281  llp/1132-shown-and-interactive.rfc.md
ae0ed40d05dba74cc093737baebe0484341388b5717220d13dd2ce49f511d6a3  scenarios.md
0ed6e074e1ec80d7b0572baf1f3cfe499bf3736dd6e9daef7c24346b70e1bcba  expected.md
633e7b6cb50185a28a6fd7a732383c87cd91c1f534abc3f4237adf7ec84e2d46  prompt.md
9f3fc9b7db67b751a0e90b557ad861e490bd0c9ae8d1889013cca75ee55b6a72  /private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/heldout/author-grades.md
32915ac80c2a66bf584467947af855017f9c8a5107954febdceb4250bc07ec34  prompt-v2.md (v2 run)
3d9b4a09186dc889f835127175f0f806a0e03e018bacff6339bf0cae1e8ce8b4  llp/1132-shown-and-interactive.rfc.md (v2 run)
```

4. Luna 6 ran once and read only the RFC, in four parts so that none of it was cut off.
5. Sol 6.1 (max) graded blind: it wrote its own answer key from the RFC before reading Luna's answers, and never saw the author's key or grades (`1132-shown-and-interactive.heldout-grades.sol-max.md`).

## Results

| | Author | Sol (blind) |
|---|---|---|
| Scenarios 1 to 20, every field correct | 20 | 20 |
| Scenarios 21 to 29, every field correct | 9 | 8 |
| Fields | — | 129 correct, 2 partial, 0 wrong, 0 missing |

Sol's answer key and the author's agree on every value. The one difference is strictness: Sol grades 27(c) partial because Luna gave no reason for the acceptance, and 27(d) partial for "false through 200 ms" where "until" is meant. The author noted 27(d) but did not mark it down.

Sol's grading found three places where a reader must choose between readings, all fixed in a7b66ef3f7: Apple navigation timing was described as running from router ops only; the `router` event's time was not given; and "leaves no events" for a launch that ends unshown could also suppress its raw marks.

## Regression check of v2

After the writing pass and fixes (a7b66ef3f7), Luna answered the same scenarios again. Every value in every field is unchanged; the differences are wording, two of them better (scenario 10 now notes the raw marks, and scenario 3 drops an imprecise reason). By the author's grading both runs are 29/29, and by Sol's strict standard both are 28/29.

## Regression check of v3 (999f709b9e), with scenarios 30 and 31

The shared premises add that on iOS the scene is foreground-active before the first frame, which v3's new wait needs; the expected answers to 1 to 29 do not change. Luna read only the RFC.

Every value in 1 to 29 is the same as in the v1 and v2 runs. 30 (the scene's first foreground-active state) and 31 (input inside a section) are correct. Differences in wording: 27(d) now says "false until 200 ms" (correct); 27(c) still gives no reason for the acceptance; 5 and 15 add the `router` event's time, and in 5 Luna assumes a commit at 0 that the scenario does not give (no field asked for it); 10 adds that services load at the background, 150 (correct).

Author's grade: 31/31. Under the Sol grader's strict standard: 30/31 (27(c) partial). No regression.

The later fix 700bc0ac10 (triggers for a subtree's input needs) changes no answer to these scenarios.

### Runs against the readiness rulings (v4 to v7)

The owner's readiness ruling (2026-10-18) changed the expected answers to 2(f), 3, 9, 11, 20, 22 and 30 before the v4 run, and new scenarios 32 to 36 cover a held and a cancelled back swipe, a covered deep link and a covered change with no frame. Each run's inputs were hashed before it (`frozen.sha256` lines "v4 run" to "v7 run").

### v4 (d3095d8654), with scenario 32

Scenario keys for 2(f), 3, 9, 11, 20, 22 and 30 changed before the run to the owner's readiness ruling. Luna answered every changed scenario and 32 correctly. 16 regressed (cause `program`, start 5): the text did not say where a state change's cause comes from; 50f1c16317 says it.

### v5 (50f1c16317), with scenarios 33 to 35

33 to 35 (a covered deep link, a covered change with no frame, a cancelled swipe) and 16 are correct. Two misses by the frozen key: 12, where Luna made F interactive although D2 cancels a visit that starts after poisoning (Luna's error; the text did not change); and 8, where Luna answered 5,000 for a section shown by a tab selection. The key said 5,016, from the frame rule before the owner's ruling; the ruling and the RFC's own test give 5,000. d5a92f375f says so, and the key for 8 is changed for the next run, with that note in it.

### v6 (d5a92f375f), with scenario 32 asking for Observe's metrics

8 (with its changed key), 12 and 32 are correct. One miss: 22, where Luna took the pushed fullscreen route as ending A's visit; D1 decides it (a sheet route starts a new presentation) but said "a later sheet", and e67ec38a48 says "a later sheet route, modal or fullscreen", and that pushing one leaves the route beneath it in place.

### v7 (e67ec38a48), with scenario 36

36 (a swipe cancelled after its screen became interactive) and 22 are correct. Two slips, neither pointing to unclear text: 8, where Luna gave the interactive time 5,000 and then a cancellation at 5,005, which its own answer rules out; and 22, where Luna gave B, pushed by a timer's action, the cause `input`, which v6 answered `program` under the same Cause text.

Across v4 to v7 each run missed one or two fields, each in a different scenario. Every miss that pointed to unclear text was fixed (16, 8, 22); the others are slips (12 in v5; 8 and 22 in v7).


### Retries of the three last misses (8, 12, 22)

The owner asked for small changes that add clarity, then for Luna to retry the scenarios it last failed. c718082d6d changes three sentences: deselecting a panel cancels only a pending measurement; a commit by a timer's action or a response's handler has the cause `program`, even right after an input; and D1's End says that poisoning and failed activation cancel every marked visit that starts after them. Luna then answered the three scenarios three times, independently, against the unchanged key: 8 and 12 were correct in all three runs, and 22 in two. The third run again took the pushed fullscreen route as ending A's visit. 33f2a3168e defines a sheet route as a route with `navigationPresentation="modal"`, a sheet, or `"fullscreen"`, a fullscreen route; three more runs of 22 were all correct.

### Runs after the readability work (v8 to v10)

The frozen set of 36 scenarios and its key were unchanged; each run's inputs were hashed before it.

- **v8, after the readability pass (de60ddbcec).** Four misses. 22 read the fullscreen route as a non-sheet route: the readability pass had made "sheet" the modal kind while D1's presentations still said "up to the next sheet". 8 repeated the self-contradiction, helped by the scenario's "before that frame". 32 sent a second `tti` and 25 omitted the dialog's `shown`: slips.
- **v9, after the consistency edits and Sol's notes (5ec9e50061).** 8, 12, 22, 25 and 32 correct. Two misses pointing to text: 32's "Unclear" found a test that sent a completed swipe's `tti` against the once-per-screen rule, and 23 read D5's "a resource showing its placeholder" as one that a view displays. One slip: 24 gave a time for a duration.
- **v10, after those two fixes and Sol's term notes (9392b64a2f).** All 36 correct by the key. Under the strict standard, 27(c) is partial, as in every run: it gives the outcome without the reason.

## Scenarios 30 and 31

30. **A scene not yet active.** On iOS, the process starts at 0. The root carries no mark. The launch screen S carries `markinteractive=true`; UIKit shows S, and its first frame is at 300. The data module's activation completes at 350. The scene becomes foreground-active only at 420, after the system's launch animation; nothing on screen changes then. At 2,000 a system alert makes the scene inactive until 2,500. At 2,100 a timer's action pushes screen T, which carries `markinteractive=true`; T's first frame is at 2,140, and nothing on T changes after it.
   Answer: S's interactive time; T's interactive time; which event Observe's startup `tti` comes from, and its time.

31. **Input inside a section.** On the web, screen Q carries `markinteractive=true` and contains only plain native buttons, except for `section id="tools" markinteractive=true` inside it, which contains a `column` with a press handler (a pressable that is not a native control). Q's commit is at 0 and its first frame at 30. The deferred input module finishes loading at 250, which changes nothing on screen. Another route, not in place, also contains a custom pressable. (b) Instead, the section's pressable is inside an `if` block whose condition is false, so it is not mounted, and nothing else on the page needs the input module.
   Answer: Q's interactive time; the section's interactive time; in (b), Q's interactive time.

### Expected answers, written before the run

30. S: 420. It waits for the scene's first foreground-active state, because a foreground-inactive scene receives no events; nothing changed then, so that moment. T: 2,140. The wait applies once per app visit, so the alert's inactivity does not delay T. Observe's startup `tti`: from S's `interactive` at 420.

31. Q: 250. The section's pressable is a mounted descendant of Q, so Q waits for the input module too. Section: 250. The other route is outside Q's subtree and does not count. (b) Q: 30. Nothing mounted in Q needs the input module.

### Luna's answers

30. **A scene not yet active**
- S’s interactive time: 420 ms, when the scene first becomes foreground-active.
- T’s interactive time: 2,140 ms; the scene’s later inactivity under the alert does not pause measurement.
- Startup Observe `tti` comes from S’s `screen` `interactive` event, at 420 ms.
- Sections: D2; D7.
- Unclear: none.

31. **Input inside a section**
- Q’s interactive time: 250 ms; its mounted section’s custom pressable requires the deferred input module.
- `tools` section’s interactive time: 250 ms.
- (b) Q’s interactive time: 30 ms; its only mounted controls are plain native buttons and the conditional pressable is absent.
- Sections: D2; D6; Tests, The web.
- Unclear: none.

## Scenarios

All times are milliseconds on one clock. "Frame at t" means a frame presented at t. Unless a scenario says otherwise: the app uses a router; nothing else is loading; the runtime is ready (activation done, no input module needed); every entrance has ended by the first frame; a native app runs outside the agent; and a web app has a launch module with a web service, so its timing runs.

Answer every field listed under "Answer" with a value. Give durations for Observe's metrics (for example "`cold_ttr` 30 ms"), and give times for Exact's events.

1. **A pushed screen.** On iOS, the user taps a row at 0. The tap's handler commits a push to the Detail screen at 10. Detail's route element carries `markinteractive=(item.loaded)`, and `item` shows its placeholder. UIKit begins to show Detail, and its first frame is at 40. The item's answer lands at 300, and the frame that shows it is at 316. Detail has not been visited before in this session.
   Answer: Detail's cause; start; shown time; interactive time; each metric Observe sends for this navigation, with its value.

2. **A sheet over a pending screen.** As in scenario 1, but at 200 a timer's action (no input) pushes a sheet route, Compose (`navigationPresentation="modal"`, not marked), over Detail. Compose's first frame is at 230. The item lands at 300, which changes Detail's content; the next frame is at 316. Compose has not been visited before in this session.
   Answer: (a) whether Detail's visit ends at 200; (b) Detail's interactive time; (c) what Exact journals for Compose (cause, start, shown, interactive); (d) whether Observe sends a `tti` for Detail; (e) each metric Observe sends for Compose, with its value; (f) Detail's interactive time if UIKit had instead adapted Compose's sheet to fullscreen in a compact height, and Detail's view controller had received `viewDidDisappear` at 260.

3. **A fullscreen cover.** Screen A was shown at 100; its mark is false. At 190 the user taps, and the handler pushes a fullscreen route B (`navigationPresentation="fullscreen"`) at 200; the frame at 216 shows B. At 300 A's resource lands, which makes A's mark true and changes A's text; the next frame is at 316. B stays open until 5,000.
   Answer: A's interactive time on the web; A's interactive time on iOS.

4. **A push away.** Screen A (marked) was shown at 50, and its mark is still false. At 120 the user taps, and the handler commits a push of screen B into the same stack at 125. There is no sheet and no split view. B's first frame is at 140.
   Answer: what Exact journals for A (event, reason, time).

5. **No mark.** The user taps at 0, and screen C is pushed and shown at 40. C carries no `markinteractive`, nothing inside it does, and C has not been visited before in this session.
   Answer: each event Exact journals for C, with its time; each metric Observe sends, with its value.

6. **A section opened by a tap.** Screen D carries `markinteractive=(post.loaded)`. It was shown at 900, and `post` is still loading. At 1,000 the user taps a button. Its handler works for 40 ms and commits at 1,040, which mounts `section id="comments" markinteractive=(comments.loaded)` inside D. The post lands at 1,100, and the frame that shows it is at 1,116. The comments land at 1,200, and the frame that shows them is at 1,216.
   Answer: the section's start; cause; interactive time; D's interactive time.

7. **Marks in loops.** A router app declares its screens as `for e in stack(nav) key=e.id` with each route element `column navigationKey=\`${e.id}\` markinteractive=(feed.loaded)`. Inside the screen, `column id="posts" markinteractive=(feed.loaded)` contains a list, `for post in feed.posts key=post.id`, that renders `article markinteractive=(post.ready)` for each post.
   Answer: the compiler's outcome (accepted or refused, and why) for the route element's mark; for the `posts` column's mark; for the `article`'s mark.

8. **A section in a tab panel.** The root has a `tablist` and panels. Inside the unselected "Stats" panel, outside every route, is `section id="stats" markinteractive=true`, mounted since launch. At 5,000 the user taps the Stats tab, and the first frame that shows the panel is at 5,016.
   Answer: the section's measurement start; cause; interactive time; what Exact would journal if the user had switched away from Stats at 5,005, before that frame (event, reason, time).

9. **A marked root.** On iOS, the root carries `markinteractive=(config.loaded)`. The process starts at 0, and the first frame is at 400. At 600 the user taps to navigate to Settings. The config lands at 1,200, and the frame that shows the change is at 1,216.
   Answer: the launch's shown time; its interactive time; whether the navigation cancels the launch's measurement; without a root mark, the launch's interactive time; without a root mark, which event Observe's startup `tti` comes from.

10. **Background before the first frame.** An iOS app launches; the process starts at 0. At 150, before any frame, the app goes to the background. It returns at 10,000, and the first frame after the return is at 10,040.
   Answer: which visit is the launch; what Exact journals for it; whether the app visit that begins at 10,000 is the launch, and its cause.

11. **Return beneath a cover.** On the web, screen A was shown, and its mark is false. A fullscreen route B covers it. At 15,000 the app goes to the background, and at 20,000 it returns; the first frame after the return is at 20,016, with B still covering A. At 20,300 A's mark becomes true.
   Answer: what Exact journals for A at the background (event, reason, time); A's visit after the return (cause, start, shown, interactive).

12. **Poison.** Screen E (marked, still false) is shown. At 500 the runner is poisoned. At 600 the user swipes back to screen F, which is marked; UIKit begins the back transition itself at 600, and F's first frame is at 620.
   Answer: what Exact journals for E (event, reason, time); for F (each event, with its time and any reason).

13. **Web input readiness.** On the web, screen G carries `markinteractive=true` and contains a `tablist` of native buttons. Its commit is at 0 and its first frame at 30. The deferred input module finishes loading at 250, which changes nothing on screen. In another app, screen H carries `markinteractive=true` and contains only plain native buttons; its commit is at 0 and its first frame at 30.
   Answer: G's interactive time; H's interactive time.

14. **A failed input load.** As screen G in scenario 13, but the input module fails to load at 250. G also contains `section id="actions" markinteractive=true` that holds only plain native buttons and was mounted with G.
   Answer: each event Exact journals for G, with its time and any reason; for the section.

15. **A replace.** The app's routes are `for e in stack(nav) key=e.id`, which renders `column navigationKey=\`${e.id}\` markinteractive=(t.loaded)` for every entry, whatever its route. Screen `/t/1` (its mark still false) was shown at 100. At 400 the user taps, and the handler commits `replace` to `/t/2` at 410. The first frame of `/t/2` is at 430; `/t/2` has not been shown before in this session, and `t.loaded` stays false. Consider only what happens from 400 on (`/t/1`'s own navigation was measured earlier).
   Answer: what Exact journals for `/t/1` (event, reason, time); `/t/2`'s cause, start, shown and interactive; which mark `/t/2`'s visit reads; each metric Observe sends, with its value.

16. **Navigation without a router.** An app has no router. Its root is `column navigationKey=screen` with `state screen = "a"`, and it has two routes, A and B; B carries `markinteractive=(b.loaded)`. A is shown. At 0 the user taps, and the handler sets `screen = "b"` at 5. B's first frame is at 30. `b` lands at 200, and the frame that shows it is at 216.
   Answer: B's cause; start; shown; interactive; whether Observe sends a navigation metric.

17. **A mark that flips.** Screen J was shown at 50. At 300 a change makes J's expression true and changes the screen; the next frame would come at 316. At 305 another change makes it false. At 500 it becomes true again with a visible change, and the frame that shows it is at 516.
   Answer: J's interactive time.

18. **An overdue timer inside a tap.** Screen K is shown and interactive. The user taps at 1,000, and the host dispatches the tap at 1,030. A timer that was due at 990 runs first, at 1,030, and its action mounts `section id="banner" markinteractive=(promo.loaded)` inside K. The tap's own action changes nothing. The promo lands at 1,100, and the frame that shows it is at 1,116.
   Answer: the banner's start; cause; interactive time.

19. **Services.** On iOS, the process starts at 0 and didFinishLaunching is at 120. The app has a stream that never sends its first message. (a) The root carries `markinteractive=(ready)`, which becomes true at 800, and the frame that shows the change is at 816. (b) The same app without a root mark. (c) The root carries `markinteractive=(config.loaded)`, and the config lands only at 40,000; the frame that shows the change is at 40,016.
   Answer: when the services load in (a); in (b); in (c); in (c), whether Exact journals the launch's interactive time, and when.

20. **A macOS sheet.** On macOS, the user clicks at 0, and the handler commits `showModal("prefs")` at 5. The `dialog id="prefs"` carries `markinteractive=true`. The host calls `beginSheet`; the sheet's first frame is at 20, and `beginSheet` returns at 260, when the entrance animation ends.
   Answer: the dialog's cause; start; shown time; interactive time.

21. **A split view.** On the web, a stack shows as a split view: its first route, the list L, is beside a pane that shows the route above it. L carries `markinteractive=(list.loaded)` and was shown at 100. At 200 the user taps a row, and the handler pushes the detail D at 205; D's first frame is at 230. D carries no mark and has not been visited before in this session. (a) The list lands at 300, and the frame that shows it is at 316. (b) Instead, the list lands only at 600, and at 500 the window narrows and the host collapses the split view, applying the new layout at 500, so that D fills the window. (c) Continuing (b), at 800 the window widens, the host applies the split layout at 800, and the frame at 816 shows L beside D.
   Answer: in (a), whether the push ends L's visit, L's interactive time, D's cause, start and shown time, and each metric Observe sends for D with its value; in (b), what Exact journals for L (event, reason, time); in (c), L's new visit's cause, start, shown and interactive times, and whether Observe sends a navigation metric for it.

22. **Hidden while waiting for a frame.** On the web, screen A was shown at 100 after a navigation at 50, and its mark is false. At 300 a change makes A's mark true and changes A's text. At 305 a timer's action commits a push of a fullscreen route B, and the host applies the new stack at 308. The frame at 316 shows B. B carries no mark and has not been visited before in this session.
   Answer: A's interactive time; B's cause, start and shown time; each metric Observe sends for B, with its value; whether Observe sends a `tti` for A's navigation.

23. **Activation at launch.** On iOS, the process starts at 0 and didFinishLaunching is at 120. The root carries no mark. The launch screen S carries `markinteractive=true` and contains `section id="feed" markinteractive=(feed.loaded)`. A stream `prices` is read only by a settings screen that is not open, and it never sends. UIKit shows S, and the first frame is at 300. The data module's activation completes at 450 and changes nothing on screen. The feed lands at 600, and the frame that shows it is at 616.
   Answer: the launch's shown time; the launch's interactive time; S's interactive time; the section's interactive time; which event Observe's startup `tti` comes from, and its time; when the services load.

24. **Screen history.** On the web, a router app's launch screen is `/home`, which carries no mark. Screen `/a` carries `markinteractive=(a.loaded)`, where `a` is a root resource that stays loaded once loaded. At 1,000 the user taps, and the handler pushes `/a` as a new entry; its first frame is at 1,030. The resource lands at 1,200, and the frame that shows it is at 1,216. At 2,000 the user taps, and `/b` is pushed; its first frame is at 2,030. At 3,000 the user taps Back, which removes `/b`'s entry; the frame at 3,016 shows `/a`. At 4,000 the user taps Back again, which removes `/a`'s entry; the frame at 4,016 shows `/home`. At 5,000 the user taps, and the handler pushes `/a` as a new entry again; its first frame is at 5,030.
   Answer: for each of the three visits of `/a` (starting at 1,000, 3,000 and 5,000): its interactive time, and each metric Observe sends, with its value.

25. **A dialog, a section and a window.** On macOS, screen S carries `markinteractive=(s.loaded)`; it was shown long ago and its mark is still false. The user clicks at 0, and the handler commits `showModal("share")` at 5. The `dialog id="share"` carries `markinteractive=(contacts.loaded)` and contains `section id="recent" markinteractive=(recent.loaded)`, mounted with the dialog. The sheet's first frame is at 20, and `beginSheet` returns at 260. At 150 the user types in the dialog's search field, and the handler's commit at 152 unmounts the `recent` section. At 400 the user presses Escape, and the dialog closes in the commit at 402. Neither `contacts` nor `recent` ever loads. At 1,000 the user closes the app's only window.
   Answer: what Exact journals for the section, for the dialog and for S (each event, with its reason and time).

26. **Failed activation.** On iOS, the process starts at 0 and didFinishLaunching is at 120. The root carries no mark. The launch screen M carries `markinteractive=true`, and its first frame is at 300. The data module's activation fails at 450. At 600 the user taps, and the handler pushes screen P, which carries `markinteractive=true`, at 605; P's first frame is at 640.
   Answer: each event Exact journals for M, with its time and any reason; for P; when the services load; whether Observe sends a startup `tti`.

27. **Compiler checks and a bad first value.** (a) A component `Card` declares only the prop `title`, and a screen uses `Card(title="x", markinteractive=true)`. (b) A `popover id="more"` contains `column markinteractive=true`. (c) A screen uses a component `Panel` twice; `Panel`'s view contains `section id="p" markinteractive=(ready)`. (d) A route element carries `markinteractive=(summary.ready)`, where `summary` has no static type, and the first value of `summary.ready` is the string `"yes"`. The screen is shown at 50. A commit at 100 that would set `summary.ready` to `true` is refused; an accepted commit at 200 sets it to `true` and changes nothing on screen.
   Answer: the compiler's outcome for (a), (b) and (c), with the reason, and for (c) how many section measurements the screen's visit has; for (d), whether the mark is registered, its value until 200, and the screen's interactive time.

28. **A served web page and a redirect.** On the web, a page for `/` that was not prerendered is served with its content; its navigation timing origin is 0. First Contentful Paint reports a paint at 40 first; Element Timing later reports the first text painted at 45. The script's first commit is at 100. At 120 a response arrives, and its handler commits a `replace` to `/login`; the rendering opportunity after it is at 136. Neither screen is marked, and `/login` has not been shown before in this session. (b) Instead, the page was prerendered, and the user activated it at 5,000.
   Answer: the app visit's shown time; `/`'s shown time; the launch screen; `/login`'s cause, start and shown time; each metric Observe sends for `/login`, with its value; in (b), the moment the launch is timed from.

29. **Linux.** On Linux with a DRM display, the app is unmarked. The display opens at 50, and the first page flip is at 80. At 1,000 the user taps, and the handler commits `showModal("help")` at 1,005; `dialog id="help"` carries `markinteractive=true`. (b) The same app runs headless.
   Answer: the launch's shown time; when the launch modules' services start; what Exact journals for the dialog; in (b), what timing Exact journals.

## Expected answers

1. Cause: input. Start: 0. Shown: 40. Interactive: 316. Observe: `cold_ttr` 40 ms; `tti` 316 ms.

2. (a) No: a sheet neither pauses nor ends the visit beneath it. (b) 316: a selected sheet that is not fullscreen leaves the route just beneath it drawn, so Detail is not hidden and the frame rule applies. (c) Compose: cause `program`, start 200, shown 230; no interactive time (its `shown` says it is not marked). (d) No: Compose's newer navigation ended Observe's measurement of Detail; Exact still journals Detail's `interactive`. (e) `cold_ttr` 30 ms; no `tti`. (f) 300: UIKit stopped drawing Detail at 260, so Detail is hidden when its conditions come to hold, and its interactive time is that moment.

3. Web: 300. The web hides the routes beneath a fullscreen route, so A is hidden when its conditions come to hold, and it does not wait for B to close. iOS: 316. iOS presents a fullscreen route over the screen and keeps drawing beneath it, so A is not hidden and the frame rule applies.

4. `cancelled`, reason `left`, at 125 (the push's commit).

5. `shown` at 40 (cause input, start 0), saying that C is not marked. No `interactive` and no `cancelled`. Observe: `cold_ttr` 40 ms; no `tti`.

6. Section: start 1,000 (the tap); cause input; interactive 1,216. D: 1,116; a visit's interactive time does not wait for its sections.

7. Route element: accepted (a screen visit; the stack's `for` is above its owner). `posts` column: accepted (no `for` row between it and its owner). `article`: refused, because a `for` row lies between it and its owner; the error says to put `markinteractive` on the element that contains the list.

8. Start: 5,000. Cause: input. Interactive: 5,016. Switched away at 5,005: `cancelled`, reason `left`, at 5,005.

9. Shown: 400. Interactive: 1,216. The navigation does not cancel the launch's measurement. Without a root mark: no interactive time for the launch; Observe's startup `tti` comes from the first screen `interactive` in the launch session before the app goes to the background.

10. The launch is the session's first app visit, which began with the process at 0. It ended unshown at 150 and leaves no events. The visit at 10,000 is not the launch; its cause is `foreground`.

11. At the background: `cancelled`, reason `background`, at 15,000. After the return: cause `foreground`, start 20,000, shown 20,016, interactive 20,300 (hidden beneath B on the web, so the moment the conditions hold).

12. E: `cancelled`, reason `poisoned`, at 500. F: `shown` at 620 (cause input, start 600), then `cancelled`, reason `poisoned`, at 620.

13. G: 250. H: 30.

14. G: `shown` at 30, then `cancelled`, reason `input_failed`, at 250. Section: `interactive` at 30.

15. `/t/1`: `cancelled`, reason `left`, at 410. `/t/2`: cause input, start 400, shown 430, no interactive time. It reads the same route element's mark, `t.loaded` (the template keeps one element per entry, whatever its route), which is false. Observe: `cold_ttr` 30 ms; no `tti`.

16. Cause: input. Start: 0. Shown: 30. Interactive: 216. No navigation metric: no router change started the visit.

17. 516.

18. Start: 1,030. Cause: `program`. Interactive: 1,116.

19. (a) 816. (b) 30,120: the stream keeps the launch unsettled, so the 30 s bound after didFinishLaunching applies. (c) 30,120; yes, Exact journals the launch's interactive time at 40,016, because loading the services does not end measurement.

20. Cause: input. Start: 0. Shown: 20. Interactive: 260.

21. (a) No: L stays in place as the split view's first route beside the pane. L: 316. D: cause input, start 200, shown 230. Observe for D: `cold_ttr` 30 ms; no `tti`. (b) L: `cancelled`, reason `left`, at 500. (c) Cause `layout`, start 800, shown 816, interactive 816. No navigation metric.

22. A: 308. Its conditions held at 300 while it waited for the frame that shows the change, and it became hidden at 308. B: cause `program`, start 305, shown 316. Observe for B: `cold_ttr` 11 ms; no `tti`. No `tti` for A: B's newer navigation ended Observe's measurement of A at 305, before A's interactive time.

23. Launch shown: 300. Launch interactive: none (the root carries no mark). S: 450 (it waits for activation; nothing changed then, so that moment). Section: 616. Observe's startup `tti`: from S's `interactive` at 450. Services: 30,120. Settled covers the whole app, so `prices` keeps the launch unsettled, and the 30 s bound after didFinishLaunching applies.

24. Visit at 1,000: interactive 1,216; `cold_ttr` 30 ms, `tti` 216 ms. Visit at 3,000: interactive 3,016; `warm_ttr` 16 ms; no `tti` (Observe sends one per screen). Visit at 5,000: interactive 5,030; `cold_ttr` 30 ms, `tti` 30 ms (the removal at 4,000 forgot the old entry, and the new entry is a new screen).

25. Section: `cancelled`, reason `unmounted`, at 152. Dialog: `shown` at 20 (cause input, start 0), then `cancelled`, reason `closed`, at 402. S: `cancelled`, reason `ended`, at 1,000 (closing the window ends its session).

26. M: `shown` at 300, then `cancelled`, reason `activation_failed`, at 450. P: `shown` at 640 (cause input, start 600), then `cancelled`, reason `activation_failed`, at 640. Services: 450. Observe sends no startup `tti`: no screen in the launch session becomes interactive.

27. (a) Refused with `type-unknown-prop`: a component use accepts only its declared props. (b) Refused: `markinteractive` inside a `popover`. (c) Accepted; two section measurements, one per use. (d) Registered; it counts as false until 200 (the runner clears the invalid value, and the refused commit at 100 changes nothing); interactive at 200.

28. App visit shown: 40. `/` shown: 40. Launch screen: `/`. `/login`: cause `program`, start 120, shown 136. Observe for `/login`: `cold_ttr` 16 ms; no `tti`. (b) 5,000.

29. Launch shown: 80. Services start when the display opens, at 50, on their own thread. The dialog: nothing, because a DRM display refuses `showModal`, so there is no dialog visit. (b) None: a headless run reports no timing.

## The author's grades

Graded field by field against `expected.md` and the RFC at `llp-1126-v1`.

Scenarios 1 to 20 (revised): 20 of 20 with every field correct.
Scenarios 21 to 29 (new): 9 of 9 with every field correct.

Notes, none of which changes a value:
- 3 (web): the value 300 is right, but the reason "when it becomes hidden while its mark is true" is imprecise. A became hidden when B was laid out, before 300; the rule that decides is "hidden when the conditions come to hold".
- 9: Luna adds that the scenario does not say whether Settings is marked. The field asks only where the startup `tti` comes from, which Luna answers.
- 27 (d): "false through 200 ms" should read "false until 200 ms"; the interactive time, 200, is right.

## Luna's answers against `llp-1126-v1`

1. **A pushed screen**
- Detail’s cause: `input`.
- Start: `0 ms`.
- Shown time: `40 ms`.
- Interactive time: `316 ms`.
- Observe metrics: `cold_ttr` `40 ms`; `tti` `316 ms`.
- Sections: D1 Cause, Start, Shown; D2; D7.
- Unclear: none.

2. **A sheet over a pending screen**
- (a) Detail’s visit ends at 200 ms: **no**; the sheet does not end or pause it.
- (b) Detail’s interactive time: `316 ms`.
- (c) Compose: cause `program`; start `200 ms`; shown `230 ms`; interactive time: none, because it has no mark.
- (d) Observe sends a `tti` for Detail: **no**; Compose’s newer navigation supersedes Detail’s pending navigation measurement.
- (e) Compose’s Observe metrics: `cold_ttr` `30 ms`; no `tti`.
- (f) If UIKit adapts the sheet to fullscreen and Detail receives `viewDidDisappear` at 260 ms, Detail’s interactive time is `300 ms`.
- Sections: D1 Visits, Cause, Start, Shown; D2; D7.
- Unclear: none.

3. **A fullscreen cover**
- A’s web interactive time: `300 ms`, when it becomes hidden while its mark is true.
- A’s iOS interactive time: `316 ms`, when the frame shows the change; iOS keeps drawing beneath a fullscreen route.
- Sections: D1 Visits; D2.
- Unclear: none.

4. **A push away**
- A: `cancelled`, reason `left`, at `125 ms`.
- Sections: D1 End; D2.
- Unclear: none.

5. **No mark**
- C’s visit event: `shown` at `40 ms`; no `interactive` event because C has no mark.
- Observe metrics: `cold_ttr` `40 ms`; no `tti`.
- Sections: D2; D3 Common cases; D7.
- Unclear: none.

6. **A section opened by a tap**
- Section start: `1,000 ms`.
- Section cause: `input`.
- Section interactive time: `1,216 ms`.
- D’s interactive time: `1,116 ms`.
- Sections: D1 Sections; D2; D6.
- Unclear: none.

7. **Marks in loops**
- Route element’s mark: **accepted**; the route is the visit’s own element, and route rows are allowed.
- `posts` column’s mark: **accepted**; it is a section outside any list row.
- Each `article` mark: **refused**; a `for` row lies between the section and its screen owner.
- Sections: D1 Visits and Sections; D3 Compiler checks.
- Unclear: none.

8. **A section in a tab panel**
- Section measurement start: `5,000 ms`.
- Cause: `input`.
- Interactive time: `5,016 ms`.
- If the user switches away at `5,005 ms` before the frame, Exact journals `cancelled`, reason `left`, at `5,005 ms`.
- Sections: D1 Sections; D2; D7.
- Unclear: none.

9. **A marked root**
- Launch shown time: `400 ms`.
- Launch interactive time: `1,216 ms`.
- The navigation cancels the launch measurement: **no**.
- Without a root mark, launch interactive time: none.
- Without a root mark, Observe startup `tti` comes from the first screen `interactive` event in the launch session before backgrounding, if one occurs. No such event is specified here.
- Sections: D1 App; D2; D4; D7.
- Unclear: The scenario does not say whether Settings has a mark that would produce a screen `interactive` event.

10. **Background before the first frame**
- Launch: the initial app visit that began at process start (`0 ms`).
- Exact journals for that launch: no events; it ended before its first frame.
- The app visit beginning at `10,000 ms` is the launch: **no**. Its cause is `foreground`.
- Sections: D1 App, End; D4.
- Unclear: none.

11. **Return beneath a cover**
- At background, A: `cancelled`, reason `background`, at `15,000 ms`.
- A’s return visit: cause `foreground`; start `20,000 ms`; shown `20,016 ms`; interactive `20,300 ms`.
- Sections: D1 Visits, Start, Shown; D2.
- Unclear: none.

12. **Poison**
- E: `cancelled`, reason `poisoned`, at `500 ms`.
- F: `shown` at `620 ms`, then `cancelled`, reason `poisoned`, at `620 ms`; no interactive time.
- Sections: D1 End; D2.
- Unclear: none.

13. **Web input readiness**
- G’s interactive time: `250 ms`; its `tablist` needs the deferred input module.
- H’s interactive time: `30 ms`; plain native buttons need neither deferred input part.
- Sections: D2.
- Unclear: none.

14. **A failed input load**
- G: `shown` at `30 ms`; `cancelled`, reason `input_failed`, at `250 ms`.
- `actions` section: `interactive` at `30 ms`; its plain native buttons do not need the failed module.
- Sections: D1 Shown; D2.
- Unclear: none.

15. **A replace**
- `/t/1`: `cancelled`, reason `left`, at `410 ms`.
- `/t/2`: cause `input`; start `400 ms`; shown `430 ms`; no interactive time.
- `/t/2` reads the mark on the route element then in place: `t.loaded`, still false.
- Observe metrics: `cold_ttr` `30 ms`; no `tti`.
- Sections: D1 End, Cause, Start; D2; D7.
- Unclear: none.

16. **Navigation without a router**
- B’s cause: `input`.
- Start: `0 ms`.
- Shown: `30 ms`.
- Interactive: `216 ms`.
- Observe navigation metric: **none**; the state change does not produce a router change.
- Sections: D1 Visits, Cause, Start, Shown; D2; D7.
- Unclear: none.

17. **A mark that flips**
- J’s interactive time: `516 ms`. The earlier true value becomes false before the frame that would have shown it.
- Sections: D2.
- Unclear: none.

18. **An overdue timer inside a tap**
- Banner start: `1,030 ms`.
- Cause: `program`.
- Interactive time: `1,116 ms`.
- Sections: D1 Sections; D2; D6.
- Unclear: none.

19. **Services**
- (a) Services load at `816 ms`, at the root’s interactive time.
- (b) Services load at `30,120 ms`: 30 seconds after the `120 ms` didFinishLaunching anchor, since the never-sending stream prevents settling.
- (c) Services load at `30,120 ms`, at the 30-second bound.
- (c) Exact does journal the launch’s interactive time: `40,016 ms`.
- Sections: D4 Services; D5 Settled.
- Unclear: none.

20. **A macOS sheet**
- Dialog cause: `input`.
- Start: `0 ms`.
- Shown time: `20 ms`.
- Interactive time: `260 ms`, when `beginSheet` returns and the entrance ends.
- Sections: D1 Dialog, Cause, Start, Shown; D2.
- Unclear: none.

21. **A split view**
- (a) The push ends L’s visit: **no**; L remains in place beside the pane.
- L’s interactive time: `316 ms`.
- D’s cause: `input`; start `200 ms`; shown `230 ms`.
- D’s Observe metrics: `cold_ttr` `30 ms`; no `tti`.
- (b) L: `cancelled`, reason `left`, at `500 ms`, when the split collapses.
- (c) L’s new visit: cause `layout`; start `800 ms`; shown `816 ms`; interactive `816 ms`.
- (c) Observe navigation metric for L’s new visit: **none**; its cause is `layout`.
- Sections: D1 Visits, Cause, Start, Shown, End; D2; D7.
- Unclear: none.

22. **Hidden while waiting for a frame**
- A’s interactive time: `308 ms`, when it becomes hidden while waiting for the frame that would show its change.
- B’s cause: `program`; start `305 ms`; shown `316 ms`.
- B’s Observe metrics: `cold_ttr` `11 ms`; no `tti`.
- Observe sends a `tti` for A’s navigation: **no**; B’s newer navigation supersedes it.
- Sections: D1 Cause, Start, Shown; D2; D7.
- Unclear: none.

23. **Activation at launch**
- Launch shown time: `300 ms`.
- Launch interactive time: none; the root is unmarked.
- S’s interactive time: `450 ms`; activation completes without changing the screen.
- Feed section interactive time: `616 ms`.
- Observe startup `tti`: comes from S’s `interactive` event at `450 ms`.
- Services load at `30,120 ms`; the unopened settings stream keeps the app unsettled until the 30-second bound.
- Sections: D1 App; D2; D4; D5; D7.
- Unclear: none.

24. **Screen history**
- `/a` visit starting at `1,000 ms`: interactive at `1,216 ms`; Observe sends `cold_ttr` `30 ms` and `tti` `216 ms`.
- `/a` visit starting at `3,000 ms`: interactive at `3,016 ms`; Observe sends `warm_ttr` `16 ms`; no `tti`, because Observe records one TTI per screen.
- `/a` visit starting at `5,000 ms`: interactive at `5,030 ms`; Observe sends `cold_ttr` `30 ms` and `tti` `30 ms`. The earlier entry was removed, so it is forgotten from screen history.
- Sections: D1 End, Start, Shown; D2; D7.
- Unclear: none.

25. **A dialog, a section and a window**
- `recent` section: `cancelled`, reason `unmounted`, at `152 ms`.
- `share` dialog: `shown` at `20 ms`; `cancelled`, reason `closed`, at `402 ms`.
- S: `cancelled`, reason `ended`, at `1,000 ms`.
- Sections: D1 Sections, Dialog, End; D2.
- Unclear: none.

26. **Failed activation**
- M: `shown` at `300 ms`; `cancelled`, reason `activation_failed`, at `450 ms`.
- P: `shown` at `640 ms`; `cancelled`, reason `activation_failed`, at `640 ms`.
- Services load at `450 ms`.
- Observe sends a startup `tti`: **no**.
- Sections: D1 Shown, End; D2; D4; D7.
- Unclear: none.

27. **Compiler checks and a bad first value**
- (a) Refused: `type-unknown-prop`; `Card` does not declare `markinteractive`.
- (b) Refused: `markinteractive` inside a `popover` is not allowed.
- (c) Accepted; the two uses produce two section measurements.
- (d) The mark is registered. Its value is false through `200 ms`: the invalid first value is cleared, and the refused commit at `100 ms` changes nothing. The screen becomes interactive at `200 ms`, when the accepted commit sets the mark true; nothing on screen changed.
- Sections: D3 Compiler checks, Components and scope; D6; D2.
- Unclear: none.

28. **A served web page and a redirect**
- App visit shown time: `40 ms`.
- `/` shown time: `40 ms`.
- Launch screen: `/`.
- `/login`: cause `program`; start `120 ms`; shown `136 ms`.
- Observe metrics for `/login`: `cold_ttr` `16 ms`; no `tti`.
- (b) For the prerendered page, the launch is timed from activation at `5,000 ms`.
- Sections: D1 Cause, Start, Shown; D4; D7.
- Unclear: none.

29. **Linux**
- Launch shown time: `80 ms`.
- Launch modules’ services start when the display opens, at `50 ms`.
- Dialog: no visit or timing events; DRM refuses `showModal`.
- (b) Headless: Exact journals no timing; a headless Linux run reports none.
- Sections: D1 Dialog; D4 Services; D7.
- Unclear: none.
