# LLP 1132 — blind grading of Luna 6's held-out answers by Sol (gpt-6.1-sol, max), against `llp-1126-v1`, 2026-10-18

I fixed the key before reading Luna. All times below are milliseconds; Observe metric values are **durations**. Line references are to [the spec](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md).

Journal fields cover visit/section events during the described actions. The scope ambiguities involving router records, raw launch marks and earlier events are listed afterward. An answer matching an allowed reading earns **correct**.

| Scenario | Field | My value | Spec lines |
|---|---|---|---|
| 1 | Detail cause | `input` — tap | 76–93 |
| 1 | Start | 0 | 85–93 |
| 1 | Shown | 40 | 101–103 |
| 1 | Interactive | 316 | 168–179, 193, 198 |
| 1 | Observe metrics | `cold_ttr` 40 ms; `tti` 316 ms | 85–93, 422–424 |
| 2(a) | Detail ends at 200? | No | 49–60, 128 |
| 2(b) | Detail interactive | 316 | 56, 128, 178, 193 |
| 2(c) | Compose cause | `program` | 76, 88 |
| 2(c) | Compose start | 200 | 85–88 |
| 2(c) | Compose shown | 230 | 101–103 |
| 2(c) | Compose interactive | None: unmarked. Its visit journals `shown`, without `interactive`. | 20–22, 289, 386 |
| 2(d) | Observe `tti` for Detail? | No: Compose supersedes Observe’s pending navigation measurement. | 422–424 |
| 2(e) | Observe metrics for Compose | `cold_ttr` 30 ms; no `tti` | 85–93, 422–424, 429 |
| 2(f) | Detail interactive under adapted fullscreen sheet | 300: already hidden since 260 | 56, 195–198, 461 |
| 3 | A interactive, web | 300: conditions hold while already hidden | 56, 195 |
| 3 | A interactive, iOS | 316: fullscreen route leaves A drawn | 56, 193, 461 |
| 4 | A journal | `cancelled(left)` at 125 | 58–60, 118, 200–202 |
| 5 | C visit events | `shown` at 40, marked false; no `interactive`. Separate router-record scope is discussed below. | 101, 289, 385–386 |
| 5 | Observe metrics | `cold_ttr` 40 ms; no `tti` | 85–93, 422–424, 429 |
| 6 | Section start | 1,000 | 140–146 |
| 6 | Section cause | `input` — tap | 140–146 |
| 6 | Section interactive | 1,216 | 168–179, 193 |
| 6 | D interactive | 1,116; does not wait for comments | 168–179, 193, 276 |
| 7 | Route mark | Accepted: it marks the visit; the route-producing row is not below its owner. | 253–256 |
| 7 | `posts` mark | Accepted: no `for` row lies between it and its route owner. | 255–256 |
| 7 | `article` mark | Refused: the posts loop lies between this section and its route owner. | 255, 262 |
| 8 | Section start | 5,000 | 134–146 |
| 8 | Section cause | `input` — tab tap | 136, 140–146 |
| 8 | Section interactive | 5,016 | 168–198, 476 |
| 8 | Journal if deselected at 5,005 | `cancelled(left)` at 5,005; no interactive or section-shown event | 136, 166, 200–202, 476 |
| 9 | Launch shown | 400 | 70–72, 101–103, 302 |
| 9 | Launch interactive | 1,216 | 168–179, 193, 303 |
| 9 | Navigation cancels launch? | No | 120, 303, 483 |
| 9 | Unmarked launch interactive | None | 306 |
| 9 | Unmarked startup `tti` source | First screen `interactive` in the launch session before background, if any. No particular qualifying event is established here. | 306, 418–420, 429 |
| 10 | Which visit is launch? | Initial app visit: starts at 0, ends unshown at 150 | 70–72, 91, 296–298 |
| 10 | Launch journal | No visit/section events. Raw launch marks are separately required; see ambiguity below. | 120–124, 298, 326, 387 |
| 10 | Return visit is launch? | No | 298 |
| 10 | Return cause | `foreground` | 89, 95, 298 |
| 11 | A journal at background | `cancelled(background)` at 15,000 | 95–97, 120, 200–205 |
| 11 | New A cause | `foreground` | 95 |
| 11 | New A start | 20,000 | 89, 95 |
| 11 | New A shown | 20,016, despite the cover | 97, 462 |
| 11 | New A interactive | 20,300 | 168, 195, 462 |
| 12 | E journal | New event: `cancelled(poisoned)` at 500. Its earlier shown event is given as a premise. | 101, 120, 200–211 |
| 12 | F journal | `shown` and `cancelled(poisoned)`, both at 620; no interactive | 211–219, 481 |
| 13 | G interactive | 250 | 168–177, 194 |
| 13 | H interactive | 30 | 168–177 |
| 14 | G journal | `shown` at 30; `cancelled(input_failed)` at 250 | 101, 173–177, 200–211 |
| 14 | Actions section journal | `interactive` at 30; no shown event or cancellation from this input failure | 168–177, 211–217, 386 |
| 15 | `/t/1` journal | `cancelled(left)` at 410 | 64, 118, 200–202 |
| 15 | `/t/2` cause | `input` — tap | 76–93 |
| 15 | `/t/2` start | 400 | 85–93 |
| 15 | `/t/2` shown | 430 | 101–103 |
| 15 | `/t/2` interactive | None yet: mark remains false | 168–179, 198 |
| 15 | Mark read by `/t/2` | Same mounted route element’s `t.loaded` mark | 64 |
| 15 | Observe metrics | `cold_ttr` 30 ms; no `tti` yet | 85–93, 423–424 |
| 16 | B cause | D1 reading: `input` — tap. D7’s Apple router-op wording creates a competing recording reading; see gap below. | 66, 76, 392, 456 |
| 16 | B start | D1 reading: 0. Literal router-only Apple reading: timing unrecorded. | 66, 85–93, 392 |
| 16 | B shown | D1 reading: 30. Literal router-only Apple reading: timing unrecorded. | 66, 101, 392, 456 |
| 16 | B interactive | D1–D2 reading: 216. Literal router-only Apple reading: timing unrecorded. | 66, 168–198, 392 |
| 16 | Observe navigation metric? | No: no router change | 422, 426 |
| 17 | J interactive | 516 | 193, 198 |
| 18 | Banner start | 1,030: timer’s mounting commit | 140–146, 362, 474 |
| 18 | Banner cause | `program` | 76, 143, 362 |
| 18 | Banner interactive | 1,116 | 168–179, 193 |
| 19(a) | Services load | 816 | 310–314, 348, 486 |
| 19(b) | Services load | 30,120 = didFinishLaunching 120 + 30,000 | 310–314, 322, 348 |
| 19(c) | Services load | 30,120 | 310–314, 322 |
| 19(c) | Late launch interactive journaled? | Yes, at 40,016 | 168–198, 318, 502 |
| 20 | Dialog cause | `input` — click | 68, 76 |
| 20 | Dialog start | 0 | 85–88 |
| 20 | Dialog shown | 20 | 101–104 |
| 20 | Dialog interactive | 260 | 168–178, 194 |
| 21(a) | Push ends L? | No: L remains the first route beside the pane | 51–60, 454 |
| 21(a) | L interactive | 316 | 168–179, 193 |
| 21(a) | D cause | `input` — tap | 76–93 |
| 21(a) | D start | 200 | 85–93 |
| 21(a) | D shown | 230 | 101–106 |
| 21(a) | Observe metrics for D | `cold_ttr` 30 ms; no `tti` | 85–93, 422–424, 429 |
| 21(b) | L journal | `cancelled(left)` at 500 | 62, 118, 200–202, 454 |
| 21(c) | New L cause | `layout` | 66, 76, 90 |
| 21(c) | New L start | 800 | 85, 90 |
| 21(c) | New L shown | 816 | 101–106 |
| 21(c) | New L interactive | 816: already loaded, but must reach this visit’s shown time | 168–179 |
| 21(c) | Observe navigation metric for L? | No | 422, 426 |
| 22 | A interactive | 308: hidden while awaiting the frame | 193–198, 460 |
| 22 | B cause | `program` | 76, 88 |
| 22 | B start | 305 | 85–88 |
| 22 | B shown | 316 | 101–106 |
| 22 | Observe metrics for B | `cold_ttr` 11 ms; no `tti` | 85–93, 423–424 |
| 22 | Observe `tti` for A? | No: B supersedes Observe’s measurement before A becomes interactive | 422–424 |
| 23 | Launch shown | 300 | 70–72, 101–103, 302 |
| 23 | Launch interactive | None: unmarked root | 306 |
| 23 | S interactive | 450: activation completes without a screen change; section and unrelated stream do not hold S’s mark | 168–194, 276 |
| 23 | Feed section interactive | 616 | 168–179, 193 |
| 23 | Startup `tti` source | S’s `interactive` event | 418–420 |
| 23 | Startup `tti` time/value | Event at 450; startup duration 450 ms | 85–91, 418–420 |
| 23 | Services load | 30,120: `prices` holds whole-app settled | 310–314, 322, 338, 348 |
| 24, first visit | `/a` interactive | 1,216 | 168–179, 193 |
| 24, first visit | Observe metrics | `cold_ttr` 30 ms; `tti` 216 ms | 85–93, 423–424 |
| 24, return visit | `/a` interactive | 3,016 | 168–179 |
| 24, return visit | Observe metrics | `warm_ttr` 16 ms; no second `tti` for this entry and URL | 423–424 |
| 24, new entry | `/a` interactive | 5,030 | 168–179 |
| 24, new entry | Observe metrics | `cold_ttr` 30 ms; `tti` 30 ms | 423–424 |
| 25 | Recent section journal | `cancelled(unmounted)` at 152; no shown or interactive | 151, 166, 200–204 |
| 25 | Share dialog journal | `shown` at 20; `cancelled(closed)` at 402; no interactive | 68, 104, 119, 200–203 |
| 25 | S journal | New event: `cancelled(ended)` at 1,000. Earlier `shown` is a premise with no numeric time supplied. | 120–122, 128, 200–206 |
| 26 | M journal | `shown` at 300; `cancelled(activation_failed)` at 450 | 101–103, 120, 200–211 |
| 26 | P journal | `shown` and `cancelled(activation_failed)`, both at 640 | 211–219 |
| 26 | Services load | 450 | 310–314, 318 |
| 26 | Observe startup `tti`? | No | 211–219, 418–420, 429 |
| 27(a) | Card compiler outcome | Refused, `type-unknown-prop`: undeclared component-use prop | 268 |
| 27(b) | Popover compiler outcome | Refused: mark is inside a popover | 254 |
| 27(c) | Panel compiler outcome | Accepted: repeated explicit component uses are permitted | 256, 272 |
| 27(c) | Section measurement count | Two separate measurements, despite shared `id="p"` | 138, 156–162, 256 |
| 27(d) | Mark registered? | Yes, despite invalid first value | 253, 356–357, 477 |
| 27(d) | Value until 200 | False before the accepted commit at 200; true from that commit. Refused commit at 100 changes nothing. | 253, 356–360 |
| 27(d) | Interactive | 200 | 168–194, 356–360 |
| 28 | App shown | 40: FCP reports first | 108–110 |
| 28 | `/` shown | 40: served paint precedes script commit | 112, 490 |
| 28 | Launch screen | `/` | 112, 425 |
| 28 | `/login` cause | `program`: response-driven redirect | 76, 81, 88 |
| 28 | `/login` start | 120: redirect commit | 81, 88 |
| 28 | `/login` shown | 136 | 106, 112 |
| 28 | Observe metrics for `/login` | `cold_ttr` 16 ms; no `tti` | 85–93, 423–425, 429 |
| 28(b) | Launch timed from | Page activation at 5,000 | 91, 323, 492 |
| 29 | Launch shown | 80: first page flip | 101–105 |
| 29 | Services start | 50: display opens | 324, 348 |
| 29 | Help dialog journal | Nothing: DRM refuses `showModal`, so no visit | 68, 465 |
| 29(b) | Headless timing journal | None | 393, 447 |

