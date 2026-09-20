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
typealias PlatformFont = UIFont
typealias PlatformColor = UIColor
#else
import AppKit
typealias PlatformFont = NSFont
typealias PlatformColor = NSColor
#endif
import CExact
import CoreText

/// One styled run: what changes glyph metrics.
struct Run: Hashable {
    var text: String
    var size: CGFloat
    var weight: Int
    var family: Int
    var italic: Bool
    var lineHeight: CGFloat?
    var letterSpacing: CGFloat
    var color: [Double]? = nil
    var decoration: String = ""
    var href: String = ""

    static func == (lhs: Run, rhs: Run) -> Bool {
        guard lhs.size == rhs.size, lhs.weight == rhs.weight, lhs.family == rhs.family,
              lhs.italic == rhs.italic, lhs.lineHeight == rhs.lineHeight,
              lhs.letterSpacing == rhs.letterSpacing, lhs.color == rhs.color,
              lhs.decoration == rhs.decoration, lhs.href == rhs.href else { return false }
        // CoreText's ranges address the original UTF16 source. Swift String's
        // canonical equality would alias NFC/NFD paragraphs with different
        // source lengths, so both equality and hashing use the exact UTF8.
        var a = lhs.text, b = rhs.text
        return a.withUTF8 { left in b.withUTF8 { right in left.elementsEqual(right) } }
    }

    func hash(into hasher: inout Hasher) {
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
        hasher.combine(color)
        hasher.combine(decoration)
        hasher.combine(href)
    }
}

/// A paragraph's specification: runs plus paragraph style.
struct Spec: Hashable {
    var runs: [Run]
    var align: Int // 0 left, 1 center, 2 right, 3 justify
    var lineClamp: Int
    var color: [Double] // r g b a, 0–255
    var overflowWrap: Int = 0 // CSS: normal, break-word, anywhere
    var strut: Run? = nil // paragraph minimum line box, including smaller inline runs
}

/// A wrapped paragraph at one width: what is measured is what is painted.
final class Paragraph {
    let lines: [CTLine]
    /// Baseline of each line, measured from the top.
    let baselines: [CGFloat]
    let width: CGFloat
    let height: CGFloat
    let lineBottoms: [CGFloat]
    let shape: TextShape?
    let residencyKey: TextParagraphKey?
    let coreTextEstimateBytes: Int
    var ownedPayloadBytes: Int {
        lines.count * MemoryLayout<CTLine>.stride
            + (baselines.count + lineBottoms.count) * MemoryLayout<CGFloat>.stride
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
    init(lines: [CTLine], baselines: [CGFloat], width: CGFloat, height: CGFloat, lineBottoms: [CGFloat] = [],
         shape: TextShape? = nil, offeredWidth: CGFloat? = nil, glyphCount: Int = 0) {
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
        for i in value.runs.indices {
            value.runs[i].color = nil
            value.runs[i].decoration = ""
            value.runs[i].href = ""
        }
        if var strut = value.strut {
            strut.text = ""; strut.color = nil; strut.decoration = ""; strut.href = ""
            value.strut = strut
        }
        return value
    }
}

private struct RegisteredFace {
    let weight: Int
    let italic: Bool
    /// Created from the registered URL itself — never from the Contract alias
    /// or a lookup in the system font library (LLP 1019 D3).
    let descriptor: CTFontDescriptor
}

/// CoreText's process-wide registration, once per URL, never undone.
enum FontRegistry {
    nonisolated(unsafe) private static var registered: Set<URL> = []

