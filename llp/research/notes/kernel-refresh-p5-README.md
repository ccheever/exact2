RFC 0491 Phase 5 exit pack — the four registered pass/fail exit checks named in the RFC's phase table, beside the author-ratified freeze statement (a separate conjunct, and not a check). Registered in `exact-verify.json` under the `kernel-refresh-p5` profile. LLP 0506 D1(e).

# RFC 0491 Phase 5 exit conjuncts

RFC 0491's phase table makes program exit conditional on **five** things: the
freeze statement ratified by the author, plus four registered pass/fail exit
checks green. This directory is the evidence home for the four checks. The
freeze statement is the author's to make; its derived draft, with the author's
2026-08-26 classifications folded in, lives at
`llp/0491.002-oq7-abi-freeze-statement.decision.md`.

**State below measured at `origin/main` `adf44906ca2f86814169c658f8926408380649b2`
(2026-08-26). This table is a snapshot; re-read it with
`bun scripts/exact-verify.mjs --profile kernel-refresh-p5` before relying on it.**

| # | Conjunct | Registered check | Status |
|---|----------|------------------|--------|
| — | Freeze statement ratified by the author | *(not a check — author-ratified by definition)* | **NOT RATIFIED, but no longer a questionnaire** — `llp/0491.002` §0.5 now carries the author's four-surface classification (nothing frozen; C ABI + rlib negotiated under the extended 2026-07-11 fluidity directive; EXWF and EXNODE not freezable). One sentence from ratified. |
| 1 | Forced-full-relayout result-equality differential on the 0486/0487 corpora | `kernel-p5-relayout-result-equality` | **green** — re-run end-to-end 2026-08-26, 7 pinned-1K + 43 corpus cases; named 0486 remainder |
| 2 | The dirty-set instrument existing and measured | `kernel-p5-dirty-set-instrument` | **green** — re-run end-to-end 2026-08-26, 7 bindings, pin reproduced |
| 3 | W0-A precommitted regression limits on every axis, positive targets on every non-maintenance axis | `kernel-p5-precommitment-axes` | **red on 2 timing axes — WAIVED** (`llp/0491.004` §2). **Every positive target is now MET**; see below |
| 4 | One integrated wake → layout → receipt → presentation device measurement | `kernel-p5-integrated-device-measurement` | **red — WAIVED** (`llp/0491.004` §3), omitted promise named. Owed INSTRUMENTATION, not a hardware booking (see below) |

