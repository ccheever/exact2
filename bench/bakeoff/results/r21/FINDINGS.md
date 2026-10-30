# Round r21: what it found

The first round of the rebuilt bake-off (`summary.md`), on exact2 `f0eeade6b1`: twelve iPhone specs, each built
with Exact and with SwiftUI or UIKit by the same model (GPT-6.1 Sol through Codex, or Claude Opus 5.5), graded
blind by Claude, and every app's list probed with real touches. Its numbers are not comparable with round 20's:
the specs, the harness and one builder model are new.

Exact lost, 2 to 10: overall 6.8 against 8.2, platform 6.8 against 7.3, 6.2 tells an app against 4.8, 1.48× the
minutes and 2.69× the tokens. Two things in the harness inflate Exact's side and are fixed for r22:

- **The assigned bundle ids had a part starting with a digit.** Exact refuses that (the one id is also the Android
  package and the Linux app id) with only the regex as the reason. All twelve Exact builders hit it, and most
  worked around it at length: preload plugins over the schema, re-stamped and re-signed bundles, and in contacts a
  patched private copy of the Swift host. r22's ids start with a letter, and the refusal now says why and how to
  fix it (`79afe8b4f1`).
- **Nine new simulators went through first boot** during the first builds (load average 860 on 18 cores), which
  costs a compile-bound build more than a `swiftc` one.

## Scrolling (the probe, `host/apple/feel`)

Exact's lists match UIKit's physics exactly: deceleration, finger tracking, rubber band, scroll to top, a tap
that stops a fling. But in six of eleven apps with a long list they hitch where the native app does not:

| App | Exact hitch ratio (ms/s) | Native | Exact offset jumps | Native |
|---|---|---|---|---|
| watchlist (live prices) | 79.7 | 0 (UIKit) | many | 0 |
| moodboard (600-tile grid) | 73.3 | 0 (SwiftUI) | 20 | 0 |
| podcasts | 26.6 | 10.2 (SwiftUI) | 23 | 0 |
| contacts (sections, index) | 26.1 | 1.2 (SwiftUI) | 18 | 0 |
| inbox | 17.4 | 0 (UIKit) | 8 | 0 |
| expenses | 9.1 | 0 (UIKit) | 23 | 0 |

Under 5 ms/s is Apple's "good". The grader called every list smooth: it judges from screenshots and cannot see
this, which is why the probe exists. On the calibration list a fast fling's release also costs the main thread
17–37 ms (UIKit ≤ 4). Both are being fixed. The probe's blank-row detector reports false positives on native
grouped lists (settings, inbox, moodboard), so blank samples are a note, not a finding, until it is fixed.

## Tells with one cause in Exact

| Tell | Exact apps |
|---|---|
| Rows are exposed to accessibility as Links, with stray nested scroll views | 6 |
| Disclosure chevrons drawn bold and black, not UIKit's tertiary indicator | 5 |
| Pushed or inline screens have an opaque bar with a hairline, not the scroll-edge effect | 5 |
| A swiped row turns into a grey rounded card | 4 |
| The bottom search field stays under the keyboard | 2 |
| Swipe actions are untinted grey circles without labels | 1 |

The rest are the builders' (no swipe actions where iOS has them, a hand-built selection toolbar, always-visible
grips, explanatory footers). Some point at gaps in what Exact offers: reordering inside a grouped list, a
collapsible section, a picker row that pushes a checkmark list, a section index with a non-letter section.

## Builders' notes (`notes/`)

Beyond the bundle id: the section index works only when every header is one character; plain-list sticky headers
are transparent at large text sizes; a grouped row whose only child is a column is read as a standard cell and
loses its authored paint without a warning; reorder needs a virtualized list, which cannot be grouped; the agent's
clock starts at a fixed epoch, so builders sorting by time saw new items sort first; Hermes' `Intl` puts U+202F
before "AM", so iOS and web tests disagree.

## Tokens

Exact builds took 41–134 calls against 25–60 for the native ones, and 100–200 KB of tool output, about half of
it docs: `start-here.md` (22 KB, read whole), `contract vocab`/`docs` lookups (24–48 KB) and patterns. Claude
building SwiftUI was the cheapest arm in the round (settings: 0.9 Mtok, 4.4 minutes).
