# LLP 1057: Keyframe animation — CSS `@keyframes` and `animation`

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Contract (syntax, lowering), Kernel (style row bit 102, wire), Motion (`exact-motion`), Web, Apple, Linux, Agent API
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1002 (motion v1: one representation, two executors — this extends it), LLP 1003 (motion as built), LLP 1012 (agent clock), LLP 1048 (projected documents), `rules/NOT-DOING.md` §Motion

## Summary

A voice journal (grnl) ported onto exact2 lost two animations because exact2
had only `transition`: a checklist step's mark that breathes while the step
runs, and a Reader entry that fades in when it first appears. Both are CSS
keyframe animations. This adds them the way LLP 1002 added transitions: CSS's
model, one kernel row, the browser as executor on the web and `exact-motion`
everywhere else, held to the browser by recorded samples.

```
keyframes breathe
  from
    opacity=0.4
    scale=0.9
  50%
    opacity=1
  to
    opacity=0.4

text "Transcribing" animation=(running ? "breathe 1.6s ease-in-out infinite" : "none")
```

## Motivation

`transition` animates between two targets when a target changes. Neither
consumer changes a target: a mark pulses for as long as a state holds, and an
entry animates once because it exists. CSS says both with `animation`, and
the web is the standard. The alternative, a timer toggling a target under a
transition, is a per-frame dependency on app logic, is not seekable the same
way, and is not what a browser does.

## Design

**D1 — Contract syntax follows CSS, indented.** A top-level
`keyframes Name` declaration holds keyframe blocks: a selector line (`from`,
`to`, `N%`, or several with commas) over lines of `attr=literal`, exactly a
`style` body. `animation-timing-function` in a block is that interval's easing,
as in CSS. The `animation` attribute takes CSS's shorthand: name, duration,
easing (the transition set: keywords, `cubic-bezier()`, `steps()`, `linear()`),
delay, iteration count (`infinite`), direction, fill mode, play state, in any
order, comma lists allowed, with CSS's rule that a word fits the first longhand
still unset (the first `none` is the fill mode). `keyframes` is not a reserved
word, and it merges across `use` like `style`. The formatter keeps `50%` whole.
*Rejected:* CSS text in a string (`keyframes="@keyframes …"`) — a second
syntax inside the language, unformattable and unchecked; braces — no other
declaration has them.

**D2 — Keyframes animate the four compositor rows.** `translate`, `scale`,
`rotate`, `opacity`; each value goes through the kernel's own row parser
(`StyleProps::set_dynamic`), so `translate="4px 8px"` means in a keyframe what
it means on a node. Anything else is refused by name with the reason. Numeric
`height` stays a transition-only trial (LLP 1002 D7). *Rejected:* colours and
layout properties — natively they need a second interpolation vocabulary and
a layout per frame, which would break parity with the web for these consumers'
sake alone.

**D3 — Names resolve at compile time.** Lowering rewrites every literal an
`animation` expression can produce — a string, or either side of a condition,
match or `let`, or a class choice's unset side — into the row's self-contained
text: the shorthand, then each `@keyframes` rule it names in CSS syntax
(`breathe 1.6s ease-in-out 0s infinite normal none running @keyframes
breathe{0%{opacity:0.4;scale:0.9;}…}`). The kernel parses that text
(`Animations::parse`) into the row. An undeclared name is a compile error that
lists the declared ones; a computed value (`animation=\`${name} 1s\``) is
refused, since which keyframes play is known when the app compiles.
*Rejected:* a keyframes table in the plan resolved by name at runtime — a new
plan table, runner lookup, and host lookup, and a name that can be wrong after
compilation.

**D4 — One row, one engine.** Style row bit 102, `animation`, codec
`animations`: bytes whose grammar is `schema.json`'s `_animations`, its easing
encoding shared with `_transitions` (a spring refused), its Rust type
`exact_motion::Animations`, validated on decode and on structured apply. Bits
100 and 101 are held by placeholder rows for `text-transform` and
`background-image`, landing in parallel. `MotionSync` carries each created or
touched node's row after its targets. `Engine::set_animations` is CSS
Animations §3:

- an animation starts when its row first names it (node creation included);
- an animation still named under the same keyframes keeps its start time, and
  new duration, delay, count or direction apply as if it always had them;
  `paused` holds local time and `running` resumes from it;
- an unchanged row changes nothing, so re-rendering never restarts one;
- one no longer named stops, and the row's own value shows again.

The timing model is Web Animations §4 (phases, fill, fractional counts,
directions, negative delay, zero duration). The easing applies per keyframe
interval; a property missing from `0%`/`100%` interpolates from and to its
underlying value; later animations in the list replace earlier ones for a
property they set. The animated value is composited over the transition-level
value, so an animation wins while it applies and a transition keeps running
underneath (CSS's cascade). Apple and Linux run this through the engine they
already run transitions through; no Swift or presenter code changed. Identity
is the keyframes' content, not the name alone: on the web a rule's name
derives from its content (D5), so both executors restart on the same edits.
*Rejected:* a Core Animation executor (`CAKeyframeAnimation`) — `NOT-DOING`
keeps it unbuilt pending measurement, and it would need its own paths for
seeking under the agent clock, view recycling, and not restarting on updates,
which the engine already has.

**D5 — The web is CSS.** The host emits the `animation` declaration in the
node's `cssText` and each distinct keyframes list as one real `@keyframes`
rule named `<name>-<FNV-1a of the rule>`, sent once by a `keyframes` batch op
into a sheet the glue owns (and kept across a restart, deduplicated by name),
before the first declaration naming it. A projected document (LLP 1048)
carries the same rules in its head. The browser executes; the web's engine,
which lowers springs and holds, never hears animations.

**D6 — The clock seeks animations like transitions.** Every value is closed
form in local time, so `advance(t)` is a seek. `settle_time` includes each
finite animation's end; an `infinite` one never ends and is not waited for, on
native (`settle_time`) or on the web (`clock settle` skips a non-finite
`endTime`). `quiescent` is false while any animation's value still moves, so a
native host keeps frames coming for an endless one.

**D7 — Reduced motion is unchanged.** Motion has no engine policy (LLP 1002
§4, `NOT-DOING`); no host yet reports the preference for transitions either.
Animations inherit the same state: an author chooses `none`.

**D8 — Held to the browser.** Eight keyframe cases join the parity harness
(`host/web/src/parity.rs`): per-interval easing, a keyframe's own `steps()`
and `cubic-bezier()`, implicit keyframes, delay with fill both, alternate,
alternate-reverse with a fractional count, reverse with a negative delay,
`linear()`, zero duration. Chrome 153 recorded 38 samples; the engine matches
all 143 in the fixture within 1e-3.

## Not done

- `animation-composition`, `animation-timeline`, `animation-range`, and
  per-keyframe values for anything but the four rows.
- `steps()`'s before flag, as for transitions (LLP 1002).
- At most 8 animations per node and 32 keyframes per rule; times and values
  ride the wire as f32 (LLP 1002 §5).