Run them all with `bun scripts/exact-verify.mjs --profile kernel-refresh-p5`.
**The profile is red today, on purpose, and it stays red under the waivers.**
A waiver that turned its check green would convert visible debt into silent
absence, which is exactly what LLP 0506 D1(e) forbids ("a waived row is visible
debt, never silent absence"). What the waivers change is whether D1(e) may close
over these reds — not whether the reds exist.

**The freeze statement is an INDEPENDENT conjunct.** It is not gated on the four
checks and the four checks are not gated on it — LLP 0506 D1(e) names it "a
conjunct, not subsumed". The four surfaces were enumerated by measurement with
file:line citations, every classification the tree already settled was read out
and cited, and every genuine choice was left as an explicit open question rather
than silently resolved. **The author answered those questions on 2026-08-26**
(`llp/0491.002` §0.5), so the document is now a statement rather than a
questionnaire; ratifying it is one sentence. **With (3) and (4) waived, RFC
0491's contribution to LLP 0506 D1(e) turns on this conjunct alone.**

## Conjunct 1 — result equality

`kernel-p5-relayout-result-equality` asserts that **incremental relayout is
result-equal to full relayout**, comparing raw `f32` bits so a one-ULP drift is
a failure rather than a rounding opinion. Two reference arms per case, because
they fail differently:

* **forced-full** — every live Taffy node is dirtied (`Kernel::force_full_relayout`)
  and layout recomputed. Catches a subtree the incremental pass declined to
  revisit.
* **fresh-tree** — a second kernel built from scratch out of the same post-patch
  state. Catches a mutation that never reached Taffy at all, which a forced pass
  would recompute identically wrong.

Executable arms: the pinned 1K fixture (7 named patch cases, including RFC 0491's
two named ones — reorder and keyboard avoidance) and the **LLP 0487 corpus**
(20 fixtures × 3 shape-agnostic patches, 43 applicable cases), realized through
the corpus harness's own `TemplateTree`/`realize_template` seam so corpus drift
reaches this gate and the Phase 1 gate together rather than one of them.

**Named remainder, deliberately not built.** The RFC says "the 0486/0487
corpora". The **0486 arm is UNMET**: LLP 0486's conformance fixtures are realized
by `layout-profile-parity` through the TypeScript lowering, not through the
kernel, and `tests/layout/layout-profile.json` records its own
`bandArtifact.status: "not-built"`. Making that arm real needs a kernel-executable
0486 corpus, which does not exist today. It is recorded here rather than quietly
counted as covered.

## Conjunct 2 — the dirty-set instrument

This is also **RFC 0492 M-D's declared prerequisite**: "this workstream does not
start until RFC 0491 WS-H's dirty-set instrument exists". It now exists.

The instrument is `kernel/src/dirty_set.rs` plus three `Kernel` methods —
`begin_dirty_set_scope(binding)`, `mark_patch_applied()`,
`end_dirty_set_scope()`. One scope yields one `DirtySetObservation` carrying all
three components the RFC names:

1. **per-binding affected-set sizes** — `k` (the scoped changed-geometry receipt
   size) beside the live `n`, attributed to a named binding;
2. **changed-geometry receipts** — the absolute-frame receipt the kernel already
   publishes from `publish_layout_changes`, scoped to this patch;
3. **patch-application → receipt-publication endpoints** — scope open → patch
   applied → receipt published, the middle span being the RFC's named one.

The patch node set is recovered from the kernel's *own* pending-mutation delta,
so a caller cannot mis-report what it touched. A kernel with no open scope pays
one `Option` test per layout pass and allocates nothing.

**It gates no threshold.** The ≤1 ms / ≤64-node budget is RFC 0492's claim,
adjudicated there (RFC 0491 §7.4 r15); which gestures may claim it is 0492's
ruling. This check gates structure, the negative control, and the committed
affected-set pin — never a number.

### First measured values

Pinned 1K fixture, `n = 1000`, release build, arm64 macOS, 2026-08-26:

| binding | patch nodes | affected set `k` | `k/n` |
|---|---|---|---|
| `single-leaf-height` | 1 | 5 | 0.5% |
| `single-leaf-width` | 1 | 3 | 0.3% |
| `container-size` | 1 | 4 | 0.4% |
| `keyboard-avoidance` | 1 | 1 | 0.1% |
| `root-resize` | 1 | 741 | 74.1% |
| `reorder` | 5 | 999 | 99.9% |
| `paint-only` (negative control) | 1 | 0 | 0.0% |

RFC 0491 wrote that "reorder and keyboard avoidance are not obviously `k` ≪ `n`".
On this fixture the instrument says: **reorder is not a `k` ≪ `n` case at all**
(999 of 1000), while keyboard avoidance is the sparsest case measured. That is a
0492 input, not a 0491 blocker — exactly the division of ownership §7.4 set up.

Timings are recorded and reported, never gated: they are host-dependent. The
deterministic half (patch and affected-set sizes) is pinned in
`dirty-set-expectations.json`. Refresh that pin only for a reviewed, real change:

```sh
bun scripts/check-kernel-p5-dirty-set-instrument.mjs --write-expectations
```

## Conjunct 3 — the signed per-axis limits

`docs/kernel-refresh/w0a/performance-precommitment.json` reads `status: "signed"`
(Charlie Cheever, 2026-08-26, recorded @11992d827), so its **8 regression limits
and 3 positive targets bind**. `kernel-p5-precommitment-axes` reads them and
never re-derives, re-baselines, or relaxes them.

### Captured 2026-08-26 on the pinned box — then RE-captured, post-allocation-work

**Read this first, because the two captures are not the same measurement.** The
first paired Bones capture (@`083507748`) measured its live arm at `861e23586`,
which is an **ancestor of `646d16a24`** — the commit that landed the WS-A
allocation work (vendor/taffy patches 3 and 4 plus the kernel
`validate_certified_root_admission` presence-set early-out). **That capture never
measured the patched code.** The current artifact (@`ab86ed142`) is the first
Bones measurement of the post-patch tree, not a re-run of the same thing on a
quieter box.

Both captures were produced on Bones (`Charlies-Mac-mini-bones-M416.local`) in
one session each: three **alternating** freeze/live rounds, artifact from the
last round by a rule fixed before any number was seen — the same rule both
times, deliberately, so the round choice cannot be shopped between captures. The
freeze arm is the W0-B pinned `kernel/src` (tree `6e962588…`), and in the current
capture `freezePin.measuredSrcMatchesPin` self-certifies **true** (the run was
launched from the repo root, so the `git ls-tree` prefix bug does not fire).
Session quality: A/A noise floor p95 0.097–0.172%, 1-minute load 2.18–2.60
against a 3.5 ceiling bracketed around every individual capture, zero foreign
bench processes before and after, `caffeinate` armed and alive at the end.

**Conjunct 3 is RED on two axes. Four of the original six failures closed by
MEASUREMENT; the two survivors are waived.**

**(a) Every positive target is now MET — the substantive finding, reversed.**
Allocator counts are machine-independent and reproduced identically across all
three rounds:

| axis | signed target | first capture | **current capture** |
|---|---|---|---|
| `allocation-full-relayout-1k-calls` | 822 | 1643 | **634** |
| `allocation-full-relayout-1k-bytes` | 361390 | 722780 | **151884** |
| `layout-full-1k` (positive target) | 289539 ns | 349729 | **289136** |

Unasked, `build-plus-first-layout-1k` also improved: 3235 → 2248 calls,
11154900 → 10594212 bytes.

**Two caveats, kept beside the claims rather than in a footnote.**

* `layout-full-1k` passes its target by **403 ns (0.139%)**, which is *inside*
  the session's own A/A noise floor. The robust statement is the paired
  within-session improvement — **−14.25% / −15.15% / −15.47%** across the three
  rounds — not the margin over the constant.
* The bytes figure is **Bones-measured**, not converted from anything. Allocator
  *calls* are architecture-independent (634 on arm64 macOS and on x86_64 Linux);
  *bytes* are not. The Linux reading of the same tree is 151980, a 96-byte
  difference, and the pre-change arm differed by 176 bytes — **the offset is not
  a constant**, so no Bones byte figure may be predicted from a Linux one.

**(b) Two regression limits exceeded — and this signal is NOT trustworthy. The
paired arm proves it, now in two independent sessions.** The signed timing limits
carry only **0.094%** headroom, derived from an A/A micro-interleave of a single
export operation. Within-session drift of an arm *median* on this box is **2–3%**:

| axis | signed limit | session A freeze r1/r2/r3 | session B freeze r1/r2/r3 |
|---|---|---|---|
| `layout-full-1k` | 340954 | 331458 / 339183 / 338227 | 330455 / 337641 / **342047** |
| `mutation-styles-1k` | 159172 | 155819 / **159907** / **161155** | 157206 / **160381** / **161192** |
| `export-typed-1k` | 53516 | 52611 / **54613** / **54187** | 53419 / **53989** / **53949** |

The **freeze arm is unchanged code** — it is the frozen baseline itself — and it
**exceeds its own signed limits** in rounds 2 and 3 of both sessions, and all
three limits in session B. A limit that the baseline code fails cannot
distinguish a regression from thermal drift. Session A also recorded the paired
within-round delta **flipping sign** across rounds (+2.09%, −0.17%, +3.40%).

**Disposition: WAIVED, not re-derived.** Charlie ruled *"waive for now"* on
2026-08-26; `llp/0491.004-phase5-conjunct-waivers.decision.md` §2 names exactly
what the waiver drops (these three timing regression limits, for exit purposes
only) and what it does not. **Nothing signed was re-derived, relaxed, or
re-baselined**, in either session or in the waiver. The check stays RED so the
debt stays visible.

The durable question — whether the timing axes should be judged on the paired
within-session delta the signed rule already describes, given a trend-only
disposition, or left as signed and re-waived — is open, with options, costs and a
lane recommendation in `llp/0491.004` §2.5. The check currently **requires** a
paired capture, **prints** the paired baseline, and **never judges against it**.

Reproduce with the driver on branch `agent/l-d1e` (and its predecessor on
`agent/p5-hardware-evidence`); both arms must be built `--no-run` and executed
from the **repo root** (see
`issues/20260826-cargo-bench-release-profile-*` and
`issues/20260826-w0a-bench-freezepin-*`).

## Conjunct 4 — the integrated device measurement

> **WAIVED 2026-08-26** (Charlie, relayed via orchestration session `exact-88`;
> recorded in `llp/0491.004-phase5-conjunct-waivers.decision.md` §3). The waiver
> names the omitted promise in these words: **Exact exits RFC 0491 Phase 5
> without ever having measured, end to end on a device, the latency from runtime
> wake to pixels on screen.** The check stays RED. The signed
> `deviceMeasurement.disposition` is **untouched** — it still reads `trend-only`,
> and this waiver does not convert it to an envelope. The waiver is against RFC
> 0491's Phase 5 conjunct list, not against the signed precommitment's numbers.
> The instrumentation below is post-window work and should get a named owner;
> `llp/0491.004` §3.4 carries the waive-vs-build tradeoff.

The signed precommitment records `deviceMeasurement.disposition: "trend-only"`,
and RFC 0491 says this conjunct is judged against that disposition and must
**never** carry an unpredicated pass/fail line. So
`kernel-p5-integrated-device-measurement` gates on *evidence*, not on latency: the
measurement must exist, declare exactly the signed disposition (a capture that
quietly upgrades itself to an envelope is refused — only a signed amendment may
change that), carry no verdict field while trend-only, be taken on the pinned
hardware class on a clean tree, be taken under **representative JS/GC contention**
rather than on an idle device, name its sampling protocol, and carry at least 30
samples whose four stage endpoints are present and correctly ordered. It then
reports wake→presentation p50/p95 as trend.

Expected artifact: `docs/kernel-refresh/p5/integrated-device-measurement.json`.

### It is owed instrumentation, not a hardware booking (surveyed 2026-08-26)

The wording above — and this pack's earlier status line — implied that conjunct
(4) merely needs time on the pinned box. **It does not.** A survey of the tree
found that three of the four stage endpoints do not exist in any readable form
and the fourth is deliberately fenced off:

* **wake** — no runtime-wake timestamp is recorded anywhere.
* **receipt** — the kernel's `DirtySetObservation` timing endpoints have **zero
  FFI exposure**; the host and JS can never see them, and their
  `std::time::Instant` clock has no epoch anchor to join to a host timeline.
* **layout** — host-side timing is millisecond `Double`s scoped to **boot**, not
  per interaction.
* **presentation** — **no present receipt exists on any Apple host.**
  `recordCommitPresent` hardcodes `fidelity: "proxy"` and is taken *after*
  `presenter.apply(snapshot:)` returns, before any CATransaction flush. Two
  validators actively reject a `commitPresent` not labeled `"proxy"`.

There is also **no representative JS/GC contention workload** in the tree — only
a ~75%-duty CPU spin, which is CPU pressure, not allocation/GC churn.

**That danger is now closed (2026-08-26).** The check gated on evidence
*structure*, not fidelity, so a 30-sample artifact assembled from proxy values
would have gone **green** — and since the hosts can currently produce *only*
proxy values, the first good-faith attempt at this artifact would have passed
while measuring nothing the conjunct asks for. The artifact schema now requires
`endpointFidelity.<stage> = { fidelity, source }` for all four stages; only
`"measured"` is admissible, an absent map fails closed, and a fidelity claim with
no named instrument is refused. Proven with a guarded committed baseline, each
arm run against both the pre-change and strengthened checks: a 40-sample
artifact with **no** `endpointFidelity` exited **0** on the old check and **1**
on the new one, as did proxy presentation, derived wake, a missing `source`, and
an out-of-vocabulary value — while the all-`measured` arm still goes green.

Full gap analysis, true build cost, and recommended disposition:
`issues/20260826-conjunct4-integrated-device-measurement-has-no-instrumentation.md`.

## Why these gates are believable

Per RFC 0496's gates-prove-themselves rule, none of the four landed without
being watched to go red. Runs on 2026-08-26, before landing:

| Planted defect / arm | Check | Result |
|---|---|---|
| `set_style` skips Taffy on a height-only change (a missed invalidation) | conjunct 1 | RED, 3 cases, via the fresh-tree arm |
| `force_full_relayout` dirties nothing | conjunct 1 | RED — "the reference arm is vacuous" |
| `publish_layout_changes` publishes every node as changed geometry | conjunct 2 | RED — the paint-only negative control |
| the receipt-publication endpoint is never taken | conjunct 2 | RED — "closed before any layout pass published receipts" |
| synthetic capture: current == baseline | conjunct 3 | RED — 3 positive-target misses |
| synthetic capture: +5000 ns export regression | conjunct 3 | RED — past its signed limit |
| synthetic capture: taken off the pinned box | conjunct 3 | RED |
| synthetic capture: meets every positive target | conjunct 3 | **GREEN** — so it is not an unconditional failure |
| synthetic artifact: self-declared envelope with a verdict | conjunct 4 | RED |
| synthetic artifact: `contention.representative: false` | conjunct 4 | RED |
| synthetic artifact: receipt precedes layout | conjunct 4 | RED |
| synthetic artifact: 5 samples | conjunct 4 | RED |
| synthetic artifact: conforming 40-sample trend-only capture | conjunct 4 | **GREEN** |

## Where they run

Conjuncts 1 and 2 are also members of the **`governance`** profile, which runs on
every push and PR and is input-gated — so a change under `kernel/src/**`,
`packages/exact-kernel-phase5-gates/**`, the LLP 0487 corpus, or the pinned
fixture re-runs them on main, in CI. Conjuncts 3 and 4 are `kernel-refresh-p5`
only: they are honestly red pending hardware, and parking a permanent red in a
per-push gate teaches people to re-run reds instead of believing them.

## Files

* `dirty-set-expectations.json` — the committed per-binding affected-set pin
  (deterministic; timings are excluded on purpose).
* `performance-exit.capture.json` — **re-captured 2026-08-26 on Bones after the
  allocation work landed**, conjunct 3 (red on 2 waived timing axes; every
  positive target met — see above). The superseded capture (@`083507748`)
  measured a pre-patch tree.
* `integrated-device-measurement.json` — **owed**, conjunct 4.
* `*.report.json` — regenerated by every run and gitignored (host-dependent
  timings).
