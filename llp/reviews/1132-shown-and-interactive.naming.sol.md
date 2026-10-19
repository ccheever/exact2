# LLP 1132 — naming discussion with Sol (gpt-6.1-sol, xhigh), 2026-10-17

The owner asked that the reviewer and the author reach consensus on the attribute's name. Three rounds; each of the author's replies is quoted in the next round's prompt.

## Round 1: Sol

**I recommend `interactivetiming=(…)`.** I give the distinction between observation and control more weight than brevity or matching the owner’s phrasing.

D2 strengthens that distinction: the boolean supplies the app’s readiness condition; Exact also waits for runtime readiness and presentation. Thus `interactive=true` can precede the measured interactive moment. [D2](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:94)

My ranked shortlist:

1. **`interactivetiming`**

   **Strongest argument:** It identifies instrumentation while retaining the searchable metric word *interactive*. Its compact lowercase form follows the pattern of `elementtiming`, `fetchpriority` and `autofocus`. This is a new Exact attribute with a familiar web form; it does not borrow an existing attribute’s different meaning. It fits routes, dialogs, the root and identified parts equally well.

   `interactivetiming=true` reads as requesting interactive timing with no additional app wait. `interactivetiming=(feed.loaded or failed(feed))` supplies the condition for that timing. Neither strongly suggests changing input handling.

   **Worst problem:** `interactivetiming=false` sounds like switching measurement off, whereas the RFC registers a declaration whose condition has not passed and may ultimately journal cancellation. A bound expression can similarly read as an instrumentation toggle. Documentation must explicitly call its value a **readiness condition**. This ambiguity is real, but it stays within measurement rather than promising UI behaviour.

2. **`interactive`**

   **Strongest argument:** The shortest direct connection to “time to interactive,” with excellent discoverability for authors and agents. `interactive=(feed.loaded)` reads naturally as a declaration of application state. It works at every permitted scope.

   **Worst problem:** Both literal forms resemble controls: `interactive=true` enables interaction; `interactive=false` disables it. The bound form naturally inherits that interpretation. An author could consequently believe input is blocked while loading. That misunderstanding affects application behaviour.

   HTML’s interactive-content category is a conceptual near-collision, not an attribute collision. More concretely, `interactive-widget` already exists in Exact and controls keyboard-related viewport behaviour. [Lowering](/Users/admin/Developer/observe-lab/exact2/contract/lower/src/tags.rs:600)

3. **`ready`**

   **Strongest argument:** `ready=true`, `ready=false` and `ready=(feed.loaded)` read naturally as readiness assertions. It accommodates screens, dialogs, roots and parts without suggesting that every descendant must handle input.

   **Worst problem:** It leaves “ready for what?” unanswered: painting, data completion, interaction or capture. It loses the metric’s vocabulary, weakens searchability and offers little indication that the attribute exists solely for measurement. The web’s document-readiness terminology adds another nearby meaning.

Across `tags.rs`, `contract/cli/src/vocab/` and `schema.json`, I found no existing attribute/property collision for these three candidates. Existing `disabled`, `inert` and ARIA state mappings make the behavioural interpretation of bare `interactive` especially plausible. `loading` already names a vocabulary concept and describes a different condition: key UI can be usable while other work continues.

I would exclude `aria-busy`, `hidden`, `disabled` and `inert`: each brings established semantics this declaration does not implement. `elementtiming` and `containertiming` likewise bring paint-measurement meanings. `usable` retains `ready`’s vagueness; `interactive-ready` retains the state/control ambiguity. `time-to-interactive` sounds like a duration value. Hyphenating `interactive-timing` is defensible, but the compact spelling better follows the HTML attribute examples.

The project’s naming principle favours established web vocabulary where the meaning matches. Here it supports familiar form without requiring reuse of a mismatched term. [LLP 1115](/Users/admin/Developer/observe-lab/exact2/llp/1115-write-the-web-ship-the-platform.principle.md:48)

**What would change my mind:** independent authors and agents consistently interpreting bare `interactive` as observation, while interpreting `interactivetiming` as a measurement toggle—even after each receives the same one-sentence definition. That evidence would move `interactive` into first place.

## Round 1: the author's reply

I accept your main argument: the name must read as an observation, not a control, and bare `interactive` fails that, because `interactive=false` reads as "disable input" and an author could believe input is blocked while loading. I now drop `interactive` as my first choice.

I have two problems with `interactivetiming`:
1. The family it borrows from has a different value type. `elementtiming="hero"` and `containertiming` take an identifier: a name for the entry (PerformanceElementTiming.identifier, PerformanceContainerTiming.identifier). Here the value is a boolean readiness condition, and the part's name is its `id`. An author who knows Element Timing would write `interactivetiming="feed"` and expect a named entry; the compiler would refuse it as not boolean. The familiar form invites the wrong value.
2. As you note, `interactivetiming=false` reads as "measurement off", and a bound value reads as a toggle. You propose documentation as the remedy; I would rather the name itself say "condition".

My counter-proposal is a name shaped as a condition: `interactivewhen=(…)`.
- `interactivewhen=(feed.loaded or failed(feed))` reads as the sentence it means: interactive when the feed has loaded or failed.
- `interactivewhen=true` reads "interactive when true", that is, as soon as Exact's own conditions hold; `interactivewhen=false` reads as never, which is what it does, with no suggestion of disabling input or switching measurement off.
- It keeps the metric's word for discoverability (time to interactive), is one lowercase token like `fetchpriority`, and has no collision that I know of in HTML, ARIA, CSS or Contract (please check `contract/lower/src/tags.rs`, `contract/cli/src/vocab/`, `kernel/tables/schema.json`).
- Its weaknesses: no web precedent for a "...when" attribute (CSS has the `@when` at-rule, a different thing), and it is longer.

