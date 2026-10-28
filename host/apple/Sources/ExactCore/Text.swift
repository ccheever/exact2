// Core Text measurement, shaping and paragraph geometry shared by headless
// and presented sessions (LLP 1126 D8, LLP 1008 §3). ExactKit paints these same
// CTLines; it supplies paint attributes, controls and Markdown through
// TextPresentation. The ink index and ellipsized lines remain here because
// they are geometry over shaped text, with no drawing or platform view API.
// One Core Text engine measures and paints: TextKit wrapped about 5× slower,
// and measuring and painting with different engines costs correctness (exact1 LLP 0418/0430).
// Each session owns its font catalog and caches (LLP 1031 D12); process-wide
// Core Text file registration belongs to FontRegistry.
#if canImport(UIKit)
import UIKit
package typealias PlatformFont = UIFont
package typealias PlatformColor = UIColor
#else
import AppKit
package typealias PlatformFont = NSFont
package typealias PlatformColor = NSColor
#endif
import CExact
import CoreText

/// One styled run: what changes glyph metrics.
extension Run {
    /// This run at `scale` of its size, in the kernel's f32 arithmetic
    /// (`TextStyle::scaled`, LLP 1116.001 D1.4: a fitted paragraph).
    package func scaled(_ scale: Float) -> Run {
        guard scale != 1 else { return self }
        var run = self
        run.size = CGFloat(Float(size) * scale)
        run.lineHeight = lineHeight.map { CGFloat(Float($0) * scale) }
        run.letterSpacing = CGFloat(Float(letterSpacing) * scale)
        return run
    }
}

package struct Run: Hashable {
    package var text: String
    package var size: CGFloat
    package var weight: Int
    package var family: Int
    package var italic: Bool
    package var lineHeight: CGFloat?
    package var letterSpacing: CGFloat
    /// CSS `font-variant-numeric` bits: 1 is `tabular-nums`, the face's own
    /// `tnum` feature. It changes advances: a metric (LLP 1053 G4).
    package var numeric: Int = 0
    /// CSS `font-stretch`, percent of normal width: a metric (LLP 1019 D8).
    package var stretch: CGFloat = 100
    /// CSS `font-variation-settings`: a metric (LLP 1019 D9).
    package var variations: [FontAxis] = []
    package var color: [Double]? = nil
    package var decoration: String = ""
    /// The lines its decorating ancestors draw through it, each in that
    /// box's colour; nil where none does (`propagate`). Paint.
    package var underlineColor: [Double]? = nil
    package var strikeColor: [Double]? = nil
    package var href: String = ""
    /// The inline box's `background-color`: paint, never metrics.
    package var background: [Double]? = nil
    /// The inline box that `background` is (its node's id, 0 for none) and
    /// that box's `border-radius` (top-left, top-right, bottom-right,
    /// bottom-left; empty for square): paint, never metrics. A box's runs
    /// share one background, rounded only at the box's own ends.
    package var backgroundBox: UInt32 = 0
    package var backgroundRadii: [InlineRadius] = []
    /// CSS visibility hides ink without changing shaping or descendant visibility.
    package var hidden = false
    /// Its colour is one its author fixed: a literal or a `light-dark()`
    /// pair, not a role and not unsaid (`BatchValue.fixesInk`). Selected on
    /// the Mac in the light appearance, such text keeps its ink, as an
    /// `NSTextView` leaves it (LLP 1115.000 D7.9). Paint, never metrics.
    package var fixedInk = false
    /// Its own `text-shadow` (offset x, y, blur, r g b a) when the
    /// paragraph's runs differ (`Spec.gatherShadows`), and its
    /// `-webkit-text-stroke` (width, r g b a): paint (LLP 1077 D3, D7).
    package var shadow: [Double]? = nil
    package var stroke: [Double]? = nil
    /// A Markdown list item's head indent and hung marker (LLP 1045 D4,
    /// `LineInsets`): where its paragraph's lines start, so a metric.
    package var indent: CGFloat = 0
    package var hang = false
    /// An image in the paragraph's text (LLP 1011.002): the box this run's
    /// U+FFFC reserves. A metric.
    package var box: InlineBox?

    package static func == (lhs: Run, rhs: Run) -> Bool {
        guard lhs.size == rhs.size, lhs.weight == rhs.weight, lhs.family == rhs.family,
              lhs.italic == rhs.italic, lhs.lineHeight == rhs.lineHeight,
              lhs.letterSpacing == rhs.letterSpacing, lhs.numeric == rhs.numeric, lhs.stretch == rhs.stretch,
              lhs.variations == rhs.variations, lhs.color == rhs.color,
              lhs.decoration == rhs.decoration, lhs.underlineColor == rhs.underlineColor,
              lhs.strikeColor == rhs.strikeColor, lhs.href == rhs.href,
              lhs.hidden == rhs.hidden, lhs.fixedInk == rhs.fixedInk, lhs.background == rhs.background, lhs.backgroundBox == rhs.backgroundBox,
              lhs.backgroundRadii == rhs.backgroundRadii, lhs.shadow == rhs.shadow, lhs.stroke == rhs.stroke,
              lhs.indent == rhs.indent, lhs.hang == rhs.hang, lhs.box == rhs.box else { return false }
        // CoreText's ranges address the original UTF16 source. Swift String's
        // canonical equality would alias NFC/NFD paragraphs with different
        // source lengths, so both equality and hashing use the exact UTF8.
        var a = lhs.text, b = rhs.text
        return a.withUTF8 { left in b.withUTF8 { right in left.elementsEqual(right) } }
    }

    package func hash(into hasher: inout Hasher) {
        // Native Strings expose their existing storage; no byte-array key or
        // full-text copy is created for each lookup. Foreign Strings may need
        // UTF8 materialization, but use the identical byte/hash contract.
        var value = text
        value.withUTF8 { hasher.combine(bytes: UnsafeRawBufferPointer($0)) }
        hasher.combine(size)
        hasher.combine(weight)
        hasher.combine(family)
        hasher.combine(italic)
        hasher.combine(lineHeight)
        hasher.combine(letterSpacing)
        hasher.combine(numeric)
        hasher.combine(stretch)
        hasher.combine(variations)
        hasher.combine(color)
        hasher.combine(decoration)
        hasher.combine(underlineColor)
        hasher.combine(strikeColor)
        hasher.combine(href)
        hasher.combine(background)
        hasher.combine(backgroundBox)
        hasher.combine(backgroundRadii)
        hasher.combine(hidden)
        hasher.combine(fixedInk)
        hasher.combine(shadow)
        hasher.combine(stroke)
        hasher.combine(indent)
        hasher.combine(hang)
        hasher.combine(box)
    }
    package init(text: String, size: CGFloat, weight: Int, family: Int, italic: Bool, lineHeight: CGFloat?, letterSpacing: CGFloat, numeric: Int = 0, stretch: CGFloat = 100, variations: [FontAxis] = [], color: [Double]? = nil, decoration: String = "", href: String = "", background: [Double]? = nil, hidden: Bool = false, fixedInk: Bool = false, shadow: [Double]? = nil, stroke: [Double]? = nil, indent: CGFloat = 0, hang: Bool = false) {
        self.text = text
        self.size = size
        self.weight = weight
        self.family = family
        self.italic = italic
        self.lineHeight = lineHeight
        self.letterSpacing = letterSpacing
        self.numeric = numeric
        self.stretch = stretch
        self.variations = variations
        self.color = color
        self.decoration = decoration
        self.href = href
        self.background = background
        self.hidden = hidden
        self.fixedInk = fixedInk
        self.shadow = shadow
        self.stroke = stroke
        self.indent = indent
        self.hang = hang
    }

    /// Adds the lines a decorating ancestor box draws through this run
    /// (CSS Text Decoration 3 §2.1: propagated, not inherited), in that
    /// box's `color`. Called from the nearest box outward: where boxes draw
    /// the same line, Chrome shows the nearest box's, and a run's own line
    /// over any box's.
    package mutating func propagate(_ lines: String, color: [Double]) {
        if lines.contains("underline"), underlineColor == nil { underlineColor = color }
        if lines.contains("line-through"), strikeColor == nil { strikeColor = color }
    }
}

