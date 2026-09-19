# LLP 1043: Pretext, measured — a userland line breaker against the hosts' own text engines

**Type:** Research
**Status:** Draft
**Systems:** Text (the `TextMeasurer` seam of LLP 1001 §6 — what it asks a host and how often), Apple host (`TextEngine`: the typesetter cache, the break loop, the paragraph snapshot), Linux host (cosmic-text, one `Buffer` per paragraph-width), Web host (the browser lays out; the measured-height List reads row heights from the DOM), Kernel (whether a line breaker belongs below the seam), Scrolling (LLP 1010 §6 — heights of rows that are not mounted)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-18
**Related:** LLP 1001 §6 (text measurement is a per-kernel injected trait object; one structural run IR), LLP 1008 §3 (CoreText, one engine for measuring and painting — "measuring with one engine while painting with another is a correctness tax"), LLP 1015 §3 (the same lesson on Linux, in cosmic-text), LLP 1010 §6 (bounded list memory; measured-height windows), LLP 1019 (declared fonts — the identity a width cache is keyed by), LLP 1033 (the Markdown viewer — the app with the most paragraphs), LLP 1035.000.000 (CSS `line-height`), `CLAUDE.md` (the web is the standard; verify by running), `rules/NOT-DOING.md`, `QUEUE.md` ("Markdown performance comparison": exact-offer measurement reuse "halves callback traffic but has only a modest CPU benefit"). External, each read 2026-09-18: `github.com/chenglou/pretext` `README.md` at `main`; npm `@chenglou/pretext` 0.0.9 (MIT).

## Summary

Charlie asked, 2026-09-18: *"could we include cheng lou's pretext thing in
exact2? or does it not fit well? would it make sense to have it be
something built in?"* It was answered in conversation from the code; this
is the write-up he asked for, with the one claim in that answer that
needed a measurement measured — and corrected. It decides nothing and is
not linked into `llp/current/` (the working set is full at fifteen, and a
research record decides nothing).

Pretext is a TypeScript library that lays out multiline text without the
DOM. `prepare(text, font)` segments the text with `Intl.Segmenter`,
measures each segment with canvas `measureText`, and returns a handle;
`layout(prepared, width, lineHeight)` is then arithmetic over cached
widths. It exists because asking a browser how tall a paragraph is costs
a reflow.

| | |
|---|---|
| The library, on the web host | No place to stand: the browser is the layout engine there and no `TextMeasurer` runs (F1) |
| The library, on a native host | Cannot run: its two inputs are canvas `measureText` and `Intl.Segmenter`, and exact2 has no app JavaScript above the data seam (F2) |
| Its split, in exact2 today | Built, structurally, on both native hosts: each keeps one width-independent shaped paragraph (a `CTTypesetter` on Apple, cosmic-text `ShapeLine`s on Linux) and lays it out per width (F3) |
| What re-breaking costs on CoreText | **63–92 µs** per paragraph per width — about two-thirds of what shaping the paragraph cost (97–155 µs). Not arithmetic (F4) |
| What Pretext's arithmetic costs, on the same text | **0.16–0.22 µs** per paragraph per width — about **400×** less (F4) |
| What the arithmetic costs in agreement | A naive breaker over separately measured segments gave a different line count from CoreText in 22 of 10,000 paragraph-widths, always one line more (F5) |
| A first height | Not helped: preparing is the cost either way, and CoreText prepares a whole paragraph faster than the probe measures its segments (F6) |

The answer: **the library does not fit, and should not be built in.** Its
architecture is the interesting part, and the measurement says exact2 has
less of it than the conversation claimed: the width-independent half is
kept on both native hosts, but on Apple the width-dependent half is a
CoreText call per line, four hundred times the cost of adding numbers. That gap only
matters where one paragraph is asked at many widths — a live window
resize over a long document, `text-wrap: balance`, a shrink-wrapped
bubble, text flowed around a shape — and exact2 has none of the last
three as rows and windows the first (LLP 1010 §6). §4 lays out the three
ways to take the idea and what each costs; §5 says which, and what would
reopen the rest. One thing found on the way is cheap and independent of
all of it (F7).