    static func register(_ url: URL) -> Bool {
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

final class TextEngine {
    var fonts: [String: PlatformFont] = [:]
    private var residency: TextResidency
    var residencyStats: TextResidencyStats { residency.stats }
    /// The NodeView's existing cachedTextLayout is the accepted lease. The
    /// cache keeps only a weak lookup once that view owns the paragraph.
    func accepted(_ paragraph: Paragraph) { residency.accepted(paragraph) }
    private var catalog: [Int: [RegisteredFace]] = [:]
    /// Where a declared face's relative source resolves: the app's resolver
    /// (LLP 1031 D1 — the committed complete generation, else the root).
    let resolve: (String) -> URL?
    let read: (String) -> Data?
    private var pendingFonts: [URL] = []
    /// Native callback entries, native cache hits, and native cache/layout time
    /// since session start. Rust identified-metric hits bypass this callback;
    /// the timer below excludes C-run decoding and Swift String construction.
    var measureCount = 0
    var measureHits = 0
    var measureSeconds = 0.0
    private var lineBreaker: CFStringTokenizer?

    init(resolve: @escaping (String) -> URL?, read: ((String) -> Data?)? = nil,
         coldTextTargetBytes: Int = TextResidency.defaultSoftTargetBytes) {
        residency = TextResidency(softTargetBytes: coldTextTargetBytes)
        self.resolve = resolve
        self.read = read ?? { name in resolve(name).flatMap { try? Data(contentsOf: $0) } }
    }

    func commitFonts() {
        for url in pendingFonts { _ = FontRegistry.register(url) }
        pendingFonts = []
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

        fileprivate init(_ engine: TextEngine) {
            pendingFonts = engine.pendingFonts
            fonts = engine.fonts
            residency = engine.residency
            catalog = engine.catalog
        }

        fileprivate func restore(into engine: TextEngine) {
            engine.pendingFonts = pendingFonts
            engine.fonts = fonts
            engine.residency = residency
            engine.catalog = catalog
        }
    }

    func checkpoint() -> Checkpoint { Checkpoint(self) }
    func restore(_ checkpoint: Checkpoint) { checkpoint.restore(into: self) }

    /// Replace this session's plan-scoped catalog before layout. A fresh
    /// residency namespace prevents old accepted font identities from aliasing
    /// the candidate; checkpoints retain and restore their original namespace.
    func install(_ pointer: UnsafePointer<ExactFontCatalog>?) {
        fonts.removeAll(keepingCapacity: true)
        residency = TextResidency(softTargetBytes: residency.softTargetBytes)
        catalog.removeAll(keepingCapacity: true)
        guard let value = pointer?.pointee else { return }
        let rows = UnsafeBufferPointer(start: value.faces, count: value.count)
        var staged: [Int: [RegisteredFace]] = [:]
        var failed = Set<Int>()
        for row in rows {
            let stack = Int(row.stack)
            guard let sourceBytes = row.source else { failed.insert(stack); continue }
            let source = String(decoding: UnsafeBufferPointer(start: sourceBytes, count: row.source_len), as: UTF8.self)
            guard URL(string: source)?.scheme == nil, !source.hasPrefix("/"),
                  let bytes = read(source),
                  let descriptors = CTFontManagerCreateFontDescriptorsFromData(bytes as CFData) as? [CTFontDescriptor],
                  let descriptor = descriptors.first else {
                failed.insert(stack)
                continue
            }
            if let url = fontURL(source) { pendingFonts.append(url) }
            staged[stack, default: []].append(RegisteredFace(
                weight: Int(row.weight), italic: row.italic != 0, descriptor: descriptor))
        }
        for stack in failed {
            staged.removeValue(forKey: stack)
            fputs("[Fonts] font.registration.failed: stack=\(stack)\n", stderr)
        }
        catalog = staged
    }

    private func fontURL(_ source: String) -> URL? {
        guard URL(string: source)?.scheme == nil, !source.hasPrefix("/") else { return nil }
        return resolve(source)
    }

    private static func matched(_ faces: [RegisteredFace], weight: Int, italic: Bool) -> RegisteredFace {
        let styled = faces.filter { $0.italic == italic }
        let candidates = styled.isEmpty ? faces : styled
        func rank(_ face: RegisteredFace) -> (Int, Int) {
            let w = face.weight
            if weight >= 400 && weight <= 500 {
                if w >= weight && w <= 500 { return (0, w - weight) }
                if w < weight { return (1, weight - w) }
                return (2, w - 500)
            }
            if weight < 400 {
                return w <= weight ? (0, weight - w) : (1, w - weight)
            }
            return w >= weight ? (0, w - weight) : (1, weight - w)
        }
        return candidates.dropFirst().reduce(candidates[0]) { best, face in
            let a = rank(best), b = rank(face)
            return b.0 < a.0 || (b.0 == a.0 && b.1 < a.1) ? face : best
        }
    }

    func font(size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont {
        let key = "\(family)/\(size)/\(weight)/\(italic)"
        if let f = fonts[key] { return f }
        if let faces = catalog[family], !faces.isEmpty {
            let face = TextEngine.matched(faces, weight: weight, italic: italic)
            let f = CTFontCreateWithFontDescriptor(face.descriptor, size, nil) as PlatformFont
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
            f = NSFontManager.shared.convert(f, toHaveTrait: .italicFontMask)
            #endif
        }
        fonts[key] = f
        return f
    }

    /// A color from the style dictionary's `[r,g,b,a]` bytes (sRGB).
    static func color(_ c: [Double]) -> PlatformColor {
        #if canImport(UIKit)
        UIColor(red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
        #else
        NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
        #endif
    }

    func attributed(_ spec: Spec) -> NSAttributedString {
        let s = NSMutableAttributedString()
        let color = TextEngine.color(spec.color)
        for r in spec.runs {
            var a: [NSAttributedString.Key: Any] = [.font: font(size: r.size, weight: r.weight, family: r.family, italic: r.italic), .foregroundColor: r.color.map(TextEngine.color) ?? color]
            if r.letterSpacing != 0 { a[.kern] = r.letterSpacing }
            if r.decoration.contains("underline") || (r.decoration.isEmpty && !r.href.isEmpty) { a[.underlineStyle] = NSUnderlineStyle.single.rawValue }
            if r.decoration.contains("line-through") { a[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
            s.append(NSAttributedString(string: r.text, attributes: a))
        }
        return s
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
        let p = layout(shape, width: width, breaks: breaks)
        if width.isFinite { residency.put(p) }
        return p
    }

    private func shape(_ key: TextShapeKey, identity: TextIdentity) -> TextShape {
        if let cached = residency.shape(key) { return cached }
        residency.prepare(estimatedBytes: identity.ownedBytes + identity.utf16Count * 32)
        let shape = TextShape(key: key, identity: identity, attributed: attributed(key.paint.applying(to: identity)))
        residency.put(shape)
        return shape
    }

    private func layout(_ shape: TextShape, width: CGFloat, breaks: Paragraph? = nil) -> Paragraph {
        let spec = shape.spec, typesetter = shape.typesetter
        let length = shape.identity.utf16Count
        let strut = spec.strut ?? spec.runs.first
        func extents(_ run: Run) -> (CGFloat, CGFloat) {
            let f = font(size: run.size, weight: run.weight, family: run.family, italic: run.italic)
            let natural = f.ascender - f.descender + f.leading
            let half = ((run.lineHeight ?? natural) - natural) / 2
            return (f.ascender + half, -f.descender + f.leading + half)
        }
        let minimum = strut.map(extents) ?? (0, 0)
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
        var explicit = false
        var lineBottoms: [CGFloat] = []
        var lines: [CTLine] = []
        var glyphCount = 0
        var baselines: [CGFloat] = []
        var maxWidth: CGFloat = 0
        var y: CGFloat = 0
        var start = 0
        let limit = width.isFinite ? Double(width) : Double.greatestFiniteMagnitude
        // CoreText breaks a word when it cannot fit; CSS normal instead lets
        // that word overflow. Public Unicode line boundaries distinguish those
        // emergency breaks from ordinary opportunities (including CJK).
        var boundaries: [Int] = []
        var boundaryIndex = 0
        if spec.overflowWrap == 0 && width.isFinite && breaks == nil {
            boundaries = lineBoundaries(spec.runs.map(\.text).joined() as NSString, length: length)
        }
        while start < length {
            if spec.lineClamp > 0 && lines.count == spec.lineClamp { break }
            var count: Int
            if let breaks, lines.count < breaks.lines.count {
                count = CTLineGetStringRange(breaks.lines[lines.count]).length
            } else {
                count = CTTypesetterSuggestLineBreak(typesetter, start, limit)
                while boundaryIndex < boundaries.count && boundaries[boundaryIndex] < start + count {
                    boundaryIndex += 1
                }
                if boundaryIndex < boundaries.count {
                    count = boundaries[boundaryIndex] - start
                }
            }
            if count <= 0 { count = length - start }
            var line = CTTypesetterCreateLine(typesetter, CFRangeMake(start, count))
            if spec.lineClamp > 0 && lines.count + 1 == spec.lineClamp && start + count < length {
                line = ellipsizedLine(spec, range: NSRange(location: start, length: count), width: limit) ?? line
            }
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            let w = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, &leading))
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
                var offset = 0
                var matched = false, includesNormal = false
                // CoreText can coalesce adjacent spans with the same glyph
                // attributes even when their authored line heights differ.
                for authored in spec.runs {
                    let end = offset + (authored.text as NSString).length
                    defer { offset = end }
                    guard offset < range.location + range.length && end > range.location else { continue }
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
                let a = CTFontGetAscent(shapedFont), d = CTFontGetDescent(shapedFont), l = CTFontGetLeading(shapedFont)
                // Normal line height includes the actual emoji/fallback face's
                // metrics, as CTLine measurement did before typed line heights.
                include(a, d + l, explicit: false)
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
        // An authored CSS line height fixes the line box, including fractions.
        // Keep intrinsic width and `normal` height measurement separate: changing
        // their rounding also changes wrapping and the established host parity.
        return Paragraph(lines: lines, baselines: baselines, width: ceil(maxWidth),
                         height: explicit ? y : ceil(y), lineBottoms: lineBottoms,
                         shape: shape, offeredWidth: width, glyphCount: glyphCount)
    }

    /// Where Unicode lets a line end, as UTF16 offsets, the last being `length`.
    /// One tokenizer is handed each paragraph in turn: making one opens an ICU
    /// break iterator, which was a tenth of what measuring a paragraph cost.
    func lineBoundaries(_ text: NSString, length: Int) -> [Int] {
        let range = CFRange(location: 0, length: length)
        let tokenizer: CFStringTokenizer
        if let lineBreaker {
            CFStringTokenizerSetString(lineBreaker, text as CFString, range)
            tokenizer = lineBreaker
        } else {
            tokenizer = CFStringTokenizerCreate(nil, text as CFString, range, kCFStringTokenizerUnitLineBreak, nil)!
            lineBreaker = tokenizer
        }
        var boundaries: [Int] = []
        while CFStringTokenizerAdvanceToNextToken(tokenizer).rawValue != 0 {
            let token = CFStringTokenizerGetCurrentTokenRange(tokenizer)
            boundaries.append(token.location + token.length)
        }
        if boundaries.last != length { boundaries.append(length) }
        return boundaries
    }

    private func ellipsizedLine(_ spec: Spec, range: NSRange, width: Double) -> CTLine? {
        let source = attributed(spec)
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

    /// As narrow as the content can be: the longest unbreakable piece.
    func minContentWidth(_ spec: Spec) -> CGFloat {
        let identity = residency.identity(spec)
        if let width = residency.minimum(identity) { return width }
        residency.retireWidths(TextParagraphKey(shape: TextShapeKey(identity: identity, paint: TextPaint(spec)), width: .infinity))
        residency.prepare(estimatedBytes: identity.utf16Count * 32)
        var widest: CGFloat = 0
        if spec.overflowWrap == 2 {
            let source = attributed(spec), value = source.string as NSString
            var start = 0
            while start < value.length {
                let range = value.rangeOfComposedCharacterSequence(at: start)
                let line = CTLineCreateWithAttributedString(source.attributedSubstring(from: range))
                widest = max(widest, CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                start = NSMaxRange(range)
            }
            residency.putMinimum(identity, width: ceil(widest))
            return ceil(widest)
        }
        // Repeated words previously reused entire cached Paragraphs. Keep that
        // benefit with probe-local scalars, bounded by the same logical-payload
        // target; unique words beyond it are measured normally, never omitted.
        var words: [Run: CGFloat] = [:]
        var wordBytes = 0
        for r in spec.runs {
            for word in r.text.split(whereSeparator: { $0.isWhitespace }) {
                var one = spec
                one.runs = [Run(text: String(word), size: r.size, weight: r.weight, family: r.family, italic: r.italic, lineHeight: r.lineHeight, letterSpacing: r.letterSpacing)]
                let key = one.runs[0]
                if let width = words[key] { widest = max(widest, width); continue }
                // This probe needs one scalar, never a cached width-specific
                // Paragraph or a historical per-word CTTypesetter.
                let line = CTLineCreateWithAttributedString(attributed(one))
                let width = ceil(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                widest = max(widest, width)
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
        ctx.saveGState()
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        func paint(_ index: Int) {
            let line = p.lines[index], baseline = p.baselines[index]
            let x = CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(bounds.width)))
            ctx.textPosition = CGPoint(x: bounds.minX + x, y: bounds.minY + baseline.rounded())
            CTLineDraw(line, ctx)
        }
        if let dirty {
            p.inkBounds().forEachLine(from: dirty.minY - bounds.minY,
                                     through: dirty.maxY - bounds.minY, paint)
        } else {
            for index in p.lines.indices { paint(index) }
        }
        ctx.restoreGState()
    }

    /// The kernel's measurer for one request: called for every paragraph
    /// it lays out. The paragraph wrapped to answer is the one the presenter
    /// paints.
    func measure(_ request: ExactMeasureRequest) -> ExactMetrics {
        measureCount += 1
        let lookupStarted = CACurrentMediaTime()
        let knownIdentity = residency.borrowedIdentity(request)
        let intrinsic = request.width < 0
        let kind: TextScalarKind = request.width == EXACT_MIN_CONTENT ? .minContent : .maxContent
        if let identity = knownIdentity {
            if intrinsic, let metrics = residency.scalar(identity, kind: kind) {
                measureHits += 1
                measureSeconds += CACurrentMediaTime() - lookupStarted
                return metrics
            }
            if !intrinsic, let p = residency.geometry(identity, width: CGFloat(request.width)) {
                measureHits += 1
                measureSeconds += CACurrentMediaTime() - lookupStarted
                return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
            }
        }
        // Preserve measureSeconds as cache/layout work, excluding Run/Spec
        // decoding: a fallback adds its failed borrowed lookup interval below.
        let lookupSeconds = CACurrentMediaTime() - lookupStarted
        func run(_ run: ExactTextRun) -> Run {
            Run(text: String(decoding: UnsafeBufferPointer(start: run.text, count: run.len), as: UTF8.self), size: CGFloat(run.font_size), weight: Int(run.font_weight), family: Int(run.font_family), italic: run.italic != 0, lineHeight: run.has_line_height != 0 ? CGFloat(run.line_height) : nil, letterSpacing: CGFloat(run.letter_spacing))
        }
        let runs = UnsafeBufferPointer(start: request.runs, count: request.count).map(run)
        // Metric-only keys match the geometry used by the colored presenter.
        let spec = Spec(runs: runs, align: Int(request.align), lineClamp: Int(request.line_clamp), color: [0, 0, 0, 255], overflowWrap: Int(request.overflow_wrap), strut: run(request.strut))
        let started = CACurrentMediaTime()
        let identity = knownIdentity ?? residency.identityAfterBorrowedMiss(spec)
        if intrinsic, knownIdentity == nil, let metrics = residency.scalar(identity, kind: kind) {
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
        let metrics = ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
        if intrinsic { residency.put(identity, kind: kind, metrics: metrics) }
        measureSeconds += lookupSeconds + (CACurrentMediaTime() - started)
        return metrics
    }

    /// The C ABI's synchronous font seam, invoked before the kernel asks its
    /// first text measurement; `ctx` is the session's engine.
    static let installFonts: ExactFontsFn = { ctx, catalog in
        guard let ctx else { return }
        Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue().install(catalog)
    }

    /// The kernel's text measurer; `ctx` is the session's engine.
    static let measureText: ExactMeasureFn = { ctx, request in
        guard let ctx, let request = request?.pointee else { return ExactMetrics(width: 0, height: 0, baseline: -1) }
        return Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue().measure(request)
    }
}
