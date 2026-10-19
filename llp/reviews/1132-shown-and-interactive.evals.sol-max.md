# LLP 1132 — review of the Luna 6 scenario test by Sol (gpt-6.1-sol, max), of 248d441ef4, 2026-10-18

## 1. Expected answers

**Scenario 4 has an unsupported expected answer.** It expects cancellation “at 120,” but 120 is the tap’s timestamp; the push’s commit time is unspecified. A’s visit ends when the route leaves its place. The input timestamp establishes B’s measurement start, not A’s cancellation time. Moreover, a push can leave A in place as the first pane of a split view. The correct answer is conditional: `left` when the push actually removes A from place; no cancellation if A remains beside the detail pane. [Expected answer 4](llp/reviews/1132-shown-and-interactive.luna-scenarios.md:78); [RFC L51–64](llp/1132-shown-and-interactive.rfc.md:51), [L85–93](llp/1132-shown-and-interactive.rfc.md:85).

**The other 19 expected answers match their intended cases**, with two qualifications:

- **9:** “The navigation cancels nothing” should say “does not cancel the launch’s measurement.” It can end another screen’s visit. Luna’s answer makes this qualification correctly.
- **Web cases:** the shared premises should explicitly enable timing. D7 says web timing runs only when a launch module has a web service; the scenarios omit that precondition. [RFC L389–396](llp/1132-shown-and-interactive.rfc.md:389).

I found no further incorrect numeric result in the current expected answers.

## 2. Grading

**Run 10: 20 correct rule selections under the intended assumptions, but only 16 fully explicit answers under the prompt’s “exact values” requirement.** I would mark four answers partial, rather than publish an unqualified 20/20:

| Scenario | Missing explicit output |
|---|---|
| 1 | Observe `cold_ttr = 40 ms`, `tti = 316 ms` |
| 2 | Compose’s `cold_ttr = 30 ms` |
| 5 | Observe `cold_ttr = 40 ms` |
| 15 | Observe `cold_ttr = 30 ms`; `/t/1` cancellation at **410** |

For example, run 10 says only “a **cold navigation `ttr`**” in these answers. It supplies the underlying event times, but never states the metric magnitudes. In 1 and 5 the values are immediately inferable because the start is zero; 2 and 15 actually require subtracting the start. Scenario 15 also replaces its cancellation timestamp with “when the replace changes the entry’s URL.” These are omissions, not demonstrated misunderstandings. [Run 10 answers](llp/reviews/1132-shown-and-interactive.luna-scenarios.md:114).

I would **not** penalize answer 4 for declining to invent its missing cancellation time. Answer 9 also deserves credit for explaining that the unmarked-root variant lacks enough information to determine an actual startup `tti`.

The historical attributions are mixed:

- **Run 9, scenarios 1/5/15:** the earlier D7 indeed said history “determines cold or warm” without explicitly stating the direction. Calling this a specification omission is defensible. However, Luna correctly called Compose cold in answer 2 under that same text. The attribution should therefore be **missing explicit definition plus inconsistent application**, not evidence that ambiguity alone caused all three misses. Commit `3c92ab2e78` supplies the missing direction. [Run 9](</private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/luna/luna-r9.md:1>).
- **Run 9, scenario 7:** the Luna attribution holds. The original scenario explicitly said the `posts` column “contains the list”; Luna nevertheless placed that column inside the row. Presenting the nesting in source order improves readability but does not turn the original into an undecidable question.
- **Run 1:** both recorded attributions hold. Answer 2 correctly identified the missing definition of hidden; answer 16 incorrectly declared B interactive at 30 despite the resource arriving at 200. That is a reading error. [Run 1 answers 2 and 16](</private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/luna/luna-r1.md:3>).
- **Run 2, scenario 3:** blaming the specification entirely is too generous. Luna answered 200, explicitly saying A “does not wait for the resource.” The preceding text already required the conditions to hold and the expression to remain true. The clarification was useful; the answer also violated an existing requirement. [Run 2 answer 3](</private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/luna/luna-r2.md:5>).
- **Run 4, scenario 15:** this was not solely a scope problem. Luna invented `/t/1`’s start as 100 and consequently reported `cold_ttr = 0`. Only its shown time was supplied. Narrowing the question prevented the invention; it did not justify it. [Run 4 answer 15](</private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/luna/luna-r4.md:29>).

## 3. Scenario quality

**Several premises supply the classification that the RFC should be tested on.**

- **2** supplies a frame showing Detail beneath the sheet, then explicitly supplies the adapted-sheet disappearance.
- **3** says “the web now hides A”; its iOS variant supplies a frame presenting A’s change.
- **15** says “the same element stays mounted.”

These remain useful tests of downstream timing rules. They do **not** independently demonstrate that a reader can derive host visibility or element retention from the RFC. A reader with the wrong general rule could accept the supplied facts and still pass.

**Scenario 6 weakly tests independence from sections.** D is already interactive. Even an implementation that incorrectly waited for all sections before its first interactive event could pass, provided it never revised an already-recorded time.

**There is repetition without enough opposing cases.** Scenarios 2, 3 and 11 repeatedly test readiness arriving after hiding. None tests the reverse ordering: readiness first, hiding while awaiting its frame. Likewise, every classified navigation is cold; no scenario requires a warm result.

**Scenario 4 is genuinely underspecified**, as discussed above. The nesting in 7 would be clearer as a short Contract fragment. The later clarification of 15 is legitimate scoping, but it narrows what that scenario establishes.

## 4. Coverage

I would prioritize the following gaps. These can replace repeated cases or become short variants; there is no need to expand indefinitely beyond 20.