## 1. What Pretext is

From its README at `main` (npm `@chenglou/pretext` 0.0.9, MIT):

- `prepare(text, font, options?)` — "normalize whitespace, segment the
  text, apply glue rules, measure the segments with canvas, and return an
  opaque handle." `layout(prepared, maxWidth, lineHeight)` — "pure
  arithmetic over cached widths", returning `{ height, lineCount }`.
- A second tier for callers who place lines themselves:
  `prepareWithSegments`, `layoutWithLines`, `walkLineRanges`,
  `measureLineStats` (line count and widest line, no allocation),
  `measureNaturalWidth`, and `layoutNextLine` / `layoutNextLineRange`,
  which take a cursor and a width **per line** — the API that flows text
  around a shape. A rich-inline tier (`prepareRichInline`) handles a flat
  list of styled items under `white-space: normal` only; "this is not a
  general CSS inline formatting engine."
- It targets one CSS configuration: `white-space: normal` or `pre-wrap`,
  `word-break: normal` or `keep-all`, `overflow-wrap: break-word`,
  `line-break: auto`, numeric `letter-spacing`. Soft hyphens are honored;
  it inserts none. exact2's default is CSS's `overflow-wrap: normal` — a
  long word overflows rather than breaks — which is not in that list.
- It requires `Intl.Segmenter`, Canvas 2D text measurement, and Unicode
  property escapes. Server-side is "soon". `system-ui` is "unsafe for
  `layout()` accuracy on macOS"; a generic family or a missing glyph may
  resolve to a font other than the one measured; `<html lang>` changes
  breaks. It keeps a ledger of per-browser bugs.

Its stated uses: virtualization without estimated heights, masonry,
shrink-wrapped and balanced text (binary-search a width with
`walkLineRanges`, then lay out once), text around shapes, no layout
shift.

The asset is not the arithmetic, which is a few dozen lines. It is a line
breaker tested against browsers until it agrees with them across
scripts, and the ledger of where they disagree with each other.

## 2. Where exact2's text stands