The grades below treat the two small completeness issues in 27 strictly. Every requested numerical time and metric duration is correct.

| Scenario | Field | Grade | Fault | One-line reason |
|---|---|---|---|---|
| 1 | Detail cause | Correct | — | `input`. |
| 1 | Start | Correct | — | 0. |
| 1 | Shown | Correct | — | 40. |
| 1 | Interactive | Correct | — | 316. |
| 1 | Observe metrics | Correct | — | Both metrics and durations supplied. |
| 2(a) | Detail ends at 200? | Correct | — | Sheet does not end its visit. |
| 2(b) | Detail interactive | Correct | — | 316. |
| 2(c) | Compose cause | Correct | — | `program`. |
| 2(c) | Compose start | Correct | — | 200. |
| 2(c) | Compose shown | Correct | — | 230. |
| 2(c) | Compose interactive | Correct | — | None because unmarked. |
| 2(d) | Observe `tti` for Detail? | Correct | — | Correctly applies Observe supersession. |
| 2(e) | Observe metrics for Compose | Correct | — | `cold_ttr` 30 ms; no `tti`. |
| 2(f) | Detail under adapted sheet | Correct | — | 300. |
| 3 | A interactive, web | Correct | — | Requested time 300 is right; explanatory wording is discussed below. |
| 3 | A interactive, iOS | Correct | — | 316; correctly distinguishes iOS drawing. |
| 4 | A journal | Correct | — | Correct event, reason and commit time 125. |
| 5 | C visit events | Correct | — | Correct visit-event reading; separate router scope is ambiguous. |
| 5 | Observe metrics | Correct | — | `cold_ttr` 40 ms; no `tti`. |
| 6 | Section start | Correct | — | Uses tap timestamp 1,000. |
| 6 | Section cause | Correct | — | `input`. |
| 6 | Section interactive | Correct | — | 1,216. |
| 6 | D interactive | Correct | — | 1,116. |
| 7 | Route mark | Correct | — | Accepted with the visit-element distinction. |
| 7 | `posts` mark | Correct | — | Accepted outside the list’s rows. |
| 7 | `article` mark | Correct | — | Refused for row between section and owner. |
| 8 | Section start | Correct | — | Selection timestamp 5,000. |
| 8 | Section cause | Correct | — | `input`. |
| 8 | Section interactive | Correct | — | 5,016. |
| 8 | Journal if deselected | Correct | — | `cancelled(left)` at 5,005. |
| 9 | Launch shown | Correct | — | 400. |
| 9 | Launch interactive | Correct | — | 1,216. |
| 9 | Navigation cancels launch? | Correct | — | No. |
| 9 | Unmarked launch interactive | Correct | — | None. |
| 9 | Unmarked startup `tti` source | Correct | — | Correct fallback and explicit qualification that no event is established. |
| 10 | Launch identity | Correct | — | Initial app visit remains launch. |
| 10 | Launch journal | Correct | — | Follows D4’s no-visit-events reading. |
| 10 | Return visit is launch? | Correct | — | No. |
| 10 | Return cause | Correct | — | `foreground`. |
| 11 | A journal at background | Correct | — | `cancelled(background)` at 15,000. |
| 11 | New A cause | Correct | — | `foreground`. |
| 11 | New A start | Correct | — | 20,000. |
| 11 | New A shown | Correct | — | 20,016 despite hidden state. |
| 11 | New A interactive | Correct | — | 20,300. |
| 12 | E journal | Correct | — | Correct new cancellation at 500. |
| 12 | F journal | Correct | — | Shown and poisoned cancellation both at 620. |
| 13 | G interactive | Correct | — | 250; tablist requires input module. |
| 13 | H interactive | Correct | — | 30; plain native controls do not wait. |
| 14 | G journal | Correct | — | Shown 30; input failure cancellation 250. |
| 14 | Actions section journal | Correct | — | Interactive 30; unaffected by failed module. |
| 15 | `/t/1` journal | Correct | — | `cancelled(left)` at 410. |
| 15 | `/t/2` cause | Correct | — | `input`. |
| 15 | `/t/2` start | Correct | — | 400. |
| 15 | `/t/2` shown | Correct | — | 430. |
| 15 | `/t/2` interactive | Correct | — | None while its mark remains false. |
| 15 | Mark read | Correct | — | Identifies the in-place route element’s `t.loaded`. |
| 15 | Observe metrics | Correct | — | `cold_ttr` 30 ms; no `tti`. |
| 16 | B cause | Correct | — | Follows explicit D1 router-independent visit rule. |
| 16 | B start | Correct | — | 0 under D1. |
| 16 | B shown | Correct | — | 30 under D1. |
| 16 | B interactive | Correct | — | 216 under D1–D2. |
| 16 | Observe navigation metric? | Correct | — | No router change, hence none. |
| 17 | J interactive | Correct | — | Rejects transient true value; uses 516. |
| 18 | Banner start | Correct | — | Timer commit at 1,030. |
| 18 | Banner cause | Correct | — | `program`. |
| 18 | Banner interactive | Correct | — | 1,116. |
| 19(a) | Services load | Correct | — | 816. |
| 19(b) | Services load | Correct | — | 30,120, including anchor offset. |
| 19(c) | Services load | Correct | — | 30,120. |
| 19(c) | Late interactive journaled? | Correct | — | Yes, 40,016. |
| 20 | Dialog cause | Correct | — | `input`. |
| 20 | Dialog start | Correct | — | 0. |
| 20 | Dialog shown | Correct | — | 20, before entrance ends. |
| 20 | Dialog interactive | Correct | — | 260. |
| 21(a) | Push ends L? | Correct | — | Split first route remains in place. |
| 21(a) | L interactive | Correct | — | 316. |
| 21(a) | D cause | Correct | — | `input`. |
| 21(a) | D start | Correct | — | 200. |
| 21(a) | D shown | Correct | — | 230. |
| 21(a) | Observe metrics for D | Correct | — | `cold_ttr` 30 ms; no `tti`. |
| 21(b) | L journal | Correct | — | `cancelled(left)` at 500. |
| 21(c) | New L cause | Correct | — | `layout`. |
| 21(c) | New L start | Correct | — | 800. |
| 21(c) | New L shown | Correct | — | 816. |
| 21(c) | New L interactive | Correct | — | 816. |
| 21(c) | Observe navigation metric? | Correct | — | None for layout visit. |
| 22 | A interactive | Correct | — | Uses host hiding time 308. |
| 22 | B cause | Correct | — | `program`. |
| 22 | B start | Correct | — | 305. |
| 22 | B shown | Correct | — | 316. |
| 22 | Observe metrics for B | Correct | — | `cold_ttr` 11 ms; no `tti`. |
| 22 | Observe `tti` for A? | Correct | — | Correct supersession result. |
| 23 | Launch shown | Correct | — | 300. |
| 23 | Launch interactive | Correct | — | None for unmarked root. |
| 23 | S interactive | Correct | — | 450 without waiting for feed or prices. |
| 23 | Feed section interactive | Correct | — | 616. |
| 23 | Startup `tti` source | Correct | — | S’s interactive event. |
| 23 | Startup `tti` time/value | Correct | — | 450; process-start baseline is 0. |
| 23 | Services load | Correct | — | 30,120 because whole-app settled remains blocked. |
| 24, first visit | `/a` interactive | Correct | — | 1,216. |
| 24, first visit | Observe metrics | Correct | — | `cold_ttr` 30 ms; `tti` 216 ms. |
| 24, return visit | `/a` interactive | Correct | — | 3,016. |
| 24, return visit | Observe metrics | Correct | — | `warm_ttr` 16 ms; correctly suppresses duplicate `tti`. |
| 24, new entry | `/a` interactive | Correct | — | 5,030. |
| 24, new entry | Observe metrics | Correct | — | `cold_ttr` 30 ms; `tti` 30 ms. |
| 25 | Recent section journal | Correct | — | `cancelled(unmounted)` at 152. |
| 25 | Share dialog journal | Correct | — | Shown 20; closed cancellation 402. |
| 25 | S journal | Correct | — | Correct new event: ended cancellation at 1,000. |
| 26 | M journal | Correct | — | Shown 300; activation failure cancellation 450. |
| 26 | P journal | Correct | — | Shown and activation failure cancellation both at 640. |
| 26 | Services load | Correct | — | 450. |
| 26 | Observe startup `tti`? | Correct | — | No qualifying interactive event. |
| 27(a) | Card compiler outcome | Correct | — | Correct refusal and undeclared-prop reason. |
| 27(b) | Popover compiler outcome | Correct | — | Correct refusal and reason. |
| 27(c) | Panel compiler outcome | **Partial** | **Luna** | Acceptance is right, but the requested reason—repeated explicit component uses are legal—is omitted. |
| 27(c) | Section measurement count | Correct | — | Two. |
| 27(d) | Mark registered? | Correct | — | Yes despite invalid first value. |
| 27(d) | Value until 200 | **Partial** | **Luna** | “False through 200 ms” conflicts with its stated change to true at 200; the endpoint is imprecise. |
| 27(d) | Interactive | Correct | — | 200. |
| 28 | App shown | Correct | — | 40. |
| 28 | `/` shown | Correct | — | 40. |
| 28 | Launch screen | Correct | — | `/`. |
| 28 | `/login` cause | Correct | — | `program`. |
| 28 | `/login` start | Correct | — | 120. |
| 28 | `/login` shown | Correct | — | 136. |
| 28 | Observe metrics for `/login` | Correct | — | `cold_ttr` 16 ms; no `tti`. |
| 28(b) | Launch timed from | Correct | — | Activation at 5,000. |
| 29 | Launch shown | Correct | — | 80. |
| 29 | Services start | Correct | — | 50. |
| 29 | Help dialog journal | Correct | — | Refused dialog creates no visit. |
| 29(b) | Headless timing journal | Correct | — | None. |

