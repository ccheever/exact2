// Text: CoreText, one engine for measuring and painting (LLP 1008 §3; the
// lesson of exact1's LLP 0418/0430 — TextKit is ~5× slower per wrap, and
// measuring with one engine while painting with another is a correctness
// tax). A Paragraph is a width-specific snapshot: the wrapped CTLines, their
// baselines, and the size — the object that answers the kernel's measure
// and the object `draw` paints, cached by (spec, width). Fonts are cached.
//
// One `TextEngine` per session (LLP 1031 D12): the plan's catalog (stack id
// → faces) and both caches are the session's, since two plans number their
// stacks independently and a second session's install must not wipe the
// first's. What is process-wide by platform — CoreText's file registration
// — is `FontRegistry`, registered once per URL and never unregistered.
// Shared by the AppKit and UIKit presenters: the two differ only in the
// font and color classes, and the same CoreText answers the kernel on both.
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
package struct Run: Hashable {
    package var text: String
    package var size: CGFloat
    package var weight: Int
    package var family: Int
    package var italic: Bool
    package var lineHeight: CGFloat?
    var letterSpacing: CGFloat
    /// CSS `font-variant-numeric` bits: 1 is `tabular-nums`, the face's own
    /// `tnum` feature. It changes advances: a metric (LLP 1053 G4).
    var numeric: Int = 0
    package var color: [Double]? = nil
    package var decoration: String = ""
    package var href: String = ""
    /// The inline box's `background-color`: paint, never metrics.
    var background: [Double]? = nil
    /// CSS visibility hides ink without changing shaping or descendant visibility.
    var hidden = false
    /// Its own `text-shadow` (offset x, y, blur, r g b a) when the
    /// paragraph's runs differ (`Spec.gatherShadows`), and its
    /// `-webkit-text-stroke` (width, r g b a): paint (LLP 1077 D3, D7).
    var shadow: [Double]? = nil
    var stroke: [Double]? = nil
    /// A Markdown list item's head indent and hung marker (LLP 1045 D4,
    /// `LineInsets`): where its paragraph's lines start, so a metric.
    package var indent: CGFloat = 0
    package var hang = false

    package static func == (lhs: Run, rhs: Run) -> Bool {
        guard lhs.size == rhs.size, lhs.weight == rhs.weight, lhs.family == rhs.family,
              lhs.italic == rhs.italic, lhs.lineHeight == rhs.lineHeight,
              lhs.letterSpacing == rhs.letterSpacing, lhs.numeric == rhs.numeric, lhs.color == rhs.color,
              lhs.decoration == rhs.decoration, lhs.href == rhs.href,
              lhs.hidden == rhs.hidden, lhs.background == rhs.background, lhs.shadow == rhs.shadow, lhs.stroke == rhs.stroke,
              lhs.indent == rhs.indent, lhs.hang == rhs.hang else { return false }
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
        hasher.combine(color)
        hasher.combine(decoration)
        hasher.combine(href)
        hasher.combine(background)
        hasher.combine(hidden)
        hasher.combine(shadow)
        hasher.combine(stroke)
        hasher.combine(indent)
        hasher.combine(hang)
    }
}

/// A paragraph's specification: runs plus paragraph style.
struct Spec: Hashable {
    var runs: [Run]
    var align: Int // 0 left, 1 center, 2 right, 3 justify
    var lineClamp: Int
    var color: [Double] // r g b a, 0–255
    var overflowWrap: Int = 0 // CSS: normal, break-word, anywhere
    var direction: Int = 0 // CSS: ltr, rtl
    var whiteSpace: Int = 0 // CSS: normal, pre-wrap, nowrap, pre-line, pre (runs already collapsed unless pre-wrap or pre)
    /// `white-space-collapse: preserve` (pre-wrap, pre): spaces, tabs and breaks are text.
    var preserves: Bool { whiteSpace == 1 || whiteSpace == 4 }
    /// `text-wrap-mode: wrap`: soft wrap opportunities may end a line (not nowrap, not pre).
    var wraps: Bool { whiteSpace != 2 && whiteSpace != 4 }
    var strut: Run? = nil // paragraph minimum line box, including smaller inline runs
    /// CSS `text-overflow: ellipsis` in a clipping box: paint ends an
    /// over-wide line in "…"; never metrics (LLP 1053 G5).
    var ellipsis = false
    /// Collapsed → source offsets for the runs above (LLP 1053 G5).
    var source = SourceMap()
    /// CSS `text-indent` in points, the paragraph's first line only.
    var textIndent: CGFloat = 0
    /// CSS `hyphens`: 0 manual (the initial value), 1 none (soft hyphens
    /// already arrive as U+034F), 2 auto.
    var hyphens = 0
    /// The document language whose hyphenation points `auto` takes; empty
    /// for `none` and `manual`, which need none.
    var language = ""
    /// CSS `text-shadow` (LLP 1077 D3) shared by every run: offset x, y and
    /// blur in points, then the colour's r g b a (0–255), resolved for the
    /// appearance. Nil when the runs' own differ (`gatherShadows`).
    var shadow: [Double]? = nil
}