1. **Visit lifetime and events before showing — highest risk.** Split-view preservation/collapse is absent. So is the section-event buffering rule.  
   **Add:** A remains the list pane when B is pushed; A becomes ready at 300, presented at 316. Expect A’s original visit to become interactive at **316**, without `left`. Separately, a section mounts and unmounts before its owner’s first frame, then the owner leaves unshown: expect **no owner or section timing events**, including no cancellation. [RFC L51–64](llp/1132-shown-and-interactive.rfc.md:51), [L124](llp/1132-shown-and-interactive.rfc.md:124).

2. **Activation and independent readiness.** The shared defaults remove activation delays from every scenario. Screen independence from pending sections and unrelated resources is barely tested.  
   **Add:** An iOS screen is shown at 40 with a literal-true mark, a false child-section mark, and an unrelated never-sending stream. Activation completes at 200 without changing pixels. Expect screen interactive **200**, section still pending, launch unsettled. This single case tests three easily conflated conditions. [RFC L168–179](llp/1132-shown-and-interactive.rfc.md:168), [L276–291](llp/1132-shown-and-interactive.rfc.md:276).

3. **Hidden while awaiting a frame.** This prompted a revision but remains untested in its positive form.  
   **Add:** Shown A becomes true with a visual change at 300; its frame would be 316; a fullscreen route hides it at 310. Expect interactive **310**. Contrast the existing hidden-before-ready case, which yields 300. [RFC L191–198](llp/1132-shown-and-interactive.rfc.md:191).

4. **Observe history and deduplication.** No warm navigation, foreground-populated history, removal/reset, or repeated-screen `tti` is exercised.  
   **Add:** An entry/URL was previously shown through a foreground visit but has never emitted navigation `tti`. Returning to it starts at 1,000, shows at 1,040 and becomes interactive at 1,080: expect **warm_ttr 40**, **tti 80**. A later return must not emit a second `tti`; opening that URL under a new entry after removal is cold. [RFC L422–426](llp/1132-shown-and-interactive.rfc.md:422).

5. **Terminal outcomes and services after timeout.** Untested reasons are `closed`, `unmounted`, `ended` and `activation_failed`. Future measurements after input failure, motion-executor failure, and service loading on early termination are also absent.  
   **Extend 19:** With the marked root still false, services load at **30,120**; readiness changes pixels at 40,000, presented at 40,016. Expect an interactive event at **40,016**, despite services already loading. An activation-failure variant at 200 should cancel pending marked measurements with `activation_failed` and load Apple services then. [RFC L200–218](llp/1132-shown-and-interactive.rfc.md:200), [L310–324](llp/1132-shown-and-interactive.rfc.md:310).

6. **D6 registration and compiler edge cases.** Scenario 18 covers causal attribution, not most of D6.  
   **Add:** A dynamically typed initial mark is nonboolean; a proposed true update at 100 is refused; an accepted true update at 200 changes no pixels. Expect the mark registered as false initially, unchanged at 100, interactive at **200**. This catches missing registration and mutation on refused commits. Also absent: popovers, component props/scope, repeated component copies, and row refusals through components. [RFC L253–268](llp/1132-shown-and-interactive.rfc.md:253), [L356–367](llp/1132-shown-and-interactive.rfc.md:356).

7. **Host-specific launch behavior.** Linux is entirely absent. Web first-paint adoption, prerender activation, observer races/fallback, and redirects around the first paint are absent.  
   **Add:** A served web route paints at 40 before script commit at 100; a redirect commits at 120 and B shows at 136. Expect A shown **40** and selected as launch screen; B starts at **120** and has navigation TTR **16**. A redirect before the first paint instead makes its destination the launch screen. [RFC L108–112](llp/1132-shown-and-interactive.rfc.md:108), [L425](llp/1132-shown-and-interactive.rfc.md:425).

Lower-priority omissions include section identities, remount/revisit measurements, duplicate IDs, nested panels, keyboard and native-transition start attribution, Mac multiwindow launch selection, suppression/raw marks, web journal carryover, D5’s loading inclusions/exclusions, authored hiding/inertness, image/native-view readiness, and offscreen sections. Timing enablement also needs explicit coverage: Apple agent mode, Linux headless and web service gating.

For simplicity, I would **not** add individual scenarios for every D5 exclusion, log rename, frame-statistic policy or complexity bound. Group ledger exclusions into one case; verify performance and host transport behavior through implementation checks.

## 5. What the results show

The exercise demonstrates that one medium-effort model can recover the intended answers from the revised text and increasingly explicit premises. It found useful omissions. Luna generally handled the distinction between framework visits and Observe measurements well.

It does **not** establish that those are the only defensible readings, that another reader will agree, or that an implementation will behave correctly. The author supplied both expectations and grades; some expectations were corrected; scenarios changed; and repeated attempts stopped upon success. The sequence is formative feedback, not a controlled improvement curve. Earlier 20/20 runs followed by 16/20 reinforce that distinction.

The cheapest improvement is to **freeze the RFC, prompt and required answer fields, then independently regrade the existing outputs without consulting the author’s grades**. One fresh run on that frozen package, with a few unseen opposing cases, would add more confidence than another success on the repeatedly revised set.

Overall, these are useful exploratory evaluations, and Luna’s final answers are largely sound. The author’s 20/20 is defensible as a generous judgment of intended rule selection, but overstates exact-answer completeness and coverage. I would retain the exercise as evidence of improved readability, correct scenario 4, qualify the score, and avoid treating it as proof of unambiguity—even on all the tested points.