/// A paragraph's specification: runs plus paragraph style.
package struct Spec: Hashable {
    package var runs: [Run]
    package var align: Int // 0 left, 1 center, 2 right, 3 justify
    package var lineClamp: Int
    package var color: [Double] // r g b a, 0–255
    package var overflowWrap: Int = 0 // CSS: normal, break-word, anywhere
    package var direction: Int = 0 // CSS: ltr, rtl
    package var whiteSpace: Int = 0 // CSS: normal, pre-wrap, nowrap, pre-line, pre (runs already collapsed unless pre-wrap or pre)
    /// `white-space-collapse: preserve` (pre-wrap, pre): spaces, tabs and breaks are text.
    package var preserves: Bool { whiteSpace == 1 || whiteSpace == 4 }
    /// `text-wrap-mode: wrap`: soft wrap opportunities may end a line (not nowrap, not pre).
    package var wraps: Bool { whiteSpace != 2 && whiteSpace != 4 }
    package var strut: Run? = nil // paragraph minimum line box, including smaller inline runs
    /// CSS `text-overflow: ellipsis` in a clipping box: paint ends an
    /// over-wide line in "…"; never metrics (LLP 1053 G5).
    package var ellipsis = false
    /// Collapsed → source offsets for the runs above (LLP 1053 G5).
    package var source = SourceMap()
    /// CSS `text-indent` in points, the paragraph's first line only.
    package var textIndent: CGFloat = 0
    /// CSS `hyphens`: 0 manual (the initial value), 1 none (soft hyphens
    /// already arrive as U+034F), 2 auto.
    package var hyphens = 0
    /// Synthetic emergency opportunities affect line choice even when shaped text matches authored soft hyphens.
    package var emergencyBreaks: [Int] = []
    /// The document language whose hyphenation points `auto` takes; empty
    /// for `none` and `manual`, which need none.
    package var language = ""
    /// CSS `text-shadow` (LLP 1077 D3) shared by every run: offset x, y and
    /// blur in points, then the colour's r g b a (0–255), resolved for the
    /// appearance. Nil when the runs' own differ (`gatherShadows`).
    package var shadow: [Double]? = nil
    /// CSS `text-box-trim` (0 none, 1 trim-start, 2 trim-end, 3 trim-both)
    /// and `text-box-edge`'s over (0 text, 1 cap, 2 ex) and under (0 text,
    /// 1 alphabetic) edges: the paragraph is cut to its fonts' own metrics,
    /// for measuring and painting alike.
    package var textBoxTrim = 0
    package var textBoxOver = 0
    package var textBoxUnder = 0
    /// CSS `text-wrap-style: balance` (TextBreaks.swift, `balance`).
    package var balance = false
    /// `-webkit-font-smoothing: antialiased` (LLP 1019 D12): the glyphs
    /// paint with Core Graphics font smoothing off. Paint only.
    package var antialiased = false
    package init(runs: [Run], align: Int, lineClamp: Int, color: [Double], overflowWrap: Int = 0, direction: Int = 0, whiteSpace: Int = 0, strut: Run? = nil, ellipsis: Bool = false, source: SourceMap = SourceMap(), textIndent: CGFloat = 0, hyphens: Int = 0, language: String = "", shadow: [Double]? = nil, textBoxTrim: Int = 0, textBoxOver: Int = 0, textBoxUnder: Int = 0, balance: Bool = false) {
        self.runs = runs
        self.align = align
        self.lineClamp = lineClamp
        self.color = color
        self.overflowWrap = overflowWrap
        self.direction = direction
        self.whiteSpace = whiteSpace
        self.strut = strut
        self.ellipsis = ellipsis
        self.source = source
        self.textIndent = textIndent
        self.hyphens = hyphens
        self.language = language
        self.shadow = shadow
        self.textBoxTrim = textBoxTrim
        self.textBoxOver = textBoxOver
        self.textBoxUnder = textBoxUnder
        self.balance = balance
    }
}

/// Where collapsed white space went, from `exact_text_collapse`: offsets
/// into the shaped (collapsed) text map to the node's source text. Carried
/// beside a Spec but never part of its identity: equal shaped text is equal.
package struct SourceMap: Hashable {
    package var edits: [ExactCollapseEdit] = []
    /// Where `hyphens: auto` put a soft hyphen, as shaped UTF-16 offsets
    /// (`Spec.hyphenateAuto`): text the source does not have.
    package var inserted: [Int] = []
    package static func == (_: SourceMap, _: SourceMap) -> Bool { true }
    package func hash(into hasher: inout Hasher) {}
    /// A shaped UTF-16 offset's source offset.
    package func source(_ shaped: Int) -> Int {
        var lo = 0, hi = inserted.count
        while lo < hi { let mid = (lo + hi) / 2; if inserted[mid] < shaped { lo = mid + 1 } else { hi = mid } }
        return beforeInsertion(shaped - lo)
    }
    /// A source UTF-16 offset's shaped offset; removed text maps to where it
    /// was, and an offset after an inserted soft hyphen stays after it.
    package func collapsed(_ source: Int) -> Int {
        var at = collapsedBeforeInsertion(source)
        for p in inserted where p <= at { at += 1 }
        return at
    }
    /// A collapsed UTF-16 offset's source offset.
    private func beforeInsertion(_ collapsed: Int) -> Int {
        var lo = 0, hi = edits.count
        while lo < hi { let mid = (lo + hi) / 2; if Int(edits[mid].utf16) <= collapsed { lo = mid + 1 } else { hi = mid } }
        return collapsed + (lo == 0 ? 0 : Int(edits[lo - 1].removed))
    }
    /// A source UTF-16 offset's collapsed offset; removed text maps to where it was.
    private func collapsedBeforeInsertion(_ source: Int) -> Int {
        var lo = 0, hi = edits.count
        while lo < hi { let mid = (lo + hi) / 2; if Int(edits[mid].utf16 + edits[mid].removed) <= source { lo = mid + 1 } else { hi = mid } }
        let at = source - (lo == 0 ? 0 : Int(edits[lo - 1].removed))
        return lo < edits.count && at >= Int(edits[lo].utf16) ? Int(edits[lo].utf16) : at
    }

    /// CSS white space collapsing of `runs` (normal, nowrap, pre-line), as the
    /// measurer collapses them in Rust before its callback: the same function.
    package static func collapse(_ runs: inout [Run], whiteSpace: Int) -> SourceMap {
        var joined = Data(); var lens: [Int] = []
        for r in runs { let bytes = Data(r.text.utf8); joined.append(bytes); lens.append(bytes.count) }
        return joined.withUnsafeBytes { raw -> SourceMap in
            let utf8 = raw.bindMemory(to: UInt8.self).baseAddress
            let mode = UInt8(whiteSpace)
            let count = exact_text_collapse(utf8, joined.count, lens, lens.count, mode, nil, nil, nil, 0)
            guard count > 0 else { return SourceMap() }
            var out = [UInt8](repeating: 0, count: joined.count)
            var outLens = [Int](repeating: 0, count: lens.count)
            var edits = [ExactCollapseEdit](repeating: ExactCollapseEdit(), count: count - 1)
            _ = exact_text_collapse(utf8, joined.count, lens, lens.count, mode, &out, &outLens, &edits, edits.count)
            var at = 0
            for i in runs.indices {
                runs[i].text = String(decoding: out[at..<at + outLens[i]], as: UTF8.self)
                at += outLens[i]
            }
            return SourceMap(edits: edits)
        }
    }
    package init(edits: [ExactCollapseEdit] = [], inserted: [Int] = []) {
        self.edits = edits; self.inserted = inserted
    }
}

/// CSS line heights use the web host's metrics and Chrome's rounding at scale 1.
/// UIKit positions glyphs at its face's ascent within that height.
/// On iOS PresentedText.measure returns nil; FieldChromeIOS keeps UIFont.lineHeight
/// for native editors. Reader line boxes do not change those control metrics.
package enum CSSLineBox {
    /// A text advance as the browser's layout holds it: rounded up to its
    /// 1/64 px layout unit, never to a whole point (a chip's width).
    package static func layoutWidth(_ width: CGFloat) -> CGFloat { ceil(width * 64) / 64 }

    /// The content area (an inline background's extent) above and below the baseline.
    package static func content(_ font: CTFont) -> (CGFloat, CGFloat) {
        #if canImport(UIKit)
        // Chrome system-ui uses SFNS's hhea metrics (1980/-432 at 2048 units/em).
        // SFUI paints the same UI family with different vertical metrics; keep
        // its glyphs, but use the web's line box for author-laid-out text.
        if (CTFontCopyPostScriptName(font) as String).hasPrefix(".SFUI-") {
            let size = CTFontGetSize(font)
            return ((size * 1980 / 2048).rounded(), (size * 432 / 2048).rounded())
        }
        #endif
        return (CTFontGetAscent(font).rounded(), CTFontGetDescent(font).rounded())
    }

    /// Above and below the baseline: `line-height: normal` when `height` is
    /// nil, else the authored line height.
    package static func extents(_ font: CTFont, height: CGFloat?) -> (CGFloat, CGFloat) {
        let (ascent, descent) = content(font)
        guard let height else {
            let gap = CTFontGetLeading(font).rounded()
            return (ascent + floor(gap / 2), descent + ceil(gap / 2))
        }
        guard height.isFinite else { return (ascent, descent) }
        let above = ascent + floor((height - ascent - descent) / 2)
        return (above, height - above)
    }
}

/// Where a measured paragraph's lines break, as plain values: what
/// measurement publishes and painting shapes its own lines from (LLP 1072
/// §8.1). `clamped`: a `line-clamp`'s last line, made again ending in "…"
/// (`TextEngine.clampedLine`).
package struct LineGeometry {
    package let ranges: [CFRange]
    package let baselines: [CGFloat]
    package var clamped: CFRange? = nil
    package init(ranges: [CFRange], baselines: [CGFloat], clamped: CFRange? = nil) {
        self.ranges = ranges; self.baselines = baselines; self.clamped = clamped
    }
    /// A clamped last line's own range is the ellipsized line's; the
    /// geometry keeps the range it broke at.
    package init(_ p: Paragraph) {
        var ranges = p.lines.map { CTLineGetStringRange($0) }
        if let clamped = p.clampedRange, !ranges.isEmpty { ranges[ranges.count - 1] = clamped }
        self.init(ranges: ranges, baselines: p.baselines, clamped: p.clampedRange)
    }
}