*Provenance.* First read from the local `main` checkout (42d20ca4 with
other sessions' uncommitted work), which was 72 commits behind
`origin/main`. Re-read at `origin/main` 7e77aaf1 the same day, when a
lane was cut for LLP 1043.000, and corrected below: the first draft said
the Linux host reshapes at every width (it did, in that checkout; on the
trunk it does not) and named caches the trunk has since replaced.

**The seam** (`kernel/src/text.rs`, LLP 1001 §6). A host injects one
`TextMeasurer` per kernel: `measure(&TextMeasureRequest) -> TextMetrics`.
The request is the paragraph's ordered runs, its paragraph style
(direction, `text-align`, `line-clamp`, `text-overflow`, `overflow-wrap`,
the strut) and an offer on each axis; the answer is width, height, first
baseline. One question, one width. Taffy asks it several times per
layout (min-content, max-content, the definite offer). The kernel also
issues a `ParagraphStamp` — proof of the paragraph's current inputs,
"not its geometry" — and asks through `measure_identified(stamp,
request)`, so a host can key retained work on identity without hashing
the text; "offers and host catalog remain separate keys".

**The web host.** "CSS is computed here, in Rust, from the kernel's style
rows, so the browser is the layout engine" (`host/web/src/lib.rs`): no
measurer runs. The one place the web host asks how tall text came
out is the measured-height List, which reads
`getBoundingClientRect().height` from each mounted row
(`host/web/glue.js`) — a row being a subtree, not a text block.

**The Apple host** (`host/apple/Sources/ExactKit/Text.swift`). CoreText
measures and paints. A `Paragraph` is a width-specific snapshot — the
wrapped `CTLine`s, baselines, size — and it is the object `draw` paints:
what was measured is what is painted, by construction. Beneath it a
`TextShape` holds the attributed string's `CTTypesetter`, the
width-independent half, kept with the paragraphs in a byte-budgeted
`TextResidency` (`TextResidency.swift`); `layout(shape, width:, breaks:)`
walks it with `CTTypesetterSuggestLineBreak` and
`CTTypesetterCreateLine`. A colored
spec takes its breaks from its geometry-only twin (`breaks:`), so the
host already paints lines from break positions it was handed rather than
ones CoreText chose at paint time. Under `overflow-wrap: normal` the loop
also computes the paragraph's Unicode line-break boundaries with
`CFStringTokenizer`, to tell an emergency break from an opportunity.

**The Linux host** (`host/linux/src/text.rs`, `text/shaping.rs`).
cosmic-text measures and paints. A `ShapedSource` is "one immutable full
shape; each width owns independent layout and ink": the paragraph's
`ShapeLine`s are kept, and `layout(width, wrap)` runs
`ShapeLine::layout_to_buffer` per width — cosmic-text's breaker over
shaped glyphs, one width per call.

## 3. Findings

**F1 — On the web, Pretext would measure what the browser already lays
out.** Pretext's premise is a program that needs a text height and has
only the DOM to ask. exact2's web host never needs one for layout: the
kernel's tree is the DOM's, and the browser places it. The List's row
heights are the nearest thing, and a row's height is its subtree's —
padding, images, nested flex — which Pretext does not compute. Using it
there would mean running a second layout engine on the web host to
predict the first, against `CLAUDE.md`'s reason for having the browser
lay out at all.

**F2 — On a native host it has nothing to measure with.** Both inputs
are browser APIs. JavaScript in exact2 is the data seam's (LLP 1027): a
lean Hermes VM with no canvas, below the tree, off the layout path.
Supplying Pretext a `measureText` backed by CoreText would put a
JavaScript call inside every text measurement and a second breaker
beside the painter's — the tax LLP 1008 §3 names.

**F3 — The split exists on both native hosts.** See §2. The
conversation's claim — "the main idea behind it is already in exact2" —
is true of the structure and, on CoreText, false of the cost, which is
F4. The Linux host's per-width cost was not timed.

**F4 — Re-breaking a shaped paragraph on CoreText is not cheap;
arithmetic is about 400× cheaper.** The probe (§7) builds 1,000
paragraphs of 60–119 words (mean 914 UTF-16 units; English from
`/usr/share/dict/words` with Arabic, Hebrew, CJK, Hangul, emoji, URLs
and punctuation mixed in; Helvetica Neue 16) and breaks each at ten
widths from 240 to 800 — 185,995 lines, 18.6 per paragraph-width. Best
of five per phase; ranges are over six runs on an M5 Max (macOS 26.6.2,
Swift 6.4, `-O`) that was also running other builds, so the orders of
magnitude are the finding and the second digit is not.

| Phase | Cost |
|---|---|
| CoreText prepare: `CTTypesetterCreateWithAttributedString` and a first break | 97–155 µs per paragraph |
| CoreText re-break: `CTTypesetterSuggestLineBreak` to the end | 63–92 µs per paragraph-width (3.4–5.0 µs per line) |
| CoreText snapshot: breaks, `CTTypesetterCreateLine`, typographic bounds — what the host's `Paragraph` builds | 79–120 µs per paragraph-width |
| Unicode line-break boundaries, `CFStringTokenizer` | 25–31 µs per paragraph |
| Pretext's shape — prepare: tokenize, measure each distinct segment as its own `CTLine`, cache by string | 190–324 µs per paragraph cold, 134–192 µs warm (19,767 distinct segments) |
| Pretext's shape — layout: add cached widths, break greedily, hang trailing whitespace | **0.16–0.22 µs per paragraph-width** |

Within each run re-break is 335–440× the arithmetic. The typesetter
cache saves a little over half of a resize's text cost, not nearly all
of it. That agrees with `QUEUE.md`'s note that exact-offer reuse "has
only a modest CPU benefit": the expensive part was never the repeated
offer, it is each new one.

**F5 — The arithmetic disagrees with the engine unless the engine paints
the arithmetic's lines.** The naive breaker and
`CTTypesetterSuggestLineBreak` gave different line counts in 22 of
10,000 paragraph-widths (0.22%), each by one line, and every time the
arithmetic used the extra line (186,017 lines against 185,995): segments
measured apart sum slightly wider than the same text shaped together. A
27-word vocabulary gave 1 in 10,000. Where the measurer counts one line
more than the painter draws, a paragraph has a blank last line; one
fewer, a clipped one. Inside one host this is solvable by construction —
the painter draws the breaker's line ranges, which `layout(_, breaks:)`
already does for colored specs — and then the residue is a line whose
shaped width exceeds the offer by a kerning delta, not a missing line.
What is not solvable by construction is agreement with the browser,
which breaks its own lines on the web host whatever exact2 computes.
That agreement is what Pretext's test corpus is *for*, and it is the
part with value to exact2 (§4, option C).

**F6 — A first height is prepare-bound either way.** Virtualization is
Pretext's headline use, and LLP 1010 §6's measured-height List is
exact2's version of the problem: rows that are not mounted have
estimated heights. Arithmetic does not help the first answer. From F4's
unit costs, 10,000 unmounted paragraphs of this size are roughly 1.0–1.6
s of CoreText prepare plus 0.6–0.9 s of first breaks, and the
segment-measuring prepare is no faster (the probe's is slower, and
naive). What arithmetic changes is every *later* width: about 2 ms for
all 10,000 against 0.6–0.9 s. This is arithmetic from measured unit
costs, not a measured list; and a row is a subtree, so its height is a
Taffy layout in which text is one leaf.

**F7 — The Apple host recomputes a width-independent pass at every
width.** Under `overflow-wrap: normal` — the CSS default, so nearly
every paragraph — `layout(shape, width:, breaks:)` runs
`CFStringTokenizer` over the whole paragraph on each call (still so at
`origin/main` 7e77aaf1): 25–31 µs here, a quarter to a third on top of a
79–120 µs snapshot. The boundaries depend on the text alone and could
sit beside the typesetter in `TextShape`. Read from the
source and costed by the probe, not profiled in the app. Queued.

## 4. Three ways to take the idea

**A. The library as a dependency.** F1 and F2: no host can use it.
Nothing to build.

**B. A prepared handle on the seam, answered by each host's engine.**
`TextMeasurer` grows a second shape of question — prepare this
paragraph, then answer heights at several widths, or lines one at a time
with a width per line — and each host answers with the engine it paints
with. It keeps LLP 1008 §3 whole, and it is the right shape for
`text-wrap: balance`, shrink-wrap, and per-line widths. But F4 prices
it: on CoreText each extra width is 63–92 µs for a long paragraph, so a
balance search of eight widths is 0.5–0.7 ms per paragraph before the
snapshot, and on Linux each is a reshape until that host
gains a width-independent half. It makes those features *expressible*,
not cheap. No row in `kernel/tables/schema.json` needs it today: there
is no `text-wrap`, no float, no `shape-outside`.

**C. One Rust line breaker shared by the native hosts.** Both native
hosts have a Rust half (`host/apple/src/measure.rs` sits between the
kernel and Swift; the Linux host is Rust). A crate beside them — not in
`exact-kernel`, which "never embeds a platform text API" and carries the
size budget — would ask the engine once per paragraph for segment
advances, break by arithmetic at any width, and hand the painter line
ranges to draw. It buys F4's 400× on every width after the first,
identical breaks on Apple and Linux for identical advances, and the
features of B at arithmetic prices. It costs a CSS line breaker owned
here: UAX #14 with CSS's tailoring, white-space collapsing, hanging
spaces, `overflow-wrap`, `line-clamp` and ellipsis, `letter-spacing`,
bidi reordering per line, soft hyphens, tabs — and F5's standing
obligation to agree with the browser, which exact2 cannot override on
the web host and which is `CLAUDE.md`'s standard. Pretext would be the
reference for that: its glue rules, its corpus, its platform-bug ledger,
ported as tests rather than linked as code. Unmeasured here: the crate's
size (the break-class tables), its prepare cost against CoreText's
whole-paragraph shaping (F6 suggests no better), and agreement on
anything but line *counts* in one font.

## 5. Recommendation, and what would reopen it

Do not add Pretext, and do not build B or C now. No measured hitch and
no product row asks for them, and C is a standing parity obligation
taken on to make fast something exact2 does not yet do.

Do F7's cache when the Markdown comparison next profiles text: it is the
same idea at the size the evidence supports — move a width-independent
pass out of the per-width path — and it needs no new seam.

What would reopen it:

- **A measured resize hitch.** A live window resize over a long document
  misses the (spec, width) cache at every frame for every mounted
  paragraph: at F4's 79–120 µs, fifty long paragraphs are 4–6 ms of an
  8.3 ms ProMotion frame. LLP 1010 §6's windows bound the count. If the
  Markdown viewer's resize traces show text dominating, C is the fix and
  B is not.
- **A row that needs many widths per paragraph.** `text-wrap: balance`
  or `pretty`, a chat bubble that shrink-wraps to its widest line,
  floats or `shape-outside`. The row arrives as CSS names it; B is its
  seam; F4 decides whether C must come with it.
- **Exact heights for unmounted rows** becoming a List requirement (a
  scrollbar that does not re-estimate) *and* the List's width changing
  often enough that the second and later passes are the cost (F6).
- **Line breaks diverging between hosts as a reported bug**, not a
  theoretical one: the same paragraph wrapping differently on macOS and
  Linux, or either against the browser, in an app someone ships.
- **Pretext shipping its server-side mode** with a measurement backend
  that is not canvas. That would make its breaker runnable where exact2
  runs JavaScript — still below the seam and off the layout path, so it
  would change F2's wording and not its conclusion.

## 6. Confidence and limits

High: F1, F2, F3 (read from the hosts' source and Pretext's README); the
order of magnitude in F4; that F5's disagreement exists and is one-sided.

Lower: F4's exact figures (a shared machine; one font, one size, one
corpus of long paragraphs — UI text is mostly one line, where a re-break
is a few microseconds and none of this matters). F5's rate is for line
*counts* with a breaker written in a few dozen lines; Pretext's own rate
against browsers is its authors' to state, and was not measured here —
no browser was run. F6 and the resize figure in §5 are arithmetic from
unit costs. The Linux host was read, not timed. Nothing in the repo was
built or run for this; the probe is CoreText alone, outside the repo.

