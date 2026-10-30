# Code review: the web navigation split's candidate search, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `6724658dcb`.
- **Method:** one brief (sha256 `62e09e4190f07cccec264f4aaaa177c5ecd482d28d8bc24ed5f1de02e49730e4`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none.

The loop at [navigation.js:59](host/web/navigation.js:59) is correct: `!up` continues the outer loop before dereferencing null; reaching `route` excludes its own attributes from admission checks; a direct child needs zero ancestor iterations. This matches the old walk for an unchanged DOM. The narrowed guarantee at [navigation.js:52](host/web/navigation.js:52) is sound, and the detached-direct-child difference is acceptable outside that guarantee.

**Non-blocking**

1. **The counters still don’t measure executed search decisions.** [split-search.test.mjs:107](host/web/tests/split-search.test.mjs:107) classifies every candidate, independently of the search invoked afterward at line 160. It never stops when a pushing list answers. Thus later candidates can satisfy every coverage counter without the new search examining them. This round-1 finding is only partially resolved. Count actual executed decisions, or restrict classification to the visited prefix and describe it as predicted coverage.

2. **Both popover cases can pass without popover exclusion.** The fixtures at [split-search.test.mjs:251](host/web/tests/split-search.test.mjs:251) and [line 256](host/web/tests/split-search.test.mjs:256) create closed popovers. Their computed `display:none` independently excludes them, so deleting the popover predicate still yields `B`. Open each popover before comparison and assert that its computed display is not `none`. The other 17 fixed cases exercise their intended decisions and assert concrete answers.

3. **The removal-test comment remains too broad.** [split-search.test.mjs:287](host/web/tests/split-search.test.mjs:287) says a removed list is “never answered” and the walk cannot reach it. Those statements hold for this fixture, not arbitrary mutation: the old stack can retain detached nodes, and a matcher can remove the current candidate after its ancestry check but before [navigation.js:62](host/web/navigation.js:62) returns it. Also, skipping a removed direct child can return a later live list—not exclusively null. Narrow the comment to removal **before the candidate is examined** and this fixture’s ancestor arrangement; broader mutation support is unnecessary for the stated guarantee.

4. **The first colour-scheme assertion assumes light mode.** [split-search.test.mjs:204](host/web/tests/split-search.test.mjs:204) expects `.flip` to remain vertical before any media override. A browser initially reporting dark mode makes that assertion fail. Explicitly emulate light before this step.

The other round-1 fixes are resolved: the [per-route budget](host/web/tests/split-search.test.mjs:81) and [zero-empty assertion](host/web/tests/split-search.test.mjs:185), [push/pop wording](host/web/tests/split-search.test.mjs:213), [iframe existence check](host/web/tests/split-search.test.mjs:234), and [memo wording](host/web/navigation.js:51). The removal regression also rebuilds the fixture for the independent old-walk comparison, avoiding comparison against an already-mutated tree.

Static review only; no edits, network, tests, or builds.