/// A wrapped paragraph at one width: what is measured is what is painted.
package final class Paragraph {
    package let lines: [CTLine]
    /// Baseline of each line, measured from the top.
    package let baselines: [CGFloat]
    /// Flow origins already include interval alignment; empty for ordinary text.
    package let origins: [CGFloat]
    /// Where each line starts: `text-indent`, a list item's head indent.
    package var insets = LineInsets()
    /// Logical source ownership, including trimmed whitespace, one per fragment.
    package let fragments: [ExactFlowFragment]
    package var flowIncomplete = false
    /// A `line-clamp` paragraph whose last line ends in "…": that line's
    /// range as it broke, before the ellipsis (`TextEngine.clampedLine`).
    package var clampedRange: CFRange?
    package let flowLineHeight: CGFloat
    package let width: CGFloat
    package let height: CGFloat
    package let lineBottoms: [CGFloat]
    package let shape: TextShape?
    package let residencyKey: TextParagraphKey?
    package let coreTextEstimateBytes: Int
    package var ownedPayloadBytes: Int {
        lines.count * MemoryLayout<CTLine>.stride
            + (baselines.count + lineBottoms.count + origins.count) * MemoryLayout<CGFloat>.stride
            + fragments.count * MemoryLayout<ExactFlowFragment>.stride
            + (cachedInk?.storageBytes ?? 0)
    }
    /// Admission reserves the known lazy array shape, without constructing ink.
    /// Diagnostics still report only the payload actually allocated above.
    package var admissionPayloadBytes: Int {
        ownedPayloadBytes - (cachedInk?.storageBytes ?? 0)
            + ParagraphInkIndex.storageBytes(lineCount: lines.count)
    }
    private(set) package var cachedInk: ParagraphInkIndex?
    /// The C seam uses -1 when no line supplies a baseline.
    package var firstBaseline: CGFloat { baselines.first ?? -1 }
    private var truncated: (width: CGFloat, lines: [Int: CTLine])?

    /// CSS `text-overflow: ellipsis`: a line wider than the box ends in "…",
    /// as painted; the measured line (and every metric) is unchanged.
    package func ellipsized(_ index: Int, spec: Spec, width: CGFloat) -> CTLine {
        let line = lines[index]
        guard CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) - CTLineGetTrailingWhitespaceWidth(line) > width + 0.5,
              let source = shape?.attributed else { return line }
        if truncated?.width != width { truncated = (width, [:]) }
        if let kept = truncated?.lines[index] { return kept }
        let range = CTLineGetStringRange(line)
        let made = TextEngine.ellipsis(line, range: NSRange(location: range.location, length: range.length),
                                       width: Double(width), source: source) ?? line
        truncated?.lines[index] = made
        return made
    }
    package init(lines: [CTLine], baselines: [CGFloat], width: CGFloat, height: CGFloat, lineBottoms: [CGFloat] = [],
         shape: TextShape? = nil, offeredWidth: CGFloat? = nil, glyphCount: Int = 0,
         origins: [CGFloat] = [], fragments: [ExactFlowFragment] = [], flowLineHeight: CGFloat = 0) {
        self.origins = origins; self.fragments = fragments; self.flowLineHeight = flowLineHeight
        self.shape = shape
        residencyKey = shape.flatMap { shape in offeredWidth.map { TextParagraphKey(shape: shape.key, width: $0) } }
        coreTextEstimateBytes = glyphCount * 64 + lines.count * 256
        self.lineBottoms = lineBottoms
        self.lines = lines
        self.baselines = baselines
        self.width = width
        self.height = height
    }

    /// Built only when a dirty viewport is first painted, then shared by every
    /// subsequent clip of this immutable paragraph. Measurement stays ink-free.
    package func inkBounds() -> ParagraphInkIndex {
        if let cachedInk { return cachedInk }
        let index = ParagraphInkIndex(lines: lines, baselines: baselines)
        cachedInk = index
        return index
    }
}

/// A segment tree in logical paint order. Each node encloses the ink of its
/// descendant lines; pruning is safe even when baselines go backwards or many
/// zero-height lines overlap. Horizontal culling would also need alignment and
/// overhang, so this index deliberately considers only the dirty vertical band.
package final class ParagraphInkIndex {
    private struct Span {
        var top: CGFloat = .infinity
        var bottom: CGFloat = -.infinity
    }
    private let spans: [Span]
    private let leaves: Int
    private let count: Int
    /// Array payload only: excludes the object/array headers and allocator slack.
    package var storageBytes: Int { spans.count * MemoryLayout<Span>.stride }

    private static func leafCount(_ count: Int) -> Int {
        var size = 1
        while size < count { size *= 2 }
        return size
    }
    package static func storageBytes(lineCount: Int) -> Int {
        leafCount(lineCount) * 2 * MemoryLayout<Span>.stride
    }
    package init(lines: [CTLine], baselines: [CGFloat]) {
        count = lines.count
        let size = Self.leafCount(count)
        leaves = size
        var spans = [Span](repeating: Span(), count: size * 2)
        for (i, line) in lines.enumerated() {
            let ink = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            _ = CTLineGetTypographicBounds(line, &ascent, &descent, &leading)
            // Glyph paths include overhang; typographic extents also include
            // whitespace and decoration space. Retain the existing 2pt raster
            // allowance and use the same rounded baseline as CTLineDraw.
            let above = max(ascent + max(leading, 0), ink.isNull ? 0 : ink.maxY)
            let below = max(descent + max(leading, 0), ink.isNull ? 0 : -ink.minY)
            let top = baselines[i].rounded() - above - 2
            let bottom = baselines[i].rounded() + below + 2
            spans[size + i] = top.isFinite && bottom.isFinite
                ? Span(top: top, bottom: bottom) : Span(top: -.infinity, bottom: .infinity)
        }
        if size > 1 {
            for i in stride(from: size - 1, through: 1, by: -1) {
                spans[i] = Span(top: min(spans[i * 2].top, spans[i * 2 + 1].top),
                                bottom: max(spans[i * 2].bottom, spans[i * 2 + 1].bottom))
            }
        }
        self.spans = spans
    }

    package func forEachLine(from top: CGFloat, through bottom: CGFloat, _ body: (Int) -> Void) {
        func visit(_ node: Int) {
            let span = spans[node]
            guard span.top <= bottom && span.bottom >= top else { return }
            if node >= leaves {
                let line = node - leaves
                if line < count { body(line) }
            } else {
                visit(node * 2)
                visit(node * 2 + 1)
            }
        }
        visit(1)
    }
}

/// CoreText's process-wide registration, once per URL, never undone.
package enum FontRegistry {
    nonisolated(unsafe) private static var registered: Set<URL> = []
    /// The painter and the measurer each register the catalog they install.
    private static let lock = NSLock()

    package static func register(_ url: URL) -> Bool {
        lock.lock(); defer { lock.unlock() }
        if registered.contains(url) { return true }
        var error: Unmanaged<CFError>?
        let ok = CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error)
        if ok { registered.insert(url); return true }
        // A dev reload may repeat the URL; another installed or prior-plan
        // file may own the same PostScript name. In both cases the URL's own
        // descriptor still binds this plan to its exact bytes.
        if let e = error?.takeRetainedValue() as Error? {
            let ns = e as NSError
            let tolerated = ns.domain == kCTFontManagerErrorDomain as String
                && (ns.code == CTFontManagerError.alreadyRegistered.rawValue
                    || ns.code == CTFontManagerError.duplicatedName.rawValue)
            if tolerated { registered.insert(url) }
            return tolerated
        }
        return false
    }
}