| Group | Scenarios with every field correct | Total |
|---|---:|---:|
| Revised, 1–20 | **20** | 20 |
| New, 21–29 | **8** | 9 |
| Overall | **28** | 29 |

Across 131 fields: **129 correct, 2 partial, 0 wrong, 0 missing**. Scenario 27 is the only scenario with partial fields.

| Location | Spec text and readings | Smallest fix or assessment |
|---|---|---|
| **16: router-independent visits versus Apple timing description** | L66: “A route can come into place or leave it through the router, through state, or through layout.” L392: “Apple navigation timing runs from each batch’s router op”. D1 and test L456 require state-driven visits; an exclusive reading of D7 would suppress their Apple timing. The cited `Session.swift:1063` loop handles router operations. | Change L392 to **“Today, Apple navigation timing runs…”** and add: **“Under this RFC, visit timing also follows state and layout changes without a router op.”** Luna follows the explicit D1 rule, so receives credit. |
| **5 and other journal questions: visit events versus router records** | L385 requires “`router`, a new event for each router change.” L386 separately lists visit/section events. “Each event Exact journals for C” can mean C’s visit outcomes or every record concerning the navigation. The RFC does not explicitly define the new router record’s timestamp; scenario 5 supplies no commit time. | In scenarios, ask for **“visit/section events”**. In the RFC, specify **“A router event’s time is its change’s commit time”** if that is intended. The key and grades use the visit-event reading. |
| **10: “no events” versus raw launch marks** | L298: the launch “ends unshown and leaves no events.” L326: “The hosts journal the launch’s raw marks”; L387 repeats that requirement. One reading suppresses every launch record; the more coherent reading suppresses only visit/section outcomes. | Replace “leaves no events” with **“leaves no visit or section events; its raw launch marks remain journaled.”** Luna’s answer follows L298’s visit-outcome reading. |
| **9: no particular fallback event established** | L420 says startup `tti` otherwise comes from “the first screen `interactive` in the launch session before the app goes to the background.” The scenario establishes no screen mark that must produce such an event. | **No RFC fix worth making.** The fallback rule is precise. A scenario seeking a particular event must establish the screen marks. Luna correctly notes the missing facts. |
| **4, 12 and 25: earlier shown events** | L386 lists “`shown`, `interactive` and `cancelled`” events. The scenarios give an already-shown screen as an initial condition, then ask what is journaled; they do not consistently specify the answer’s time window. In 25, S’s earlier shown time is expressly unavailable numerically. | **No RFC fix worth making.** Change the scenario wording to **“What new visit/section events do these actions journal?”** The key treats earlier shown events as premises. |
| **3: already hidden versus becoming hidden** | L195: “If the visit is hidden when the conditions come to hold, use the moment they came to hold.” L196 separately covers a visit that “then becomes hidden” while awaiting a frame. Luna gives the correct requested value, 300, but describes that as when A becomes hidden. A was already hidden. | **No fix worth making.** The two bullets distinguish the cases explicitly. Luna’s explanation should say **“when the conditions hold while A is already hidden.”** The time field remains correct. |
| **27(c): omitted compiler reason** | L256: “It allows repeated uses of a component that contains sections. Each use is written in the source”. Luna states acceptance and the count, but omits the requested reason for acceptance. | **No fix worth making.** Add to Luna’s answer: **“Repeated explicit component uses are permitted; there is no intervening `for` row.”** |
| **27(d): value at the accepted commit** | L356: “After each accepted commit, it updates the set”; L358 updates a touched node’s value; L360 says “A refused commit leaves the set unchanged.” These decide the boundary: false before the accepted commit at 200, true afterward. | **No fix worth making.** Replace Luna’s “false through 200 ms” with **“false until the accepted commit at 200 ms, then true.”** |