/// Where collapsed white space went, from `exact_text_collapse`: offsets
/// into the shaped (collapsed) text map to the node's source text. Carried
/// beside a Spec but never part of its identity: equal shaped text is equal.
struct SourceMap: Hashable {
    var edits: [ExactCollapseEdit] = []
    /// Where `hyphens: auto` put a soft hyphen, as shaped UTF-16 offsets
    /// (`Spec.hyphenateAuto`): text the source does not have.
    var inserted: [Int] = []
    static func == (_: SourceMap, _: SourceMap) -> Bool { true }
    func hash(into hasher: inout Hasher) {}
    /// A shaped UTF-16 offset's source offset.
    func source(_ shaped: Int) -> Int {
        var lo = 0, hi = inserted.count
        while lo < hi { let mid = (lo + hi) / 2; if inserted[mid] < shaped { lo = mid + 1 } else { hi = mid } }
        return beforeInsertion(shaped - lo)
    }
    /// A source UTF-16 offset's shaped offset; removed text maps to where it
    /// was, and an offset after an inserted soft hyphen stays after it.
    func collapsed(_ source: Int) -> Int {
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
    static func collapse(_ runs: inout [Run], whiteSpace: Int) -> SourceMap {
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
}

/// A face's CSS line box. macOS follows Chrome, the parity oracle (the web
/// host's smoke and `browser_cases.rs` run in it), on the same faces: Blink
/// keeps ascent, descent and line gap as whole pixels, rounded to the
/// nearest; `line-height: normal` puts the gap's floor half above and the
/// rest below, and a set height centres the content area with the baseline
/// floored. Chrome 154 on macOS 26.6 at device scale 1 (the web smoke's),
/// captured with getBoundingClientRect for SF 10-40 px and the fallback
/// faces; TextParityMacTests pins it.
///
/// iOS keeps CoreText's unrounded metrics, the paragraph's height rounded up
/// once. Chrome's rule cannot be applied there: iOS's SF has other vertical
/// metrics (15.23 + 3.86 at 16 px against the Mac's 15.47 + 3.38), so
/// rounding each gives 16 at 14 px where Chrome has 17, and the scroll
/// fixture's cross-host 652 (smoke.mjs) would move. See QUEUE.
enum CSSLineBox {
    /// A text advance as the browser's layout holds it: rounded up to its
    /// 1/64 px layout unit, never to a whole point (a chip's width).
    static func layoutWidth(_ width: CGFloat) -> CGFloat { ceil(width * 64) / 64 }

    /// The content area (an inline background's extent) above and below the baseline.
    static func content(_ font: CTFont) -> (CGFloat, CGFloat) {
        #if os(macOS)
        (CTFontGetAscent(font).rounded(), CTFontGetDescent(font).rounded())
        #else
        (CTFontGetAscent(font), CTFontGetDescent(font))
        #endif
    }

    /// Above and below the baseline: `line-height: normal` when `height` is
    /// nil, else the authored line height.
    static func extents(_ font: CTFont, height: CGFloat?) -> (CGFloat, CGFloat) {
        #if os(macOS)
        let (ascent, descent) = content(font)
        guard let height else {
            let gap = CTFontGetLeading(font).rounded()
            return (ascent + floor(gap / 2), descent + ceil(gap / 2))
        }
        guard height.isFinite else { return (ascent, descent) }
        let above = ascent + floor((height - ascent - descent) / 2)
        return (above, height - above)
        #else
        let ascent = CTFontGetAscent(font), below = CTFontGetDescent(font) + CTFontGetLeading(font)
        let half = ((height ?? ascent + below) - ascent - below) / 2
        return (ascent + half, below + half)
        #endif
    }
}

/// Where a measured paragraph's lines break, as plain values: what
/// measurement publishes and painting shapes its own lines from (LLP 1072
/// §8.1). `clamped`: a `line-clamp`'s last line, made again ending in "…"
/// (`TextEngine.clampedLine`).
struct LineGeometry {
    let ranges: [CFRange]
    let baselines: [CGFloat]
    var clamped: CFRange? = nil
    init(ranges: [CFRange], baselines: [CGFloat], clamped: CFRange? = nil) {
        self.ranges = ranges; self.baselines = baselines; self.clamped = clamped
    }
    /// A clamped last line's own range is the ellipsized line's; the
    /// geometry keeps the range it broke at.
    init(_ p: Paragraph) {
        var ranges = p.lines.map { CTLineGetStringRange($0) }
        if let clamped = p.clampedRange, !ranges.isEmpty { ranges[ranges.count - 1] = clamped }
        self.init(ranges: ranges, baselines: p.baselines, clamped: p.clampedRange)
    }
}

/// A wrapped paragraph at one width: what is measured is what is painted.
final class Paragraph {
    let lines: [CTLine]
    /// Baseline of each line, measured from the top.
    let baselines: [CGFloat]
    /// Flow origins already include interval alignment; empty for ordinary text.
    let origins: [CGFloat]
    /// Where each line starts: `text-indent`, a list item's head indent.
    var insets = LineInsets()
    /// Logical source ownership, including trimmed whitespace, one per fragment.
    let fragments: [ExactFlowFragment]
    var flowIncomplete = false
    /// A `line-clamp` paragraph whose last line ends in "…": that line's
    /// range as it broke, before the ellipsis (`TextEngine.clampedLine`).
    var clampedRange: CFRange?
    let flowLineHeight: CGFloat
    let width: CGFloat
    let height: CGFloat
    let lineBottoms: [CGFloat]
    let shape: TextShape?
    let residencyKey: TextParagraphKey?
    let coreTextEstimateBytes: Int
    var ownedPayloadBytes: Int {
        lines.count * MemoryLayout<CTLine>.stride
            + (baselines.count + lineBottoms.count + origins.count) * MemoryLayout<CGFloat>.stride
            + fragments.count * MemoryLayout<ExactFlowFragment>.stride
            + (cachedInk?.storageBytes ?? 0)
    }
    /// Admission reserves the known lazy array shape, without constructing ink.
    /// Diagnostics still report only the payload actually allocated above.
    var admissionPayloadBytes: Int {
        ownedPayloadBytes - (cachedInk?.storageBytes ?? 0)
            + ParagraphInkIndex.storageBytes(lineCount: lines.count)
    }
    private(set) var cachedInk: ParagraphInkIndex?
    var firstBaseline: CGFloat { baselines.first ?? 0 }
    private var truncated: (width: CGFloat, lines: [Int: CTLine])?

    /// CSS `text-overflow: ellipsis`: a line wider than the box ends in "…",
    /// as painted; the measured line (and every metric) is unchanged.
    func ellipsized(_ index: Int, spec: Spec, width: CGFloat) -> CTLine {
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
    init(lines: [CTLine], baselines: [CGFloat], width: CGFloat, height: CGFloat, lineBottoms: [CGFloat] = [],
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
    func inkBounds() -> ParagraphInkIndex {
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
final class ParagraphInkIndex {
    private struct Span {
        var top: CGFloat = .infinity
        var bottom: CGFloat = -.infinity
    }
    private let spans: [Span]
    private let leaves: Int
    private let count: Int
    /// Array payload only: excludes the object/array headers and allocator slack.
    var storageBytes: Int { spans.count * MemoryLayout<Span>.stride }

    private static func leafCount(_ count: Int) -> Int {
        var size = 1
        while size < count { size *= 2 }
        return size
    }
    static func storageBytes(lineCount: Int) -> Int {
        leafCount(lineCount) * 2 * MemoryLayout<Span>.stride
    }
    init(lines: [CTLine], baselines: [CGFloat]) {
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

    func forEachLine(from top: CGFloat, through bottom: CGFloat, _ body: (Int) -> Void) {
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

extension Spec {
    /// Paint does not affect wrapping. Retain run boundaries and metric styles.
    var geometry: Spec {
        var value = self
        value.color = [0, 0, 0, 255]
        value.ellipsis = false
        value.source = SourceMap()
        for i in value.runs.indices {
            value.runs[i].hidden = false
            value.runs[i].color = nil
            value.runs[i].decoration = ""
            value.runs[i].href = ""
            value.runs[i].background = nil
            value.runs[i].shadow = nil
            value.runs[i].stroke = nil
        }
        if var strut = value.strut {
            strut.text = ""; strut.color = nil; strut.decoration = ""; strut.href = ""; strut.background = nil
            strut.shadow = nil; strut.stroke = nil
            value.strut = strut
        }
        return value
    }
}

/// CoreText's process-wide registration, once per URL, never undone.
enum FontRegistry {
    nonisolated(unsafe) private static var registered: Set<URL> = []
    /// The painter and the measurer each register the catalog they install.
    private static let lock = NSLock()

    static func register(_ url: URL) -> Bool {
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
/// (`TextAnswers`): each measured paragraph's metrics and line breaks.
package final class TextEngine {
    #if os(macOS)
    var readerParagraphs: [UInt32: RegionReaderParagraph] = [:] {
        didSet { readerLock.lock(); readerViews = Set(readerParagraphs.keys); readerLock.unlock() }
    }
    private let readerLock = NSLock()
    private var readerViews = Set<UInt32>()
    /// Whether a reader paragraph is held for `view`: readable off main.
    func readerHolds(_ view: UInt32) -> Bool {
        readerLock.lock(); defer { readerLock.unlock() }
        return readerViews.contains(view)
    }
    #endif
    /// The engine the kernel measures with, on the owner thread; nil in the
    /// measurer itself.
    private(set) var measurer: TextEngine?
    /// The painting engine a measurer belongs to.
    private(set) weak var painter: TextEngine?
    /// Where the measurer publishes what it measured: it answers from there
    /// before typesetting, and the painter takes its line breaks there.
    private(set) var answers: TextAnswers?
    /// This engine's key for a lookup there (`TextAnswerKey`), reused.
    private var answerKey: [UInt8] = []
    /// Whether this engine is a measurer whose answers are published: its
    /// residency then keeps shaped text only, the table the answers.
    private var publishes: Bool { painter != nil && (answers?.limit ?? 0) > 0 }
    var fonts: [String: PlatformFont] = [:]
    private var residency: TextResidency
    var residencyStats: TextResidencyStats { residency.stats }
    /// The NodeView's existing cachedTextLayout is the accepted lease. The
    /// cache keeps only a weak lookup once that view owns the paragraph.
    func accepted(_ paragraph: Paragraph) { residency.accepted(paragraph) }
    /// Under memory pressure, shaped text no view holds is dropped, as the
    /// raster loader drops its cold images.
    func dropCold() { residency.dropCold() }
    /// At rest (`ExactSession.rest`): shaped text no view holds goes.
    func dropColdShaped() { residency.dropColdShaped() }
    /// A screen shows about `visibleParagraphs` paragraphs: cold shaped
    /// text is held to two screens of them (`TextResidency.fitShaped`).
    func fitShaped(visibleParagraphs: Int) { residency.fitShaped(visibleParagraphs: visibleParagraphs) }
    var catalog: [Int: [RegisteredFace]] = [:]
    var fieldChrome: FieldChromeCache?
    #if os(iOS) || os(tvOS)
    var buttonMeasurements: ButtonMeasureCache?
    #endif
    var platformControlID: UInt16?
    var platformControlFont: PlatformFont?
    var platformControlName = NSData()
    /// Declared family names to their plan stacks, for Canvas 2D's `font`
    /// (LLP 1056 D8).
    private var familyStacks: [String: Int] = [:]
    /// Canvas 2D's fonts and lines over this engine (LLP 1056 D8).
    private(set) lazy var canvasText = CanvasText(engine: self)

    /// A declared family's stack, by name.
    func stack(named name: String) -> Int? { familyStacks[name] }
    /// Where a declared face's relative source resolves: the app's resolver
    /// (LLP 1031 D1 — the committed complete generation, else the root).
    let resolve: (String) -> URL?
    let read: (String) -> Data?
    /// A face's file in the signed app bundle, when the generation is the
    /// embedded one (LLP 1019 D5: the descriptor from the sandboxed URL).
    /// CoreText maps it, so its pages stay clean; a delivered generation has
    /// none and binds the verified bytes instead.
    let bundled: (String) -> URL?
    private var pendingFonts: [URL] = []
    /// Native callback entries, native cache hits, and native cache/layout time
    /// since session start. Rust identified-metric hits bypass this callback;
    /// the timer below excludes C-run decoding and Swift String construction.
    var measureCount = 0
    var measureHits = 0
    var measureSeconds = 0.0

    init(resolve: @escaping (String) -> URL?, read: ((String) -> Data?)? = nil,
         bundled: @escaping (String) -> URL? = { _ in nil },
         coldTextTargetBytes: Int = TextResidency.defaultSoftTargetBytes) {
        residency = TextResidency(softTargetBytes: coldTextTargetBytes)
        self.resolve = resolve
        self.bundled = bundled
        self.read = read ?? { name in resolve(name).flatMap { try? Data(contentsOf: $0) } }
    }

    /// A painting engine with its measurer.
    static func pair(resolve: @escaping (String) -> URL?, read: ((String) -> Data?)? = nil,
                     bundled: @escaping (String) -> URL? = { _ in nil },
                     answerBytes: Int = TextAnswers.defaultLimit) -> TextEngine {
        let painter = TextEngine(resolve: resolve, read: read, bundled: bundled)
        let measurer = TextEngine(resolve: resolve, read: read, bundled: bundled)
        let answers = TextAnswers(limit: answerBytes)
        painter.measurer = measurer; painter.answers = answers
        measurer.painter = painter; measurer.answers = answers
        measurer.residency.keepsAnswers = !measurer.publishes
        return painter
    }
    /// The context the kernel's measure and font hooks get: the measurer's.
    var measuring: TextEngine { measurer ?? self }

    func commitFonts() {
        for url in pendingFonts { _ = FontRegistry.register(url) }
        pendingFonts = []
        if let m = measurer { Owner.shared.sync { m.commitFonts() } }
    }

    /// This engine as the context the C callbacks hand back.
    var opaque: UnsafeMutableRawPointer { Unmanaged.passUnretained(self).toOpaque() }

    /// The live plan's exact text state while a reload candidate boots.
    /// Dictionary copies retain the already-shaped paragraphs and fonts;
    /// candidate `removeAll` calls detach through copy-on-write.
    final class Checkpoint {
        private let pendingFonts: [URL]
        private let fonts: [String: PlatformFont]
        private let residency: TextResidency
        private let catalog: [Int: [RegisteredFace]]
        private let familyStacks: [String: Int]
        private let controlID: UInt16?
        private let controlFont: PlatformFont?
        private let controlName: NSData
        private let measurer: Checkpoint?

        fileprivate init(_ engine: TextEngine) {
            // The measurer's state is the owner's (LLP 1072 §8.1).
            measurer = engine.measurer.map { m in Owner.shared.sync { Checkpoint(m) } }
            controlID = engine.platformControlID; controlFont = engine.platformControlFont; controlName = engine.platformControlName
            pendingFonts = engine.pendingFonts
            fonts = engine.fonts
            residency = engine.residency
            catalog = engine.catalog
            familyStacks = engine.familyStacks
        }

        fileprivate func restore(into engine: TextEngine) {
            engine.platformControlID = controlID; engine.platformControlFont = controlFont; engine.platformControlName = controlName
            engine.pendingFonts = pendingFonts
            engine.fonts = fonts
            engine.residency = residency
            engine.residency.refreshAfterRestore()
            engine.catalog = catalog
            engine.familyStacks = familyStacks
            engine.canvasText = CanvasText(engine: engine)
            engine.dropMeasuredBreaks()
            if let measurer, let m = engine.measurer { Owner.shared.sync { measurer.restore(into: m) } }
        }
    }

    func checkpoint() -> Checkpoint { Checkpoint(self) }
    func restore(_ checkpoint: Checkpoint) { checkpoint.restore(into: self) }

    /// Replace this session's plan-scoped catalog before layout. A fresh
    /// residency namespace prevents old accepted font identities from aliasing
    /// the candidate; checkpoints retain and restore their original namespace.
    func install(_ pointer: UnsafePointer<ExactFontCatalog>?) {
        #if os(macOS)
        readerParagraphs.removeAll()
        #endif
        fonts.removeAll(keepingCapacity: true)
        residency = TextResidency(softTargetBytes: residency.softTargetBytes)
        residency.keepsAnswers = !publishes
        dropMeasuredBreaks()
        platformControlID = nil
        catalog.removeAll(keepingCapacity: true)
        familyStacks.removeAll()
        canvasText = CanvasText(engine: self)
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
                let generic = row.weight == 1 ? generics.firstIndex(of: name) : nil
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
            staged[stack, default: []].append(RegisteredFace(family: name, generic: nil, weight: Int(row.weight),
                italic: row.italic != 0, descriptor: Self.declaredFace(descriptor, weight: Int(row.weight))))
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
        font(size: run.size, weight: run.weight, family: run.family, italic: run.italic, numeric: run.numeric)
    }

    /// CSS `tabular-nums` is the chosen face's own OpenType `tnum` feature,
    /// never a substitute monospaced face; a face without it is unchanged.
    package func font(size: CGFloat, weight: Int, family: Int, italic: Bool, numeric: Int) -> PlatformFont {
        let base = font(size: size, weight: weight, family: family, italic: italic)
        guard numeric & 1 != 0 else { return base }
        let key = "\(family)/\(size)/\(weight)/\(italic)/tnum"
        if let f = fonts[key] { return f }
        let settings = [[kCTFontOpenTypeFeatureTag: "tnum", kCTFontOpenTypeFeatureValue: 1]] as CFArray
        let descriptor = CTFontDescriptorCreateWithAttributes([kCTFontFeatureSettingsAttribute: settings] as CFDictionary)
        let f = CTFontCreateCopyWithAttributes(base as CTFont, size, nil, descriptor) as PlatformFont
        fonts[key] = f
        return f
    }

    package func font(size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont {
        if let f = controlFont(size: size, weight: weight, family: family, italic: italic) { return f }
        let key = "\(family)/\(size)/\(weight)/\(italic)"
        if let f = fonts[key] { return f }
        if let faces = catalog[family], !faces.isEmpty {
            let face = TextEngine.matched(faces, weight: weight, italic: italic)
            func candidate(_ face: RegisteredFace) -> PlatformFont {
                if let generic = face.generic { return font(size: size, weight: weight, family: generic, italic: italic) }
                return CTFontCreateWithFontDescriptor(face.descriptor, size, nil) as PlatformFont
            }
            let base = candidate(face)
            var seen = Set<String>(), cascade: [CTFontDescriptor] = []
            for member in faces where seen.insert(member.family).inserted {
                let group = faces.filter { $0.family == member.family }
                cascade.append(CTFontCopyFontDescriptor(candidate(TextEngine.matched(group, weight: weight, italic: italic)) as CTFont))
            }
            let descriptor = CTFontDescriptorCreateWithAttributes([kCTFontCascadeListAttribute: cascade] as CFDictionary)
            let f = CTFontCreateCopyWithAttributes(base as CTFont, size, nil, descriptor) as PlatformFont
            fonts[key] = f
            return f
        }
        let w: PlatformFont.Weight
        switch weight {
        case ..<200: w = .ultraLight
        case 200..<300: w = .thin
        case 300..<400: w = .light
        case 400..<500: w = .regular
        case 500..<600: w = .medium
        case 600..<700: w = .semibold
        case 700..<800: w = .bold
        case 800..<900: w = .heavy
        default: w = .black
        }
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
        // A declared stack returned above with one of its real descriptors.
        // This trait resolver is only for a platform generic, never a shear
        // applied to custom bytes (LLP 1019 §5).
        if italic {
            #if canImport(UIKit)
            if let d = f.fontDescriptor.withSymbolicTraits(.traitItalic) { f = UIFont(descriptor: d, size: size) }
            #else
            // AppKit's font manager is main's (LLP 1072 §8.1): a variant first
            // met while the owner thread measures is resolved there, once.
            let upright = f
            f = Owner.shared.callMain { NSFontManager.shared.convert(upright, toHaveTrait: .italicFontMask) }
            #endif
        }
        f = TextEngine.cssWeight(f, weight: weight, size: size)
        fonts[key] = f
        return f
    }

    /// CSS `font-weight` on a variable system face is its `wght` axis at
    /// that number, as Chrome and WebKit set it: UIKit's semibold is `wght`
    /// 590 and its medium 510, so `600` text measured 0.2% narrower than
    /// Chrome's ("Heavy list" at 17 px: 77.67 against 77.87).
    static func cssWeight(_ font: PlatformFont, weight: Int, size: CGFloat) -> PlatformFont {
        let tag = 0x77676874 as NSNumber // 'wght'
        guard let axes = CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber], let current = axes[tag],
              current.intValue != weight else { return font }
        let d = CTFontDescriptorCreateWithAttributes([kCTFontVariationAttribute: [tag: weight as NSNumber]] as CFDictionary)
        return CTFontCreateCopyWithAttributes(font as CTFont, size, nil, d) as PlatformFont
    }

    /// A color from the style dictionary's `[r,g,b,a]` bytes (sRGB), or
    /// `textChannels`' nine, in the colour's own space (LLP 1100 D2).
    package static func color(_ c: [Double]) -> PlatformColor {
        if c.count == 9, (0...2).contains(Int(c[4])), case let name = [CGColorSpace.extendedSRGB, CGColorSpace.extendedDisplayP3, CGColorSpace.extendedLinearSRGB][Int(c[4])],
           let space = CGColorSpace(name: name), let made = CGColor(colorSpace: space, components: c[5...].map { CGFloat($0) }),
           let color = PlatformColor(cgColor: ColorRange.tagged(made)) as PlatformColor? { return color }
        #if canImport(UIKit)
        return UIColor(red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
        #else
        return NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
        #endif
    }

    func attributed(_ spec: Spec) -> NSAttributedString {
        // The measure callback sees dense paragraphs as many small runs. Build
        // their text once, then assign UTF-16 spans, instead of allocating and
        // appending an attributed string for every run.
        let s = NSMutableAttributedString(string: spec.runs.map(\.text).joined())
        let color = TextEngine.color(spec.color)
        var offset = 0
        for r in spec.runs {
            var a: [NSAttributedString.Key: Any] = [.font: font(r), .foregroundColor: r.color.map(TextEngine.color) ?? color]
            if r.hidden { a[.exactHidden] = true }
            // A centred stroke over the fill: Core Text's negative width,
            // in percent of the run's size (LLP 1077 D7). Each run's own.
            if let st = r.stroke, st.count == 5, st[0] > 0, r.size > 0 {
                a[.strokeWidth] = -st[0] / Double(r.size) * 100
                a[.strokeColor] = TextEngine.color(Array(st[1...]))
            }
            if let sh = r.shadow, TextEngine.isShadow(sh) { a[.exactShadow] = TextRunShadow(sh) }
            if r.letterSpacing != 0 { a[.kern] = r.letterSpacing }
            if r.decoration.contains("underline") || (r.decoration.isEmpty && !r.href.isEmpty) { a[.underlineStyle] = NSUnderlineStyle.single.rawValue }
            if r.decoration.contains("line-through") { a[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
            if !r.hidden, let fill = r.background.map(TextEngine.color), fill.cgColor.alpha > 0 {
                let f = a[.font] as! PlatformFont
                let (ascent, descent) = CSSLineBox.content(f as CTFont)
                a[.exactBackground] = InlineBackground(color: fill.cgColor, ascent: ascent, descent: descent)
            }
            let length = r.text.utf16.count
            if length > 0 { s.setAttributes(a, range: NSRange(location: offset, length: length)) }
            offset += length
        }
        TextEngine.setBaseDirection(s, direction: spec.direction)
        if spec.preserves, s.string.contains("\t"), let first = spec.strut ?? spec.runs.first {
            setTabStops(s, run: first)
        }
        return s
    }

    /// Eight spaces of `font`, letter spacing included: a tab stop's interval.
    static func tabInterval(_ font: CTFont, letterSpacing: CGFloat) -> CGFloat {
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

    static func setTabStops(_ s: NSMutableAttributedString, interval: CGFloat) {
        guard interval > 0, s.length > 0 else { return }
        let range = NSRange(location: 0, length: s.length)
        let paragraph = ((s.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle) ?? NSMutableParagraphStyle()
        paragraph.tabStops = []
        paragraph.defaultTabInterval = interval
        s.addAttribute(.paragraphStyle, value: paragraph, range: range)
    }

    /// CSS `direction: rtl` as the paragraph's base writing direction (LLP
    /// 1053). Under `ltr` CoreText's natural direction (the first strong
    /// character) stays, as on Linux; LLP 1001 §1 declares it.
    static func setBaseDirection(_ s: NSMutableAttributedString, direction: Int) {
        guard direction == 1, s.length > 0 else { return }
        let paragraph = NSMutableParagraphStyle()
        paragraph.baseWritingDirection = .rightToLeft
        s.addAttribute(.paragraphStyle, value: paragraph, range: NSRange(location: 0, length: s.length))
    }

    /// Urgent raster work stays on the text engine's owning thread and uses
    /// the existing bounded residency policy for its exact painted typesetter.
    func rasterLines(_ spec: Spec, ranges: [CFRange], width: CGFloat = .infinity) -> (NSAttributedString, [CTLine]) {
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
    func measuredBreaks(_ spec: Spec, width: CGFloat) -> LineGeometry? {
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
    private(set) var namespace = 0

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
        publish(identity, offer: offer, ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline)), lines: lines)
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

    /// Wrap the complete source synchronously. Views/checkpoints keep accepted
    /// widths alive; a new width retires obsolete cache ownership before work.
    func paragraph(_ spec: Spec, width: CGFloat) -> Paragraph {
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
    func paragraph(_ spec: Spec, width: CGFloat, flow: [TextFlowShape]) -> Paragraph {
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

    func layout(_ shape: TextShape, width: CGFloat, breaks: Paragraph? = nil, ranges: [CFRange]? = nil) -> Paragraph {
        let spec = shape.spec, typesetter = shape.typesetter, insets = shape.lineInsets
        let length = shape.identity.utf16Count
        let strut = spec.strut ?? spec.runs.first
        func extents(_ run: Run) -> (CGFloat, CGFloat) { CSSLineBox.extents(font(run) as CTFont, height: run.lineHeight) }
        let minimum = strut.map(extents) ?? (0, 0)
        var explicit = false
        func authoredExtents(_ run: Run) -> (CGFloat, CGFloat) {
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
        if spec.wraps && width.isFinite && breaks == nil && ranges == nil {
            if let cached = shape.lineBreakBoundaries { plan.boundaries = cached }
            else {
                plan.boundaries = lineBoundaries(shape.attributed.string as NSString, length: length)
                shape.lineBreakBoundaries = plan.boundaries
                residency.refresh(shape)
            }
            plan.prepare(text, length: length)
        }
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
                let room = limit - Double(insets.at(start).width)
                plan.advance(to: start)
                func suggest(_ room: Double) -> Int {
                    plan.suggest(typesetter, start: start, room: room, offerWidth: width, breakWord: spec.overflowWrap != 0, length: length, text: text)
                }
                count = suggest(room)
                // A soft hyphen's break shows one, which must fit (TextBreaks).
                count = Self.fitSoftHyphen(start: start, count: count, room: room, boundaries: plan.boundaries, source: shape.attributed,
                                           typesetter: typesetter, length: length, retry: suggest)
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
            if spec.lineClamp > 0 && lines.count + 1 == spec.lineClamp && start + count < length {
                line = ellipsizedLine(spec, range: NSRange(location: start, length: count), width: limit) ?? line
                clampedRange = range
                clamps = true
            }
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            // The natural width, the first line's indent in it: justification
            // fills the box, never sizes it.
            let indent = insets.at(start).width
            let w = max(0, CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, &leading)) + indent)
            if spec.align == 3, !clamps, width.isFinite {
                line = TextEngine.justified(line, source: shape.attributed, range: range, width: limit - Double(indent))
            }
            // CSS inline boxes share a baseline. Include the paragraph strut
            // and only the runs on this line, preserving each font's half-leading.
            var above = minimum.0, below = minimum.1
            var aboveExplicit = strut?.lineHeight != nil, belowExplicit = aboveExplicit
            func include(_ a: CGFloat, _ b: CGFloat, explicit: Bool) {
                if a > above { above = a; aboveExplicit = explicit }
                else if a == above { aboveExplicit = aboveExplicit && explicit }
                if b > below { below = b; belowExplicit = explicit }
                else if b == below { belowExplicit = belowExplicit && explicit }
            }
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
                    if authored.lineHeight != nil {
                        // Explicit boxes use authored metrics; fallback ink
                        // can overflow without enlarging the inline box.
                        let (a, b) = authoredExtents(authored)
                        include(a, b, explicit: true)
                    } else {
                        includesNormal = true
                    }
                }
                if !matched, let strut, strut.lineHeight != nil {
                    let (a, b) = extents(strut)
                    include(a, b, explicit: true)
                    continue
                }
                if matched && !includesNormal { continue }
                let attributes = CTRunGetAttributes(glyphRun) as NSDictionary
                let shapedFont = attributes[kCTFontAttributeName] as! CTFont
                // Normal line height includes the actual emoji/fallback face's
                // line box, as the browser's does.
                let (a, d) = CSSLineBox.extents(shapedFont, height: nil)
                include(a, d, explicit: false)
            }
            explicit = explicit || aboveExplicit || belowExplicit
            baselines.append(y + above)
            y += above + below
            lineBottoms.append(y)
            maxWidth = max(maxWidth, w)
            glyphCount += CTLineGetGlyphCount(line)
            lines.append(line)
            start += count
        }
        if lines.isEmpty {
            // Empty editors retain the paragraph's own line box.
            baselines.append(minimum.0)
            y = minimum.0 + minimum.1
            explicit = strut?.lineHeight != nil
        }
        // An authored CSS line height fixes the line box, including fractions;
        // a `normal` one is whole pixels on macOS and CoreText's sum, rounded
        // up once, on iOS (CSSLineBox). A width rounds up to the browser's
        // 1/64 layout unit, so text laid out again at its own measured width
        // still fits on its lines.
        let paragraph = Paragraph(lines: lines, baselines: baselines, width: CSSLineBox.layoutWidth(maxWidth),
                                  height: explicit ? y : ceil(y), lineBottoms: lineBottoms,
                                  shape: shape, offeredWidth: width, glyphCount: glyphCount)
        paragraph.clampedRange = clampedRange
        paragraph.insets = insets
        return paragraph
    }

    /// Where a line may end, as UTF16 offsets, the last being `length`
    /// (TextEngine.lineBreaks: Chrome's opportunities, the walker's).
    func lineBoundaries(_ text: NSString, length: Int) -> [Int] {
        Self.lineBreaks(text, length: length)
    }

    /// An over-wide line truncated at `width` with the ellipsis in the style
    /// of the character it replaces; the browser keeps the first character
    /// when not even the token fits.
    static func ellipsis(_ line: CTLine, range: NSRange, width: Double, source: NSAttributedString) -> CTLine? {
        let at = max(range.location, min(NSMaxRange(range), source.length) - 1)
        let token = CTLineCreateWithAttributedString(NSAttributedString(string: "…", attributes: source.attributes(at: at, effectiveRange: nil)))
        return CTLineCreateTruncatedLine(line, width, .end, token)
    }

    func ellipsizedLine(_ spec: Spec, range: NSRange, width: Double, source: NSAttributedString? = nil) -> CTLine? {
        Self.clampedLine(source ?? attributed(spec), range: range, width: width)
    }

    /// A `line-clamp`'s last line: its text as it broke, then "…", truncated
    /// to `width`. Pure over `source`, so painting (a raster worker, the
    /// region worker) makes the same line layout made.
    static func clampedLine(_ source: NSAttributedString, range: NSRange, width: Double) -> CTLine? {
        let string = source.string as NSString
        var end = NSMaxRange(range)
        // A wrapped line already fits. Include the ellipsis before asking
        // CoreText to make room for it, removing the line's trailing break/space.
        while end > range.location && [9, 10, 13, 32, 0x2028, 0x2029].contains(Int(string.character(at: end - 1))) { end -= 1 }
        let candidate = NSMutableAttributedString(attributedString: source.attributedSubstring(from: NSRange(location: 0, length: end)))
        candidate.append(NSAttributedString(string: "…", attributes: source.attributes(at: max(range.location, end - 1), effectiveRange: nil)))
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
    func minContentWidth(_ spec: Spec) -> CGFloat {
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
        // Repeated words previously reused entire cached Paragraphs. Keep that
        // benefit with probe-local scalars, bounded by the same logical-payload
        // target; unique words beyond it are measured normally, never omitted.
        var words: [Run: CGFloat] = [:]
        var wordBytes = 0
        // CSS `text-indent` is part of the first line, so of its first word's
        // piece; a list item's indent of each of its words, its marker hung.
        var indent = spec.textIndent
        for r in spec.runs where !r.hang {
            for word in unbreakablePieces(r.text) {
                var one = spec
                one.runs = [Run(text: String(word), size: r.size, weight: r.weight, family: r.family, italic: r.italic, lineHeight: r.lineHeight, letterSpacing: r.letterSpacing, numeric: r.numeric)]
                let key = one.runs[0]
                defer { indent = 0 }
                let inset = indent + r.indent
                if let width = words[key] { widest = max(widest, width + inset); continue }
                // This probe needs one scalar, never a cached width-specific
                // Paragraph or a historical per-word CTTypesetter.
                let line = CTLineCreateWithAttributedString(attributed(one))
                let width = CSSLineBox.layoutWidth(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                widest = max(widest, width + inset)
                let bytes = key.text.utf8.count + MemoryLayout<Run>.stride + MemoryLayout<CGFloat>.stride
                if bytes <= residency.softTargetBytes - wordBytes {
                    words[key] = width; wordBytes += bytes
                }
            }
        }
        residency.putMinimum(identity, width: widest)
        return widest
    }

    /// Paint a paragraph into a y-down context (a flipped NSView's, a
    /// UIView's): one CTLineDraw per line, baselines currently rounded to
    /// logical points, flush by alignment.
    static func draw(_ p: Paragraph, spec: Spec, in bounds: CGRect, context ctx: CGContext, dirty: CGRect? = nil) {
        // CSS `text-shadow`: under every glyph of the paragraph at once, so
        // one line's shadow never covers another's text (LLP 1077 D3). Core
        // Graphics' blur is CSS's radius; its offset is base space, y up.
        if let s = spec.shadow {
            ctx.saveGState()
            ctx.setShadow(offset: CGSize(width: s[0], height: -s[1]), blur: s[2], color: TextEngine.shadowColor(s))
            ctx.beginTransparencyLayer(auxiliaryInfo: nil)
        }
        defer { if spec.shadow != nil { ctx.endTransparencyLayer(); ctx.restoreGState() } }
        func paint(_ index: Int) {
            let line = spec.ellipsis ? p.ellipsized(index, spec: spec, width: bounds.width) : p.lines[index]
            let baseline = p.baselines[index]
            let x = p.origin(index, align: spec.align, width: bounds.width)
            TextLinePaint.draw(line, at: CGPoint(x: bounds.minX + x, y: bounds.minY + baseline.rounded()), in: ctx)
        }
        if let dirty {
            p.inkBounds().forEachLine(from: dirty.minY - bounds.minY,
                                     through: dirty.maxY - bounds.minY, paint)
        } else {
            for index in p.lines.indices { paint(index) }
        }
    }

    /// The kernel's measurer for one request: called for every paragraph
    /// it lays out. The paragraph wrapped to answer is the one the presenter
    /// paints.
    func measure(_ request: ExactMeasureRequest) -> ExactMetrics {
        measureCount += 1
        #if os(macOS)
        if let pending = readerMeasureOnUI(request) { return pending }
        #endif
        let lookupStarted = CACurrentMediaTime()
        let intrinsic = request.width < 0
        let offer = request.width == EXACT_MIN_CONTENT ? TextAnswerKey.minContent
            : intrinsic ? TextAnswerKey.maxContent : TextAnswerKey.offer(width: CGFloat(request.width))
        // What this measurer answered before, whatever its residency still
        // holds: no identity, shape or paragraph is touched for it.
        if publishes, let answers, request.exclusion_count == 0, request.markup == 0 {
            let hash = TextAnswerKey.write(request, into: &answerKey)
            if let metrics = answers.metrics(hash: hash, key: answerKey, offer: offer) {
                measureHits += 1
                measureSeconds += CACurrentMediaTime() - lookupStarted
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
                measureSeconds += CACurrentMediaTime() - lookupStarted
                return metrics
            }
            if !intrinsic, let p = residency.geometry(identity, width: CGFloat(request.width)) {
                keepBreaks(p, identity: identity, width: CGFloat(request.width))
                measureHits += 1
                measureSeconds += CACurrentMediaTime() - lookupStarted
                return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
            }
        }
        // Preserve measureSeconds as cache/layout work, excluding Run/Spec
        // decoding: a fallback adds its failed borrowed lookup interval below.
        let lookupSeconds = CACurrentMediaTime() - lookupStarted
        // A new width needs layout, but exact borrowed matching already
        // proved these owned runs and metric fields are the same request.
        let spec = knownIdentity?.geometry ?? requestSpec(request)
        let started = CACurrentMediaTime()
        if request.exclusion_count > 0, let shapes = request.exclusions {
            let flow = UnsafeBufferPointer(start: shapes, count: request.exclusion_count).map(TextFlowShape.init)
            let width = request.width >= 0 ? CGFloat(request.width) : request.width == EXACT_MIN_CONTENT ? minContentWidth(spec) : paragraph(spec, width: .infinity).width
            let p = paragraph(spec, width: width, flow: flow)
            measureSeconds += lookupSeconds + (CACurrentMediaTime() - started)
            return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
        }
        let identity = knownIdentity ?? residency.identityAfterBorrowedMiss(spec)
        if knownIdentity == nil, !publishes, let metrics = residency.scalar(identity, kind: kind) {
            measureHits += 1
            measureSeconds += lookupSeconds + (CACurrentMediaTime() - started)
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
        measureSeconds += lookupSeconds + (CACurrentMediaTime() - started)
        return metrics
    }

    /// The paragraph a request describes, as the presenter paints it.
    func requestSpec(_ request: ExactMeasureRequest) -> Spec {
        func run(_ run: ExactTextRun) -> Run {
            Run(text: String(decoding: UnsafeBufferPointer(start: run.text, count: run.len), as: UTF8.self), size: CGFloat(run.font_size), weight: Int(run.font_weight), family: Int(run.font_family), italic: run.italic != 0, lineHeight: run.has_line_height != 0 ? CGFloat(run.line_height) : nil, letterSpacing: CGFloat(run.letter_spacing), numeric: Int(run.font_variant_numeric))
        }
        var runs = UnsafeBufferPointer(start: request.runs, count: request.count).map(run)
        // Markdown source arrives as one run; the archive expands it the
        // same way the presenter paints it (LLP 1045 D3).
        if request.markup != 0, let source = runs.first { runs = MarkdownLink.installed?.expand(source.text, base: source, color: nil) ?? runs }
        // Metric-only keys match the geometry used by the colored presenter.
        var made = Spec(runs: runs, align: Int(request.align), lineClamp: Int(request.line_clamp), color: [0, 0, 0, 255], overflowWrap: Int(request.overflow_wrap), direction: Int(request.direction), whiteSpace: Int(request.white_space), strut: run(request.strut))
        made.textIndent = CGFloat(request.text_indent); made.hyphens = Int(request.hyphens)
        if made.hyphens == 2, let lang = request.lang {
            made.language = String(decoding: UnsafeBufferPointer(start: lang, count: request.lang_len), as: UTF8.self)
        }
        made.hyphenateAuto()
        return made
    }

    /// The C ABI's synchronous font seam, invoked before the kernel asks its
    /// first text measurement; `ctx` is the session's engine.
    /// `ctx` is the measurer: its painter installs the same catalog on
    /// main while the owner waits (the catalog lives for this call).
    static let installFonts: ExactFontsFn = { ctx, catalog in
        guard let ctx else { return }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        engine.install(catalog)
        if let painter = engine.painter { Owner.shared.callMain { painter.install(catalog) } }
    }

    /// The kernel's text measurer; `ctx` is the session's engine.
    static let measureText: ExactMeasureFn = { ctx, request in
        guard let ctx, let request = request?.pointee else { return ExactMetrics(width: 0, height: 0, baseline: -1) }
        return Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue().measure(request)
    }
}

/// An inline box's `background-color`, carried on the attributed source so
/// that the main thread's paint and a worker's raster read the same value.
/// The content area is the run's own font's, as the web's inline box is.
final class InlineBackground: NSObject {
    let color: CGColor
    let ascent: CGFloat
    let descent: CGFloat
    init(color: CGColor, ascent: CGFloat, descent: CGFloat) {
        self.color = color; self.ascent = ascent; self.descent = descent
    }
    override func isEqual(_ object: Any?) -> Bool {
        guard let other = object as? InlineBackground else { return false }
        return color == other.color && ascent == other.ascent && descent == other.descent
    }
    override var hash: Int { color.hashValue ^ ascent.hashValue ^ descent.hashValue }
}

extension NSAttributedString.Key {
    static let exactBackground = NSAttributedString.Key("ExactInlineBackground")
}

/// One line's paint into a y-down context, shared by every Apple painter.
enum TextLinePaint {
    /// The CTM, not the text matrix, flips y: CoreText positions a glyph
    /// in text space, so a flipped text matrix turned the vertical offsets
    /// of cursive attachment and marks (SF Arabic's) upside down, and that
    /// ink fell below the line box it was measured in.
    /// `scale`: base-space units per point, for a run's own shadow (1 in a
    /// view's context, the pixel scale in a bitmap the host made).
    static func draw(_ line: CTLine, at origin: CGPoint, in ctx: CGContext, scale: CGFloat = 1, pass: Pass = .all) {
        ctx.saveGState()
        for (rect, color) in backgrounds(line, at: origin) where pass.paintsBackgrounds {
            ctx.setFillColor(color); ctx.fill(rect)
        }
        // The text matrix is not graphics state; put the caller's back.
        let matrix = ctx.textMatrix
        ctx.translateBy(x: origin.x, y: origin.y)
        ctx.scaleBy(x: 1, y: -1)
        ctx.textMatrix = .identity
        ctx.textPosition = .zero
        if !drawShadowed(line, in: ctx, scale: scale, pass: pass) { CTLineDraw(line, ctx) }
        ctx.textMatrix = matrix
        ctx.restoreGState()
    }

    /// CSS: an inline box's background covers each of its line fragments,
    /// its glyphs' advance across and its font's content area down. Glyph
    /// runs that fallback or bidi split are joined again where they touch.
    static func backgrounds(_ line: CTLine, at origin: CGPoint) -> [(CGRect, CGColor)] {
        var spans: [(CGFloat, CGFloat, InlineBackground)] = []
        let runs = CTLineGetGlyphRuns(line) as! [CTRun]
        // CSS removes the collapsible spaces that end a line: a wrapped run's
        // background stops at its last glyph, as the browser paints it. A
        // left-to-right line ends on the right (LLP 1053 §0, the Linux parity).
        var end = CGFloat.infinity
        if !runs.contains(where: { CTRunGetStatus($0).contains(.rightToLeft) }) {
            end = CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) - CGFloat(CTLineGetTrailingWhitespaceWidth(line))
        }
        for run in runs {
            let attributes = CTRunGetAttributes(run) as NSDictionary
            guard let fill = attributes[NSAttributedString.Key.exactBackground] as? InlineBackground else { continue }
            let count = CTRunGetGlyphCount(run)
            guard count > 0 else { continue }
            var positions = [CGPoint](repeating: .zero, count: count)
            var advances = [CGSize](repeating: .zero, count: count)
            CTRunGetPositions(run, CFRange(), &positions)
            CTRunGetAdvances(run, CFRange(), &advances)
            var lo = CGFloat.infinity, hi = -CGFloat.infinity
            for i in 0..<count {
                lo = min(lo, positions[i].x, positions[i].x + advances[i].width)
                hi = max(hi, positions[i].x, positions[i].x + advances[i].width)
            }
            hi = min(hi, end)
            if hi > lo { spans.append((lo, hi, fill)) }
        }
        spans.sort { $0.0 < $1.0 }
        var merged: [(CGFloat, CGFloat, InlineBackground)] = []
        for span in spans {
            if let last = merged.last, last.2.isEqual(span.2), span.0 <= last.1 + 0.01 {
                merged[merged.count - 1].1 = max(last.1, span.1)
            } else { merged.append(span) }
        }
        return merged.map { lo, hi, fill in
            (CGRect(x: origin.x + lo, y: origin.y - fill.ascent, width: hi - lo, height: fill.ascent + fill.descent), fill.color)
        }
    }
}