/// A session's text: two engines of one kind, never shared (LLP 1072 §8.1).
/// This one paints, on main; its `measurer` answers the kernel's
/// measurements on the owner thread. Each keeps its own fonts, shaped text
/// and caches; what painting reuses from measuring crosses as plain values
/// (`TextAnswers`): metrics, line breaks and optional glyph copies with immutable fonts.
package final class TextEngine {
    package let presentation: TextPresentation?
    /// The engine the kernel measures with, on the owner thread; nil in the
    /// measurer itself.
    private(set) package var measurer: TextEngine?
    /// The painting engine a measurer belongs to.
    private(set) package weak var painter: TextEngine?
    /// Where the measurer publishes what it measured: it answers from there
    /// before typesetting, and the painter takes its line breaks there.
    private(set) package var answers: TextAnswers?
    /// This engine's key for a lookup there (`TextAnswerKey`), reused.
    private var answerKey: [UInt8] = []
    /// Whether this engine is a measurer whose answers are published: its
    /// residency then keeps shaped text only, the table the answers.
    private var publishes: Bool { painter != nil && (answers?.limit ?? 0) > 0 }
    package var fonts: [String: PlatformFont] = [:]
    private var residency: TextResidency
    package var residencyStats: TextResidencyStats { residency.stats }
    /// The NodeView's existing cachedTextLayout is the accepted lease. The
    /// cache keeps only a weak lookup once that view owns the paragraph.
    package func accepted(_ paragraph: Paragraph) { residency.accepted(paragraph) }
    /// Under memory pressure, shaped text no view holds is dropped, as the
    /// raster loader drops its cold images.
    package func dropCold() { residency.dropCold(); answers?.discardGlyphs() }
    /// At rest (`ExactSession.rest`): shaped text no view holds goes.
    package func dropColdShaped() { residency.dropColdShaped(); answers?.discardGlyphs() }
    /// A screen shows about `visibleParagraphs` paragraphs: cold shaped
    /// text is held to two screens of them (`TextResidency.fitShaped`).
    package func fitShaped(visibleParagraphs: Int) { residency.fitShaped(visibleParagraphs: visibleParagraphs) }
    package var catalog: [Int: [RegisteredFace]] = [:]
    /// Declared family names to their plan stacks, for Canvas 2D's `font`
    /// (LLP 1056 D8).
    private var familyStacks: [String: Int] = [:]
    /// Canvas 2D's fonts and lines over this engine (LLP 1056 D8), which the
    /// Surfaces module makes at first use (LLP 1047.001 D4).
    package var canvasTextCache: AnyObject?

    /// A declared family's stack, by name.
    package func stack(named name: String) -> Int? { familyStacks[name] }
    /// Where a declared face's relative source resolves: the app's resolver
    /// (LLP 1031 D1 — the committed complete generation, else the root).
    package let resolve: (String) -> URL?
    package let read: (String) -> Data?
    /// A face's file in the signed app bundle, when the generation is the
    /// embedded one (LLP 1019 D5: the descriptor from the sandboxed URL).
    /// CoreText maps it, so its pages stay clean; a delivered generation has
    /// none and binds the verified bytes instead.
    package let bundled: (String) -> URL?
    private var pendingFonts: [URL] = []
    /// Native callback entries, native cache hits, and native cache/layout time
    /// since session start. Rust identified-metric hits bypass this callback;
    /// the timer below excludes C-run decoding and Swift String construction.
    package var measureCount = 0
    package var measureHits = 0
    package var measureSeconds = 0.0

    package init(assets resolve: @escaping (String) -> URL?, read: ((String) -> Data?)? = nil,
         bundled: @escaping (String) -> URL? = { _ in nil },
         coldTextTargetBytes: Int = TextResidency.defaultSoftTargetBytes, presentation: TextPresentation? = nil) {
        self.presentation = presentation
        residency = TextResidency(softTargetBytes: coldTextTargetBytes)
        self.resolve = resolve
        self.bundled = bundled
        self.read = read ?? { name in resolve(name).flatMap { try? Data(contentsOf: $0) } }
    }

    package func connect(measurer: TextEngine, answerBytes: Int) {
        let answers = TextAnswers(limit: answerBytes)
        self.measurer = measurer; self.answers = answers
        measurer.painter = self; measurer.answers = answers
        measurer.residency.keepsAnswers = !measurer.publishes
    }
    /// The context the kernel's measure and font hooks get: the measurer's.
    package var measuring: TextEngine { measurer ?? self }

    package func commitFonts() {
        for url in pendingFonts { _ = FontRegistry.register(url) }
        pendingFonts = []
        if let m = measurer { Owner.shared.sync { m.commitFonts() } }
    }

    /// This engine as the context the C callbacks hand back.
    package var opaque: UnsafeMutableRawPointer { Unmanaged.passUnretained(self).toOpaque() }

    /// The live plan's exact text state while a reload candidate boots.
    /// Dictionary copies retain the already-shaped paragraphs and fonts;
    /// candidate `removeAll` calls detach through copy-on-write.
    package final class Checkpoint {
        private let pendingFonts: [URL]
        private let fonts: [String: PlatformFont]
        private let residency: TextResidency
        private let catalog: [Int: [RegisteredFace]]
        private let familyStacks: [String: Int]
        private let restorePresentation: (() -> Void)?
        private let measurer: Checkpoint?

        fileprivate init(_ engine: TextEngine) {
            // The measurer's state is the owner's (LLP 1072 §8.1).
            measurer = engine.measurer.map { m in Owner.shared.sync { Checkpoint(m) } }
            restorePresentation = engine.presentation?.checkpoint()
            pendingFonts = engine.pendingFonts
            fonts = engine.fonts
            residency = engine.residency
            catalog = engine.catalog
            familyStacks = engine.familyStacks
        }

        fileprivate func restore(into engine: TextEngine) {
            restorePresentation?()
            engine.pendingFonts = pendingFonts
            engine.fonts = fonts
            engine.residency = residency
            engine.residency.refreshAfterRestore()
            engine.catalog = catalog
            engine.familyStacks = familyStacks
            engine.canvasTextCache = nil
            engine.dropMeasuredBreaks()
            if let measurer, let m = engine.measurer { Owner.shared.sync { measurer.restore(into: m) } }
        }
    }

    package func checkpoint() -> Checkpoint { Checkpoint(self) }
    package func restore(_ checkpoint: Checkpoint) { checkpoint.restore(into: self) }

    /// Replace this session's plan-scoped catalog before layout. A fresh
    /// residency namespace prevents old accepted font identities from aliasing
    /// the candidate; checkpoints retain and restore their original namespace.
    package func install(_ pointer: UnsafePointer<ExactFontCatalog>?) {
        presentation?.reset()
        fonts.removeAll(keepingCapacity: true)
        residency = TextResidency(softTargetBytes: residency.softTargetBytes)
        residency.keepsAnswers = !publishes
        dropMeasuredBreaks()
        catalog.removeAll(keepingCapacity: true)
        familyStacks.removeAll()
        canvasTextCache = nil
        guard let value = pointer?.pointee else { return }
        let rows = UnsafeBufferPointer(start: value.faces, count: value.count)
        var staged: [Int: [RegisteredFace]] = [:]
        var failed = Set<Int>()
        for row in rows {
            let stack = Int(row.stack)
            if let name = row.family, row.family_len > 0 {
                let name = String(decoding: UnsafeBufferPointer(start: name, count: row.family_len), as: UTF8.self)
                if familyStacks[name] == nil { familyStacks[name] = stack }
            }
            let name = row.family.map { String(decoding: UnsafeBufferPointer(start: $0, count: row.family_len), as: UTF8.self) } ?? ""
            if row.source_len == 0 {
                if stack < 8 { continue }
                let generics = ["system-ui", "ui-sans-serif", "sans-serif", "ui-serif", "serif", "ui-monospace", "monospace", "ui-rounded"]
                let generic = row.weight == 1 ? generics.firstIndex(of: name.lowercased()) : nil
                let descriptor: CTFontDescriptor?
                if let generic { descriptor = CTFontCopyFontDescriptor(font(size: 16, weight: 400, family: generic, italic: false) as CTFont) }
                else {
                    // An installed family is its real faces, matched as declared
                    // ones are: traits on a family descriptor never chose the
                    // italic or bold file (the reader diary's Georgia).
                    staged[stack, default: []] += Self.installedFaces(name)
                    continue
                }
                if let descriptor { staged[stack, default: []].append(RegisteredFace(family: name, generic: generic, weight: 0, italic: false, descriptor: descriptor)) }
                continue
            }
            guard let sourceBytes = row.source else { failed.insert(stack); continue }
            let source = String(decoding: UnsafeBufferPointer(start: sourceBytes, count: row.source_len), as: UTF8.self)
            guard URL(string: source)?.scheme == nil, !source.hasPrefix("/"),
                  let descriptors = descriptors(source), let descriptor = descriptors.first else {
                failed.insert(stack)
                continue
            }
            if let url = fontURL(source) { pendingFonts.append(url) }
            // One declared weight is pinned on the axis now; a range is set
            // per request (`RegisteredFace.drawn`).
            let weights = Int(row.weight)...max(Int(row.weight), Int(row.weight_max))
            let stretches = CGFloat(row.stretch_min) / 10...CGFloat(max(row.stretch_min, row.stretch_max)) / 10
            staged[stack, default: []].append(RegisteredFace(
                family: name, generic: nil, weights: weights, stretches: stretches, italic: row.italic != 0,
                descriptor: weights.count == 1 ? Self.declaredFace(descriptor, weight: Int(row.weight)) : descriptor))
        }
        for stack in failed {
            staged.removeValue(forKey: stack)
            fputs("[Fonts] font.registration.failed: stack=\(stack)\n", stderr)
        }
        catalog = staged
    }

    /// A declared face's descriptors: from its bundled file, else its bytes.
    private func descriptors(_ source: String) -> [CTFontDescriptor]? {
        if let file = bundled(source) {
            return CTFontManagerCreateFontDescriptorsFromURL(file as CFURL) as? [CTFontDescriptor]
        }
        guard let bytes = read(source) else { return nil }
        return CTFontManagerCreateFontDescriptorsFromData(bytes as CFData) as? [CTFontDescriptor]
    }

    private func fontURL(_ source: String) -> URL? {
        guard URL(string: source)?.scheme == nil, !source.hasPrefix("/") else { return nil }
        return resolve(source)
    }

    package func font(_ run: Run) -> PlatformFont {
        font(size: run.size, weight: run.weight, family: run.family, italic: run.italic, numeric: run.numeric, stretch: run.stretch,
             variations: run.variations)
    }

    /// CSS `tabular-nums` is the chosen face's own OpenType `tnum` feature,
    /// never a substitute monospaced face; a face without it is unchanged.
    package func font(size: CGFloat, weight: Int, family: Int, italic: Bool, numeric: Int, stretch: CGFloat = 100,
                      variations: [FontAxis] = []) -> PlatformFont {
        let base = font(size: size, weight: weight, family: family, italic: italic, stretch: stretch, variations: variations)
        guard numeric & 1 != 0 else { return base }
        let key = "\(family)/\(size)/\(weight)/\(italic)/\(stretch)/\(FontAxis.key(variations))/tnum"
        if let f = fonts[key] { return f }
        let settings = [[kCTFontOpenTypeFeatureTag: "tnum", kCTFontOpenTypeFeatureValue: 1]] as CFArray
        let descriptor = CTFontDescriptorCreateWithAttributes([kCTFontFeatureSettingsAttribute: settings] as CFDictionary)
        let f = CTFontCreateCopyWithAttributes(base as CTFont, size, nil, descriptor) as PlatformFont
        fonts[key] = f
        return f
    }

    /// `stretch` is CSS `font-stretch`: it chooses among a declared family's
    /// faces and sets a variable face's `wdth`, a platform family's too
    /// (San Francisco's widths). `variations` is CSS
    /// `font-variation-settings`, set after both.
    package func font(size: CGFloat, weight: Int, family: Int, italic: Bool, stretch: CGFloat = 100,
                      variations: [FontAxis] = []) -> PlatformFont {
        if let f = presentation?.controlFont(self, size: size, weight: weight, family: family, italic: italic) { return f }
        let key = stretch == 100 && variations.isEmpty ? "\(family)/\(size)/\(weight)/\(italic)"
            : "\(family)/\(size)/\(weight)/\(italic)/\(stretch)/\(FontAxis.key(variations))"
        if let f = fonts[key] { return f }
        if let faces = catalog[family], !faces.isEmpty {
            let face = TextEngine.matched(faces, weight: weight, italic: italic, stretch: stretch)
            func candidate(_ face: RegisteredFace) -> PlatformFont {
                if let generic = face.generic {
                    return font(size: size, weight: weight, family: generic, italic: italic, stretch: stretch, variations: variations)
                }
                return CTFontCreateWithFontDescriptor(face.drawn(weight: weight, stretch: stretch, size: size, variations: variations),
                                                      size, nil) as PlatformFont
            }
            let base = candidate(face)
            // The other families fall back in stack order; a one-family stack
            // is its face alone.
            var seen: Set<String> = [face.family], cascade: [CTFontDescriptor] = []
            for member in faces where seen.insert(member.family).inserted {
                let group = faces.filter { $0.family == member.family }
                cascade.append(CTFontCopyFontDescriptor(candidate(TextEngine.matched(group, weight: weight, italic: italic, stretch: stretch)) as CTFont))
            }
            guard !cascade.isEmpty else { fonts[key] = base; return base }
            let descriptor = CTFontDescriptorCreateWithAttributes([kCTFontCascadeListAttribute: cascade] as CFDictionary)
            let f = CTFontCreateCopyWithAttributes(base as CTFont, size, nil, descriptor) as PlatformFont
            fonts[key] = f
            return f
        }
        // Each hundred's named instance is kept under its own key, so a
        // weight between two (a transition's frame) makes one font.
        var f = weight % 100 == 0 && (100...900).contains(weight)
            ? namedSystemFace(size: size, weight: TextEngine.namedWeight(weight), family: family, italic: italic)
            : TextEngine.cssWeight(weight, size: size) { self.font(size: size, weight: $0, family: family, italic: italic) }
        f = TextEngine.cssAxes(f, stretch: stretch, variations: variations, size: size)
        fonts[key] = f
        return f
    }

    /// A platform generic at a named weight, as UIKit (AppKit) makes it.
    private func namedSystemFace(size: CGFloat, weight w: PlatformFont.Weight, family: Int, italic: Bool) -> PlatformFont {
        var f = (family == 5 || family == 6)
            ? PlatformFont.monospacedSystemFont(ofSize: size, weight: w)
            : PlatformFont.systemFont(ofSize: size, weight: w)
        if family == 3 || family == 4 || family == 7 {
            #if canImport(UIKit)
            let design: UIFontDescriptor.SystemDesign = family == 7 ? .rounded : .serif
            if let d = f.fontDescriptor.withDesign(design) { f = UIFont(descriptor: d, size: size) }
            #else
            let design: NSFontDescriptor.SystemDesign = family == 7 ? .rounded : .serif
            if let d = f.fontDescriptor.withDesign(design), let designed = NSFont(descriptor: d, size: size) { f = designed }
            #endif
        }
        // A declared stack is drawn from its real descriptors (`font` above).
        // This trait resolver is only for a platform generic, never a shear
        // applied to custom bytes (LLP 1019 §5).
        if italic {
            #if canImport(UIKit)
            // Its bold trait kept: semibold and heavier would be regular italic.
            if let d = f.fontDescriptor.withSymbolicTraits(f.fontDescriptor.symbolicTraits.union(.traitItalic)) { f = UIFont(descriptor: d, size: size) }
            #else
            // AppKit's font manager is main's (LLP 1072 §8.1): a variant first
            // met while the owner thread measures is resolved there.
            let upright = f
            f = Owner.shared.callMain { NSFontManager.shared.convert(upright, toHaveTrait: .italicFontMask) }
            #endif
        }
        return f
    }

    /// A color from the style dictionary's `[r,g,b,a]` bytes (sRGB), or
    /// `textChannels`' nine, in the colour's own space (LLP 1100 D2).

    package func attributed(_ spec: Spec) -> NSAttributedString {
        // The measure callback sees dense paragraphs as many small runs. Build
        // their text once, then assign UTF-16 spans, instead of allocating and
        // appending an attributed string for every run.
        let s = NSMutableAttributedString(string: spec.runs.map(\.text).joined())
        var offset = 0
        for (index, r) in spec.runs.enumerated() {
            var a: [NSAttributedString.Key: Any] = [.font: font(r)]
            if r.letterSpacing != 0 { a[.kern] = r.letterSpacing }
            presentation?.attributes(r, color: spec.color, attributes: &a)
            if let box = r.box { TextEngine.boxAttributes(box, font: font(r) as CTFont, index: index, into: &a) }
            let length = r.text.utf16.count
            if length > 0 { s.setAttributes(a, range: NSRange(location: offset, length: length)) }
            offset += length
        }
        if spec.lineClamp > 0, s.length > 0, var block = spec.strut ?? spec.runs.first {
            if block.shadow == spec.shadow { block.shadow = nil }
            var a: [NSAttributedString.Key: Any] = [.font: font(block)]
            if block.letterSpacing != 0 { a[.kern] = block.letterSpacing }
            presentation?.attributes(block, color: spec.color, attributes: &a)
            s.addAttribute(.exactClampStyle, value: a, range: NSRange(location: 0, length: s.length))
        }
        TextEngine.setBaseDirection(s, direction: spec.direction)
        presentation?.finish(s)
        if spec.preserves, s.string.contains("\t"), let first = spec.strut ?? spec.runs.first {
            setTabStops(s, run: first)
        }
        return s
    }

    /// Eight spaces of `font`, letter spacing included: a tab stop's interval.
    package static func tabInterval(_ font: CTFont, letterSpacing: CGFloat) -> CGFloat {
        let space = CTLineGetTypographicBounds(CTLineCreateWithAttributedString(
            NSAttributedString(string: " ", attributes: [.font: font])), nil, nil, nil)
        return 8 * (CGFloat(space) + letterSpacing)
    }

    /// CSS `tab-size: 8` (its initial value): a preserved tab advances to the
    /// next multiple of eight spaces of the paragraph's own font, letter
    /// spacing included, from the line's start. CoreText's default is twelve
    /// stops 28 pt apart; Chrome's is this (LLP 1053 G5).
    private func setTabStops(_ s: NSMutableAttributedString, run: Run) {
        TextEngine.setTabStops(s, interval: TextEngine.tabInterval(font(run) as CTFont, letterSpacing: run.letterSpacing))
    }

    package static func setTabStops(_ s: NSMutableAttributedString, interval: CGFloat) {
        guard interval > 0, s.length > 0 else { return }
        let range = NSRange(location: 0, length: s.length)
        let paragraph = ((s.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle) ?? NSMutableParagraphStyle()
        paragraph.tabStops = []
        paragraph.defaultTabInterval = interval
        s.addAttribute(.paragraphStyle, value: paragraph, range: range)
    }

    /// CSS direction sets the paragraph base; normal Unicode bidi still
    /// orders each run. The first strong character never selects the base.
    package static func setBaseDirection(_ s: NSMutableAttributedString, direction: Int) {
        let range = NSRange(location: 0, length: s.length)
        s.enumerateAttribute(.paragraphStyle, in: range) { value, range, _ in
            let paragraph = (value as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle ?? NSMutableParagraphStyle()
            paragraph.baseWritingDirection = direction == 1 ? .rightToLeft : .leftToRight
            s.addAttribute(.paragraphStyle, value: paragraph, range: range)
        }
    }

    /// Urgent raster work stays on the text engine's owning thread and uses
    /// the existing bounded residency policy for its exact painted typesetter.
    package func rasterLines(_ spec: Spec, ranges: [CFRange], width: CGFloat = .infinity) -> (NSAttributedString, [CTLine]) {
        let identity = residency.identity(spec)
        let source = shape(TextShapeKey(identity: identity, paint: TextPaint(spec)), identity: identity)
        let insets = source.lineInsets
        return (source.attributed, ranges.map {
            let inset = insets.at($0.location).width
            return TextEngine.finishedLine(CTTypesetterCreateLine(source.typesetter, $0), source: source.attributed, range: $0,
                                           justify: spec.align == 3 ? Double(width - inset) : nil)
        })
    }

    /// The line geometry the kernel's measurement of `spec` at `width`
    /// produced, if that measurement is still resident. Plain values: a
    /// worker typesets its own lines from them (TextRasterJob).
    package func measuredBreaks(_ spec: Spec, width: CGFloat) -> LineGeometry? {
        let identity = residency.identity(spec)
        if let kept = measuredBreakCache[MeasuredBreakKey(token: identity.token, width: width)] { return kept }
        if let published = publishedLines(identity, width: width) { return published }
        if let measured = residency.geometry(identity, width: width) { return LineGeometry(measured) }
        // Scalar answers keep lines only for unclamped text (TextResidency).
        return residency.answerLines(identity, width: width).map { LineGeometry(ranges: $0.0, baselines: $0.1) }
    }

    /// The breaks of recent definite-width measurements, as plain values. The
    /// measured paragraph itself is weakly resident and is usually gone by
    /// the time its row asks for pixels; without these the main thread would
    /// typeset the paragraph a second time just to tell a worker where its
    /// lines end.
    private struct MeasuredBreakKey: Hashable {
        let token: TextIdentityToken
        let width: CGFloat
    }
    private var measuredBreakCache: [MeasuredBreakKey: LineGeometry] = [:]
    private var measuredBreakOrder: [MeasuredBreakKey] = []
    private static let measuredBreakLimit = 512

    /// Counts identity namespaces: pixels kept by paint (`TextRasterizer`)
    /// under an older one may name another face by the same family index.
    private(set) package var namespace = 0

    /// A new identity namespace (a font catalog, a restored checkpoint) keys nothing here.
    private func dropMeasuredBreaks() {
        answers?.clear()
        namespace += 1
        measuredBreakCache.removeAll(keepingCapacity: true)
        measuredBreakOrder.removeAll(keepingCapacity: true)
    }

    private func keepBreaks(_ p: Paragraph, identity: TextIdentity, width: CGFloat) {
        let key = MeasuredBreakKey(token: identity.token, width: width)
        // Published again when the table let it go: a put it already holds is a touch.
        if let kept = measuredBreakCache[key] { publish(identity, offer: TextAnswerKey.offer(width: width), p, lines: kept); return }
        if measuredBreakOrder.count >= Self.measuredBreakLimit {
            measuredBreakCache.removeValue(forKey: measuredBreakOrder.removeFirst())
        }
        measuredBreakOrder.append(key)
        let geometry = LineGeometry(p)
        measuredBreakCache[key] = geometry
        publish(identity, offer: TextAnswerKey.offer(width: width), p, lines: geometry)
    }

    /// The measurer's answer for `identity` at an offer, for both engines.
    private func publish(_ identity: TextIdentity, offer: UInt64, _ p: Paragraph, lines: LineGeometry?) {
        guard publishes, let answers else { return }
        let hash = TextAnswerKey.write(identity.geometry, into: &answerKey)
        guard answers.metrics(hash: hash, key: answerKey, offer: offer) == nil else { return }
        let width = CGFloat(Double(bitPattern: offer))
        answers.put(hash: hash, key: answerKey, offer: offer,
                    metrics: ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline)),
                    lines: lines, glyphs: MeasuredGlyphs(p, spec: identity.geometry, width: width))
    }
    private func publish(_ identity: TextIdentity, offer: UInt64, _ metrics: ExactMetrics, lines: LineGeometry?) {
        guard publishes, let answers else { return }
        let hash = TextAnswerKey.write(identity.geometry, into: &answerKey)
        answers.put(hash: hash, key: answerKey, offer: offer, metrics: metrics, lines: lines)
    }
    /// The line breaks the measurer published for `identity` at `width`.
    private func publishedLines(_ identity: TextIdentity, width: CGFloat) -> LineGeometry? {
        guard let answers else { return nil }
        let hash = TextAnswerKey.write(identity.geometry, into: &answerKey)
        return answers.lines(hash: hash, key: answerKey, width: width)
    }

    package func measuredGlyphs(_ spec: Spec, width: CGFloat) -> MeasuredGlyphs? {
        guard spec.reusesMeasuredGlyphs, let answers else { return nil }
        let hash = TextAnswerKey.write(spec, into: &answerKey)
        return answers.takeGlyphs(hash: hash, key: answerKey, width: width)
    }

    /// Wrap the complete source synchronously. Views/checkpoints keep accepted
    /// widths alive; a new width retires obsolete cache ownership before work.
    package func paragraph(_ spec: Spec, width: CGFloat) -> Paragraph {
        let identity = residency.identity(spec)
        return paragraph(spec, identity: identity, width: width)
    }

    private func paragraph(_ spec: Spec, identity: TextIdentity, width: CGFloat) -> Paragraph {
        let key = TextParagraphKey(shape: TextShapeKey(identity: identity, paint: TextPaint(spec)), width: width)
        if let p = residency.paragraph(key) { return p }
        // Preserve matching measured line breaks while replacing their black
        // CTLines with the real paint attributes. No colored/black width history.
        let breaks = spec.lineClamp > 0 ? nil : residency.geometry(identity, width: width)
        residency.retireWidths(key)
        let shape = shape(key.shape, identity: identity)
        residency.prepare(estimatedBytes: identity.utf16Count * 64)
        let ranges = spec.lineClamp == 0
            ? measuredBreakCache[MeasuredBreakKey(token: identity.token, width: width)]?.ranges
                ?? (painter == nil ? publishedLines(identity, width: width)?.ranges : nil)
                ?? residency.answerLines(identity, width: width)?.0 : nil
        let p = layout(shape, width: width, breaks: breaks, ranges: ranges)
        shape.lastParagraph = p
        if width.isFinite { residency.put(p) }
        return p
    }

    // @ref LLP 1043.000 §3 D4–D7 — only the view retains a flowed width.
    package func paragraph(_ spec: Spec, width: CGFloat, flow: [TextFlowShape]) -> Paragraph {
        guard !flow.isEmpty else { return paragraph(spec, width: width) }
        let identity = residency.identity(spec)
        let source = shape(TextShapeKey(identity: identity, paint: TextPaint(spec)), identity: identity)
        let result = layoutFlow(source, width: width, flow: flow)
        residency.refresh(source)
        return result
    }

    private func shape(_ key: TextShapeKey, identity: TextIdentity) -> TextShape {
        if let cached = residency.shape(key) { return cached }
        residency.prepare(estimatedBytes: identity.ownedBytes + identity.utf16Count * 32)
        let shape = TextShape(key: key, identity: identity, attributed: attributed(key.paint.applying(to: identity)))
        residency.put(shape)
        return shape
    }

    package func layout(_ shape: TextShape, width: CGFloat, breaks: Paragraph? = nil, ranges: [CFRange]? = nil) -> Paragraph {
        let spec = shape.spec, typesetter = shape.typesetter, insets = shape.lineInsets
        let length = shape.identity.utf16Count
        let strut = spec.strut ?? spec.runs.first
        func extents(_ run: Run) -> TextLineBox { TextLineBox(font(run) as CTFont, height: run.lineHeight) }
        let minimum = strut.map(extents) ?? TextLineBox()
        var explicit = false
        func authoredExtents(_ run: Run) -> TextLineBox {
            // Reuse only this layout's exact strut metrics. Font keys preserve
            // signed zero; nonfinite inputs keep their original computation.
            guard let strut, let height = run.lineHeight, let strutHeight = strut.lineHeight,
                  run.size.isFinite, strut.size.isFinite, height.isFinite, strutHeight.isFinite,
                  Double(run.size).bitPattern == Double(strut.size).bitPattern,
                  run.weight == strut.weight, run.family == strut.family, run.italic == strut.italic,
                  Double(height).bitPattern == Double(strutHeight).bitPattern else { return extents(run) }
            return minimum
        }
        // Source spans stay ordered even when CoreText reorders bidi glyph runs.
        // The interned identity owns the UTF-16 boundaries used by every layout.
        let runEnds = shape.identity.runEnds
        let previous = spec.lineClamp == 0 ? shape.lastParagraph : nil
        var lineBottoms: [CGFloat] = []
        var lines: [CTLine] = []
        var glyphCount = 0
        var baselines: [CGFloat] = []
        var maxWidth: CGFloat = 0
        var y: CGFloat = 0
        var start = 0
        var clampedRange: CFRange?
        let limit = width.isFinite ? Double(width) : Double.greatestFiniteMagnitude
        // CSS breaks a line at the last line-break opportunity that fits;
        // `break-word` and `anywhere` break inside a word only when none
        // does (TextBreaks.swift).
        var plan = LineBreakPlan()
        let text = shape.attributed.string as NSString
        // `nowrap` takes no soft break (below), so it needs none of them.
        // Reused breaks still balance: the box measures as the greedy lines.
        if spec.wraps && width.isFinite && ((breaks == nil && ranges == nil) || (spec.balance && spec.lineClamp == 0)) {
            if let cached = shape.lineBreakBoundaries { plan.boundaries = cached }
            else {
                plan.boundaries = lineBoundaries(shape.attributed.string as NSString, length: length)
                shape.lineBreakBoundaries = plan.boundaries
                residency.refresh(shape)
            }
            plan.prepare(text, length: length, emergency: spec.emergencyBreaks)
        }
        func next(_ plan: inout LineBreakPlan, _ start: Int, _ room: Double) -> Int {
            plan.next(typesetter, source: shape.attributed, text: text, start: start, room: room, offerWidth: width,
                      breakWord: spec.overflowWrap != 0, length: length)
        }
        // What a line's trailing spaces add to its width that the box's measure leaves out, in its `room`
        func removed(_ line: CTLine, _ room: CGFloat) -> CGFloat {
            TextEngine.uncountedTrailingWidth(line, whiteSpace: spec.whiteSpace, source: shape.attributed, room: room)
        }
        // CSS `text-wrap-style: balance` (TextBreaks.swift): a narrower room until the first forced break.
        let balanced = spec.balance && spec.lineClamp == 0 && !plan.boundaries.isEmpty && width > 0
            ? plan.balance(limit, length: length, inset: { Double(insets.at($0).width) }, removed: removed, next: next, typesetter: typesetter, source: shape.attributed) : nil
        while start < length {
            if spec.lineClamp > 0 && lines.count == spec.lineClamp { break }
            var count: Int
            if let breaks, lines.count < breaks.lines.count {
                count = CTLineGetStringRange(breaks.lines[lines.count]).length
            } else if let ranges, lines.count < ranges.count {
                count = ranges[lines.count].length
            } else if spec.whiteSpace == 2 {
                // CSS nowrap: no soft wrap opportunity, so the whole source is one line.
                count = length - start
            } else if spec.whiteSpace == 4 {
                // CSS pre: preserved like pre-wrap, so a forced break (the
                // ones CoreText and pre-wrap take) ends a line; nothing else does.
                count = CTTypesetterSuggestLineBreak(typesetter, start, Double.greatestFiniteMagnitude)
            } else {
                // CSS `text-indent` takes its room from the first line only,
                // a list item's indent from each of its lines.
                let room = (balanced.map { start < $0.until ? $0.room : limit } ?? limit) - Double(insets.at(start).width)
                count = next(&plan, start, room)
            }
            if count <= 0 { count = length - start }
            let range = CFRangeMake(start, count)
            let oldLine = previous.flatMap { lines.count < $0.lines.count ? $0.lines[lines.count] : nil }
            var line: CTLine
            // Ordinary CTLines depend on this immutable shape and their exact
            // source range. Width-dependent ellipses never enter this path.
            // A justified one also depends on the width, so it is made again.
            if spec.align != 3, let oldLine, CTLineGetStringRange(oldLine).location == start, CTLineGetStringRange(oldLine).length == count {
                line = oldLine
            } else { line = TextEngine.inkedSoftHyphen(CTTypesetterCreateLine(typesetter, range), source: shape.attributed, range: range) }
            var clamps = false
            // A word too long for its line keeps its width in layout, as CSS
            // measures it; only what is drawn ends in "…".
            var natural: CGFloat?
            if spec.lineClamp > 0 && lines.count + 1 == spec.lineClamp
                && Self.clampsLast(line, start: start, count: count, length: length, width: limit) {
                if start + count == length { natural = CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) }
                line = ellipsizedLine(spec, range: NSRange(location: start, length: count), width: limit) ?? line
                clampedRange = range
                clamps = true
            }
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            // The natural width, the first line's indent in it: justification
            // fills the box, never sizes it.
            let indent = insets.at(start).width
            let drawn = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, &leading))
            let w = max(0, (natural ?? (drawn - removed(line, CGFloat(limit) - indent))) + indent)
            if spec.align == 3, !clamps, width.isFinite {
                line = TextEngine.justified(line, source: shape.attributed, range: range, width: limit - Double(indent))
            }
            // CSS inline boxes share a baseline. Include the paragraph strut
            // and only the runs on this line, preserving each font's half-leading.
            var box = minimum
            for glyphRun in CTLineGetGlyphRuns(line) as! [CTRun] {
                let range = CTRunGetStringRange(glyphRun)
                var first = 0, last = runEnds.count
                while first < last {
                    let middle = first + (last - first) / 2
                    if runEnds[middle] <= range.location { first = middle + 1 }
                    else { last = middle }
                }
                var matched = false, includesNormal = false
                // CoreText can coalesce adjacent spans with the same glyph
                // attributes even when their authored line heights differ.
                for index in first..<spec.runs.count {
                    let offset = index == 0 ? 0 : runEnds[index - 1]
                    if offset >= range.location + range.length { break }
                    let authored = spec.runs[index]
                    matched = true
                    if let b = authored.box {
                        // An image's box, wherever its line breaks (LLP 1011.002).
                        box.include(b.lineBox(font(authored) as CTFont))
                    } else if authored.lineHeight != nil {
                        // Explicit boxes use authored metrics; fallback ink
                        // can overflow without enlarging the inline box.
                        box.include(authoredExtents(authored))
                    } else {
                        includesNormal = true
                    }
                }
                if !matched, let strut, strut.lineHeight != nil {
                    box.include(extents(strut))
                    continue
                }
                if matched && !includesNormal { continue }
                let attributes = CTRunGetAttributes(glyphRun) as NSDictionary
                let shapedFont = attributes[kCTFontAttributeName] as! CTFont
                // Normal line height includes the actual emoji/fallback face's
                // line box, as the browser's does.
                box.include(TextLineBox(shapedFont, height: nil))
            }
            explicit = explicit || box.explicit
            baselines.append(y + box.baseline)
            y += box.height
            lineBottoms.append(y)
            maxWidth = max(maxWidth, w)
            glyphCount += CTLineGetGlyphCount(line)
            lines.append(line)
            start += count
        }
        // An authored CSS line height fixes the line box, including fractions;
        // a `normal` one rounds each font metric to whole pixels (CSSLineBox).
        // A width rounds up to the browser's
        // 1/64 layout unit, so text laid out again at its own measured width
        // still fits on its lines.
        // Balancing moves breaks, never the box: it measures as its greedy lines do.
        maxWidth = max(maxWidth, balanced?.widest ?? 0)
        var height = explicit ? y : ceil(y)
        // CSS `text-box-trim`: the first line cut at its fonts' text, cap or
        // ex top, the last at their baseline or descent, as the measurer and
        // the painter both see it (`Spec.textBoxTrim`).
        trimTextBox(spec, lines: lines, baselines: &baselines, lineBottoms: &lineBottoms, height: &height)
        let paragraph = Paragraph(lines: lines, baselines: baselines, width: CSSLineBox.layoutWidth(maxWidth),
                                  height: height, lineBottoms: lineBottoms,
                                  shape: shape, offeredWidth: width, glyphCount: glyphCount)
        paragraph.clampedRange = clampedRange
        paragraph.insets = insets
        return paragraph
    }

    /// Where a line may end, as UTF16 offsets, the last being `length`
    /// (TextEngine.lineBreaks: Chrome's opportunities, the walker's).
    package func lineBoundaries(_ text: NSString, length: Int) -> [Int] {
        Self.lineBreaks(text, length: length)
    }

    /// An over-wide line truncated at `width` with the ellipsis in the style
    /// of the character it replaces; the browser keeps the first character
    /// when not even the token fits.
    /// The ellipsis's attributes: the text's it replaces, marked as the
    /// token, which stands for what was cut (an inline box's end is not in it).
    private static func truncation(_ attributes: [NSAttributedString.Key: Any]) -> [NSAttributedString.Key: Any] {
        var marked = attributes
        marked[.exactTruncation] = true
        return marked
    }
    package static func ellipsis(_ line: CTLine, range: NSRange, width: Double, source: NSAttributedString) -> CTLine? {
        let at = max(range.location, min(NSMaxRange(range), source.length) - 1)
        let token = CTLineCreateWithAttributedString(NSAttributedString(string: "…", attributes: truncation(source.attributes(at: at, effectiveRange: nil))))
        return CTLineCreateTruncatedLine(line, width, .end, token)
    }

    /// Whether a `line-clamp`'s last line, `line` broken from `start` with
    /// `count` of `length` characters, ends in "…": when text follows it, as
    /// CSS clamps; and when it is wider than `width` though nothing follows (a
    /// word longer than the line), which CSS leaves to overflow but UILabel's
    /// `.byTruncatingTail` truncates, as an author asking for an ellipsis means.
    package static func clampsLast(_ line: CTLine, start: Int, count: Int, length: Int, width: Double) -> Bool {
        start + count < length || (width < Double.greatestFiniteMagnitude
            && CTLineGetTypographicBounds(line, nil, nil, nil) - CTLineGetTrailingWhitespaceWidth(line) > width + 0.5)
    }

    package func ellipsizedLine(_ spec: Spec, range: NSRange, width: Double, source: NSAttributedString? = nil) -> CTLine? {
        Self.clampedLine(source ?? attributed(spec), range: range, width: width)
    }

    /// A `line-clamp`'s last line: its text as it broke, then "…", truncated
    /// to `width`, with the block container's style (CSS Overflow 4 §5.2).
    /// Pure over `source`, so raster workers make the same line as layout.
    package static func clampedLine(_ source: NSAttributedString, range: NSRange, width: Double) -> CTLine? {
        let string = source.string as NSString
        var end = NSMaxRange(range)
        // A wrapped line already fits. Include the ellipsis before asking
        // CoreText to make room for it, removing the line's trailing break/space.
        while end > range.location && [9, 10, 13, 32, 0x2028, 0x2029].contains(Int(string.character(at: end - 1))) { end -= 1 }
        let candidate = NSMutableAttributedString(attributedString: source.attributedSubstring(from: NSRange(location: 0, length: end)))
        let block = source.attribute(.exactClampStyle, at: 0, effectiveRange: nil) as? [NSAttributedString.Key: Any]
            ?? source.attributes(at: 0, effectiveRange: nil)
        candidate.append(NSAttributedString(string: "…", attributes: truncation(block)))
        let typesetter = CTTypesetterCreateWithAttributedString(candidate)
        let line = CTTypesetterCreateLine(typesetter, CFRange(location: range.location, length: end - range.location + 1))
        // Keep paragraph-global indices, including the token, for AppKit hits.
        let token = CTTypesetterCreateLine(typesetter, CFRange(location: end, length: 1))
        // If even the token cannot fit, retain the first clipped character,
        // as the browser does, rather than replacing it with a partial ellipsis.
        return CTLineCreateTruncatedLine(line, width, .end, token)
    }

    /// As narrow as the content can be: the longest unbreakable piece, cut
    /// at the line-break opportunities layout takes (UAX #14), not only at
    /// spaces: a URL's hyphen is one, as in the browser.
    package func minContentWidth(_ spec: Spec) -> CGFloat {
        let identity = residency.identity(spec)
        if let width = residency.minimum(identity) { return width }
        residency.retireWidths(TextParagraphKey(shape: TextShapeKey(identity: identity, paint: TextPaint(spec)), width: .infinity))
        residency.prepare(estimatedBytes: identity.utf16Count * 32)
        var widest: CGFloat = 0
        if !spec.wraps {
            // CSS nowrap and pre have no soft break opportunity: min-content is max-content.
            widest = paragraph(spec, width: .infinity).width
            residency.putMinimum(identity, width: widest)
            return widest
        }
        if spec.overflowWrap == 2 {
            let source = attributed(spec), value = source.string as NSString
            var start = 0
            while start < value.length {
                let range = value.rangeOfComposedCharacterSequence(at: start)
                let line = CTLineCreateWithAttributedString(source.attributedSubstring(from: range))
                widest = max(widest, CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                start = NSMaxRange(range)
            }
            residency.putMinimum(identity, width: CSSLineBox.layoutWidth(widest))
            return CSSLineBox.layoutWidth(widest)
        }
        // Each piece is measured as a line of the paragraph's typesetter, so
        // the paragraph is shaped once. A resident shape is used if there is
        // one; this measure does not make one resident.
        // `text-indent` applies to the first line only (CSS Text 3 §8.1), so
        // only to the first piece. A run's own indent (a list item's) applies to
        // each of its pieces.
        let resident = residency.shape(TextShapeKey(identity: identity, paint: TextPaint(spec)))
        let source = resident?.attributed ?? attributed(spec)
        let typesetter = resident?.typesetter ?? CTTypesetterCreateWithAttributedString(source)
        var indent = spec.textIndent
        var offset = 0
        let emergencyEnds = Set(spec.emergencyBreaks.map { $0 + 1 })
        for r in spec.runs {
            let text = r.text as NSString
            defer { offset += text.length }
            guard !r.hang else { continue }
            let boundaries = lineBoundaries(text, length: text.length).filter { !emergencyEnds.contains(offset + $0) }
            for piece in Self.pieceRanges(text, boundaries: boundaries) {
                let range = CFRange(location: offset + piece.range.location, length: piece.range.length)
                var line = CTTypesetterCreateLine(typesetter, range)
                if piece.hyphenated { line = Self.inkedSoftHyphen(line, source: source, range: range) }
                widest = max(widest, CSSLineBox.layoutWidth(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))) + indent + r.indent)
                indent = 0
            }
        }
        residency.putMinimum(identity, width: widest)
        return widest
    }

    /// The kernel's measurer for one request: called for every paragraph
    /// it lays out. The paragraph wrapped to answer is the one the presenter
    /// paints.
    package var requiresMarkdownExpansion = false
    package var refusedMarkdown: UInt32?
    package func measure(_ request: ExactMeasureRequest) -> ExactMetrics {
        measureCount += 1
        if requiresMarkdownExpansion, request.markup != 0, presentation == nil {
            refusedMarkdown = request.view
            return ExactMetrics(width: 0, height: 0, baseline: -1)
        }
        if let pending = presentation?.measure(self, request) { return pending }
        let lookupStarted = ProcessInfo.processInfo.systemUptime
        let intrinsic = request.width < 0
        let offer = request.width == EXACT_MIN_CONTENT ? TextAnswerKey.minContent
            : intrinsic ? TextAnswerKey.maxContent : TextAnswerKey.offer(width: CGFloat(request.width))
        // What this measurer answered before, whatever its residency still
        // holds: no identity, shape or paragraph is touched for it.
        if publishes, let answers, request.exclusion_count == 0, request.markup == 0 {
            let hash = TextAnswerKey.write(request, into: &answerKey)
            if let metrics = answers.metrics(hash: hash, key: answerKey, offer: offer) {
                measureHits += 1
                measureSeconds += ProcessInfo.processInfo.systemUptime - lookupStarted
                return metrics
            }
        }
        let knownIdentity = residency.borrowedIdentity(request)
        let kind: TextScalarKind = request.width == EXACT_MIN_CONTENT ? .minContent
            : intrinsic ? .maxContent : .definite(Double(request.width == 0 ? 0 : request.width).bitPattern)
        // A measurer whose answers are published keeps none in its residency.
        if request.exclusion_count == 0, let identity = knownIdentity {
            if !publishes, let metrics = residency.scalar(identity, kind: kind) {
                measureHits += 1
                measureSeconds += ProcessInfo.processInfo.systemUptime - lookupStarted
                return metrics
            }
            if !intrinsic, let p = residency.geometry(identity, width: CGFloat(request.width)) {
                keepBreaks(p, identity: identity, width: CGFloat(request.width))
                measureHits += 1
                measureSeconds += ProcessInfo.processInfo.systemUptime - lookupStarted
                return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
            }
        }
        // Preserve measureSeconds as cache/layout work, excluding Run/Spec
        // decoding: a fallback adds its failed borrowed lookup interval below.
        let lookupSeconds = ProcessInfo.processInfo.systemUptime - lookupStarted
        // A new width needs layout, but exact borrowed matching already
        // proved these owned runs and metric fields are the same request.
        let spec = knownIdentity?.geometry ?? requestSpec(request)
        let started = ProcessInfo.processInfo.systemUptime
        if request.exclusion_count > 0, let shapes = request.exclusions {
            let flow = UnsafeBufferPointer(start: shapes, count: request.exclusion_count).map(TextFlowShape.init)
            let width = request.width >= 0 ? CGFloat(request.width) : request.width == EXACT_MIN_CONTENT ? minContentWidth(spec) : paragraph(spec, width: .infinity).width
            let p = paragraph(spec, width: width, flow: flow)
            measureSeconds += lookupSeconds + (ProcessInfo.processInfo.systemUptime - started)
            return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
        }
        let identity = knownIdentity ?? residency.identityAfterBorrowedMiss(spec)
        if knownIdentity == nil, !publishes, let metrics = residency.scalar(identity, kind: kind) {
            measureHits += 1
            measureSeconds += lookupSeconds + (ProcessInfo.processInfo.systemUptime - started)
            return metrics
        }
        let width: CGFloat = request.width == EXACT_MIN_CONTENT ? minContentWidth(spec) : intrinsic ? .infinity : CGFloat(request.width)
        let p: Paragraph
        if (intrinsic || knownIdentity == nil), let cached = residency.geometry(identity, width: width) { measureHits += 1; p = cached }
        else if intrinsic {
            // Intrinsic probes publish only scalar metrics. Their full CTLines
            // leave this scope; shaped source remains subject to the same budget.
            let key = TextParagraphKey(shape: TextShapeKey(identity: identity, paint: TextPaint(spec)), width: width)
            residency.retireWidths(key)
            let shape = shape(key.shape, identity: identity)
            residency.prepare(estimatedBytes: identity.utf16Count * 64)
            p = layout(shape, width: width)
        } else { p = paragraph(spec, identity: identity, width: width) }
        if !intrinsic { keepBreaks(p, identity: identity, width: width) }
        let metrics = ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
        if intrinsic {
            if publishes { publish(identity, offer: offer, metrics, lines: nil) } else { residency.put(identity, kind: kind, metrics: metrics) }
        }
        if spec.emergencyHyphens, publishes, request.markup == 0, let answers {
            // Raw callbacks reuse scalar answers; prepared keys retain their synthetic line offsets.
            let hash = TextAnswerKey.write(request, into: &answerKey)
            answers.put(hash: hash, key: answerKey, offer: offer, metrics: metrics, lines: nil)
        }
        measureSeconds += lookupSeconds + (ProcessInfo.processInfo.systemUptime - started)
        return metrics
    }

    /// The paragraph a request describes, as the presenter paints it.
    package func requestSpec(_ request: ExactMeasureRequest) -> Spec {
        func run(_ run: ExactTextRun) -> Run {
            Run(text: String(decoding: UnsafeBufferPointer(start: run.text, count: run.len), as: UTF8.self), size: CGFloat(run.font_size), weight: Int(run.font_weight), family: Int(run.font_family), italic: run.italic != 0, lineHeight: run.has_line_height != 0 ? CGFloat(run.line_height) : nil, letterSpacing: CGFloat(run.letter_spacing), numeric: Int(run.font_variant_numeric), stretch: CGFloat(run.font_stretch), variations: FontAxis.of(run))
        }
        var runs = UnsafeBufferPointer(start: request.runs, count: request.count).map { r in
            var made = run(r)
            // An image's box draws no ink of its own (LLP 1011.002).
            if let box = InlineBox(r) { made.box = box; made.hidden = true }
            return made
        }
        // Markdown source arrives as one run; the archive expands it the
        // same way the presenter paints it (LLP 1045 D3).
        if request.markup != 0, let source = runs.first {
            let utf8 = { (p: UnsafePointer<UInt8>?, n: Int) in p.map { String(decoding: UnsafeBufferPointer(start: $0, count: n), as: UTF8.self) } ?? "" }
            if let expanded = presentation?.markdown(self, source: source, atoms: utf8(request.atoms, request.atoms_len), faces: utf8(request.faces, request.faces_len)) { runs = expanded }
            else if requiresMarkdownExpansion { refusedMarkdown = request.view; runs = [] }
        }
        // Metric-only keys match the geometry used by the colored presenter.
        var made = Spec(runs: runs, align: Int(request.align), lineClamp: Int(request.line_clamp), color: [0, 0, 0, 255], overflowWrap: Int(request.overflow_wrap), direction: Int(request.direction), whiteSpace: Int(request.white_space), strut: run(request.strut))
        made.textIndent = CGFloat(request.text_indent); made.hyphens = Int(request.hyphens)
        made.textBoxTrim = Int(request.text_box_trim); made.textBoxOver = Int(request.text_box_over); made.textBoxUnder = Int(request.text_box_under)
        made.balance = request.text_wrap_style == 1
        if made.automaticHyphens, let lang = request.lang {
            made.language = String(decoding: UnsafeBufferPointer(start: lang, count: request.lang_len), as: UTF8.self)
        }
        made.hyphenateAuto(allowEmergency: request.exclusion_count == 0)
        return made
    }

    /// The C ABI's synchronous font seam, invoked before the kernel asks its
    /// first text measurement; `ctx` is the session's engine.
    /// `ctx` is the measurer: its painter installs the same catalog on
    /// main while the owner waits (the catalog lives for this call).
    package static let installFonts: ExactFontsFn = { ctx, catalog in
        guard let ctx else { return }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        engine.install(catalog)
        if let painter = engine.painter { Owner.shared.callMain { painter.install(catalog) } }
    }

    /// The kernel's text measurer; `ctx` is the session's engine.
    package static let measureText: ExactMeasureFn = { ctx, request in
        guard let ctx, let request = request?.pointee else { return ExactMetrics(width: 0, height: 0, baseline: -1) }
        return Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue().measure(request)
    }
}