## 7. The probe

`swiftc -O main.swift -o probe && ./probe`, in the session's scratchpad.
One run's output:

```
paragraphs 1000, mean 914 UTF-16 units, 10 widths, 185995 lines broken by CoreText, 186017 by arithmetic, 19767 distinct segments
CoreText prepare  (typesetter + first break):  96.8 µs per paragraph
CoreText re-break (SuggestLineBreak only):     68.30 µs per paragraph-width
CoreText snapshot (breaks + CTLines + bounds): 78.88 µs per paragraph-width
segments prepare, cold / warm cache:           189.7 / 133.7 µs per paragraph
arithmetic re-break over cached widths:        0.16 µs per paragraph-width
line count differs from CoreText in 22 of 10000 paragraph-widths (0.22%), by at most 1 line(s)
```

(The boundaries line was added for the last three runs: 28.9, 30.7, 25.3
µs per paragraph, 97,998 boundaries.)

```swift
import CoreText
import Foundation
import QuartzCore

let extra = ["春天到了", "بدأت", "الرحلة", "naïve", "café", "🚀", "AV", "To", "“quoted”", "well-known", "e.g.", "3.14", "https://exact.dev/a/b", "日本語のテキスト", "한국어", "עברית"]
let dict = (try! String(contentsOfFile: "/usr/share/dict/words", encoding: .utf8)).split(separator: "\n").map(String.init)
var words: [String] = extra
var seed: UInt64 = 0x9E3779B97F4A7C15
func next() -> Int { seed = seed &* 6364136223846793005 &+ 1442695040888963407; return Int(seed >> 33) }

for _ in 0..<20000 { words.append(dict[next() % dict.count]) }
for _ in 0..<2000 { words.append(extra[next() % extra.count]) }

func paragraph(_ n: Int) -> String { (0..<n).map { _ in words[next() % words.count] }.joined(separator: " ") }

let font = CTFontCreateWithName("Helvetica Neue" as CFString, 16, nil)
let attrs: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): font]
let count = 1000
let texts = (0..<count).map { _ in paragraph(60 + next() % 60) }   // 60–119 words, mean 914 UTF-16 units
let chars = texts.reduce(0) { $0 + $1.utf16.count }
let widths: [Double] = [240, 280, 320, 360, 400, 480, 560, 640, 720, 800]

// Breaks only: what a height-at-width question needs when line height is set.
func lineCount(_ t: CTTypesetter, _ length: Int, _ width: Double) -> Int {
    var start = 0, lines = 0
    while start < length {
        let c = CTTypesetterSuggestLineBreak(t, start, width)
        start += max(c, 1); lines += 1
    }
    return lines
}
// Breaks plus the CTLines and their bounds: what the host's Paragraph snapshot builds.
func snapshot(_ t: CTTypesetter, _ length: Int, _ width: Double) -> Double {
    var start = 0; var h = 0.0
    while start < length {
        let c = max(CTTypesetterSuggestLineBreak(t, start, width), 1)
        let line = CTTypesetterCreateLine(t, CFRangeMake(start, c))
        var a: CGFloat = 0, d: CGFloat = 0, l: CGFloat = 0
        _ = CTLineGetTypographicBounds(line, &a, &d, &l)
        h += Double(a + d + l); start += c
    }
    return h
}

// Warm the font and CoreText's own caches so the first paragraph does not pay for them.
for t in texts.prefix(20) {
    let ts = CTTypesetterCreateWithAttributedString(NSAttributedString(string: t, attributes: attrs))
    _ = snapshot(ts, t.utf16.count, 320)
}

// The Pretext analogue: segment at Unicode line-break opportunities, measure each
// distinct segment once (its own CTLine, as Pretext measures each with canvas
// measureText), then break greedily by adding cached widths. Trailing whitespace hangs.
struct Seg { var width: Double; var trailing: Double }
var segCache: [String: Seg] = [:]
func segments(_ text: String) -> [Seg] {
    let ns = text as NSString
    let tok = CFStringTokenizerCreate(nil, ns as CFString, CFRange(location: 0, length: ns.length), kCFStringTokenizerUnitLineBreak, nil)!
    var out: [Seg] = []
    while CFStringTokenizerAdvanceToNextToken(tok).rawValue != 0 {
        let r = CFStringTokenizerGetCurrentTokenRange(tok)
        let piece = ns.substring(with: NSRange(location: r.location, length: r.length))
        if let s = segCache[piece] { out.append(s); continue }
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: piece, attributes: attrs))
        let seg = Seg(width: CTLineGetTypographicBounds(line, nil, nil, nil), trailing: CTLineGetTrailingWhitespaceWidth(line))
        segCache[piece] = seg; out.append(seg)
    }
    return out
}
func arithmeticLines(_ segs: [Seg], _ width: Double) -> Int {
    var lines = 0, x = 0.0, any = false
    for s in segs {
        if any && x + s.width - s.trailing > width { lines += 1; x = 0 }
        x += s.width; any = true
    }
    return lines + (any ? 1 : 0)
}

func best(_ reps: Int, _ body: () -> Void) -> Double {
    var m = Double.infinity
    for _ in 0..<reps { let t = CACurrentMediaTime(); body(); m = min(m, (CACurrentMediaTime() - t) * 1000) }
    return m
}

var typesetters: [(CTTypesetter, Int)] = []
let prepareMs = best(5) {
    typesetters = []
    for t in texts {
        let ts = CTTypesetterCreateWithAttributedString(NSAttributedString(string: t, attributes: attrs))
        _ = CTTypesetterSuggestLineBreak(ts, 0, 320)   // force shaping if it is lazy
        typesetters.append((ts, t.utf16.count))
    }
}
var total = 0
let breakMs = best(5) { total = 0; for w in widths { for (ts, n) in typesetters { total += lineCount(ts, n, w) } } }
var height = 0.0
let snapshotMs = best(5) { for w in widths { for (ts, n) in typesetters { height += snapshot(ts, n, w) } } }

// The host's `overflow-wrap: normal` pass (Text.swift `layout`): the paragraph's
// Unicode line-break boundaries, recomputed at every width although they do not depend on it.
func boundaries(_ text: String) -> Int {
    let ns = text as NSString
    let tok = CFStringTokenizerCreate(nil, ns as CFString, CFRange(location: 0, length: ns.length), kCFStringTokenizerUnitLineBreak, nil)!
    var n = 0
    while CFStringTokenizerAdvanceToNextToken(tok).rawValue != 0 { n += 1 }
    return n
}
var bounds = 0
let boundsMs = best(5) { bounds = 0; for t in texts { bounds += boundaries(t) } }

var prepared: [[Seg]] = []
let segColdMs = best(1) { prepared = texts.map(segments) }            // cold: every distinct segment measured
let segWarmMs = best(5) { prepared = texts.map(segments) }            // warm: widths from the cache
var arith = 0
let arithMs = best(5) { arith = 0; for w in widths { for s in prepared { arith += arithmeticLines(s, w) } } }

// Agreement: does the arithmetic breaker give CoreText's line count?
var disagree = 0, cases = 0, worst = 0
for w in widths { for (i, (ts, n)) in typesetters.enumerated() {
    let a = arithmeticLines(prepared[i], w), c = lineCount(ts, n, w)
    cases += 1; if a != c { disagree += 1; worst = max(worst, abs(a - c)) }
} }

let per = Double(count), layouts = Double(count * widths.count)
print("paragraphs \(count), mean \(chars / count) UTF-16 units, \(widths.count) widths, \(total) lines broken by CoreText, \(arith) by arithmetic, \(segCache.count) distinct segments")
print(String(format: "CoreText prepare  (typesetter + first break):  %.1f µs per paragraph", prepareMs * 1000 / per))
print(String(format: "CoreText re-break (SuggestLineBreak only):     %.2f µs per paragraph-width", breakMs * 1000 / layouts))
print(String(format: "CoreText snapshot (breaks + CTLines + bounds): %.2f µs per paragraph-width", snapshotMs * 1000 / layouts))
print(String(format: "line-break boundaries (CFStringTokenizer):      %.1f µs per paragraph, %d boundaries", boundsMs * 1000 / per, bounds))
print(String(format: "segments prepare, cold / warm cache:           %.1f / %.1f µs per paragraph", segColdMs * 1000 / per, segWarmMs * 1000 / per))
print(String(format: "arithmetic re-break over cached widths:        %.2f µs per paragraph-width", arithMs * 1000 / layouts))
print(String(format: "line count differs from CoreText in %d of %d paragraph-widths (%.2f%%), by at most %d line(s)", disagree, cases, 100 * Double(disagree) / Double(cases), worst))
```