Please judge `interactivewhen` against `interactivetiming` on the same criteria as before, say whether my value-type objection holds, and either agree with one of the two or propose a third that answers both our objections. If you agree, say so plainly in one line at the end: "Consensus: <name>". Keep it under about 500 words.

## Round 2: Sol

**Your value-type objection holds. I withdraw `interactivetiming` as my recommendation and propose `markinteractivewhen`.**

The objection is about expectations: the `…timing` resemblance invites an identifier value, while [D3 requires a boolean condition](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:134) and the part already gets its label from `id`. My round-1 argument gave familiar spelling too much weight without accounting for the familiar value contract.

`interactivewhen` improves the value semantics. A predicate belongs naturally after “when”; literal `true` and `false` fit; it retains *interactive* for discoverability and works equally well on roots, routes, dialogs and parts. It is also shorter than `interactivetiming`.

But it leaves our highest-priority objection unresolved. Consider:

```contract
button interactivewhen=(signedIn)
```

That can readily mean “enable interaction when signed in.” Likewise, `interactivewhen=false` can mean “never allow interaction.” **“When” identifies a condition without identifying whether that condition controls behaviour or observation.** I therefore cannot accept your claim that it removes the disabling-input interpretation.

`markinteractivewhen` supplies both pieces:

- **Observation:** “mark interactive when…” identifies an instrumentation operation.
- **Value:** the condition-shaped ending invites a boolean predicate, without suggesting an entry identifier.
- **Literal meanings:** `true` permits the mark once Exact’s conditions hold; permanent `false` prevents an interactive mark while leaving the declaration pending and eligible for cancellation.
- **Discoverability and scope:** it retains *interactive* and applies naturally to every permitted scope.

The RFC already cites developer marks in User Timing and Observe’s `markInteractive()` as [measurement precedents](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:39). That supports the word *mark*, although this attribute remains new Contract vocabulary.

Its worst problem is length. It could also suggest marking immediately when the predicate becomes true, so the definition must retain Exact’s runtime and presentation requirements. Neither ambiguity suggests changing input handling.

I found no occurrence of `interactivewhen`, `interactivetiming` or `markinteractivewhen` in the requested `tags.rs`, vocabulary directory or schema.

My proposed definition: **“Declares the application readiness condition for recording this visit’s or part’s interactive time; Exact also waits for runtime readiness and presentation.”**

## Round 2: the author's reply

Your example convinces me: `button interactivewhen=(signedIn)` reads as "enable when signed in", so "when" alone does not say "observation". "Mark" is the word that does, and I accept it.

I propose the shorter `markinteractive=(…)` and would accept `markinteractivewhen` if you hold that "when" is needed:
- It keeps your observation signal: "mark" names an instrumentation act, as in User Timing's `performance.mark()`.
- It has a web precedent closer than the timing family: User Timing's Editor's Draft recommends the mark name `mark_interactive`, "the time when the page is considered interactive to an end-user as marked by the developer" (https://w3c.github.io/user-timing/). The RFC cites it in §0.
- The value still reads as a condition: `markinteractive=(feed.loaded or failed(feed))` reads "mark interactive: once the feed has loaded or failed"; `markinteractive=true` "mark it as soon as Exact's own conditions hold"; `markinteractive=false` "do not mark", which is what happens: no interactive time, and a pending measurement that can be cancelled. None suggests enabling or disabling input.
- It is 15 characters against 19, which matters on every route, dialog and part an author or agent writes.
- Its risk is the one you name for yours: it can suggest marking the instant the expression becomes true, so the definition keeps Exact's runtime, entrance and presentation conditions. One more: Observe's API is `markInteractive()`; the owner wants Observe kept apart, but the name comes from User Timing's convention, which Observe also followed, so it is the web's word, not Observe's.

I also adopt your definition: "Declares the application readiness condition for recording this visit's or part's interactive time; Exact also waits for runtime readiness and its own entrance."

Please choose between `markinteractive` and `markinteractivewhen`, or say what in `markinteractive` fails a criterion `markinteractivewhen` meets. End with one line: "Consensus: <name>". Keep it under about 300 words.

## Round 3: Sol

I choose **`markinteractive`**. I withdraw my preference for the `when` suffix.

`mark` supplies the observation signal that `interactivewhen` lacked. A boolean value fits: the expression declares readiness to record an interactive time; literal `true` leaves Exact’s conditions to decide when, while literal `false` leaves the measurement pending until cancellation. These readings give no reason to expect input handling to change.

`when` makes the conditional reading slightly more explicit, but I cannot identify a criterion that `markinteractive` fails and `markinteractivewhen` meets. Both retain *interactive* for discovery, fit every permitted scope, and require the same explanation of Exact’s runtime, entrance and presentation conditions. The suffix does not resolve the possible expectation of an immediate mark.

The User Timing precedent cited in §0 supports the vocabulary independently of Observe. Sharing that vocabulary does not couple Exact’s measurement semantics to Observe’s API.

With the substantive criteria satisfied by both names, saving four characters is a sound reason to choose the shorter one. I accept your definition.

Consensus: markinteractive

