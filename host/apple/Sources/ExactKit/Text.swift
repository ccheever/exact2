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
    private(set) var cachedInk: ParagraphInkIndex?
    var firstBaseline: CGFloat { baselines.first ?? 0 }
    init(lines: [CTLine], baselines: [CGFloat], width: CGFloat, height: CGFloat, lineBottoms: [CGFloat] = []) {
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

    init(lines: [CTLine], baselines: [CGFloat]) {
        count = lines.count
        var size = 1
        while size < count { size *= 2 }
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

/// A bounded cache that evicts the coldest eighth, never all live paragraphs.
/// Value semantics also preserve candidate-boot checkpoints.
struct TextCache<Key: Hashable, Value> {
    private var entries: [Key: (value: Value, used: UInt64)] = [:]
    private var clock: UInt64 = 0
    var count: Int { entries.count }
    mutating func get(_ key: Key) -> Value? {
        guard let index = entries.index(forKey: key) else { return nil }
        clock &+= 1
        let value = entries.values[index].value
        // Mutate through the found index: no second content hash, while the
        // dictionary still performs value-semantic COW for saved checkpoints.
        entries.values[index].used = clock
        return value
    }
    mutating func put(_ key: Key, _ value: Value) {
        if entries.count >= 4096, entries[key] == nil {
            let cold = entries.sorted { $0.value.used < $1.value.used }.prefix(512).map(\.key)
            for key in cold { entries.removeValue(forKey: key) }
        }
        clock &+= 1
        entries[key] = (value, clock)
    }
    mutating func removeAll(keepingCapacity: Bool) { entries.removeAll(keepingCapacity: keepingCapacity) }
}

struct ParagraphKey: Hashable {
    let spec: Spec
    let width: CGFloat
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
    var paragraphs = TextCache<ParagraphKey, Paragraph>()
    private var minimums = TextCache<Spec, CGFloat>()
    private var typesetters = TextCache<Spec, CTTypesetter>()
    private var catalog: [Int: [RegisteredFace]] = [:]
    /// Where a declared face's relative source resolves: the app's resolver
    /// (LLP 1031 D1 — the committed complete generation, else the root).
    let resolve: (String) -> URL?
    let read: (String) -> Data?
    private var pendingFonts: [URL] = []
    /// How many times the kernel asked, how many were answered from cache, and
    /// total measurement time, since this session started.
    var measureCount = 0
    var measureHits = 0
    var measureSeconds = 0.0

    init(resolve: @escaping (String) -> URL?, read: ((String) -> Data?)? = nil) {
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
        private let paragraphs: TextCache<ParagraphKey, Paragraph>
        private let minimums: TextCache<Spec, CGFloat>
        private let typesetters: TextCache<Spec, CTTypesetter>
        private let catalog: [Int: [RegisteredFace]]

        fileprivate init(_ engine: TextEngine) {
            pendingFonts = engine.pendingFonts
            fonts = engine.fonts
            paragraphs = engine.paragraphs
            minimums = engine.minimums
            typesetters = engine.typesetters
            catalog = engine.catalog
        }

        fileprivate func restore(into engine: TextEngine) {
            engine.pendingFonts = pendingFonts
            engine.fonts = fonts
            engine.paragraphs = paragraphs
            engine.minimums = minimums
            engine.typesetters = typesetters
            engine.catalog = catalog
        }
    }

    func checkpoint() -> Checkpoint { Checkpoint(self) }
    func restore(_ checkpoint: Checkpoint) { checkpoint.restore(into: self) }

    /// Replace this session's plan-scoped catalog before layout. Clearing
    /// both caches is the plan identity in their keys (LLP 1019 D4).
    func install(_ pointer: UnsafePointer<ExactFontCatalog>?) {
        fonts.removeAll(keepingCapacity: true)
        paragraphs.removeAll(keepingCapacity: true)
        minimums.removeAll(keepingCapacity: true)
        typesetters.removeAll(keepingCapacity: true)
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

    /// Wrap `spec` at `width` (infinite = max-content). Cached.
    func paragraph(_ spec: Spec, width: CGFloat) -> Paragraph {
        let key = ParagraphKey(spec: spec, width: width)
        if let p = paragraphs.get(key) { return p }
        let geometry = spec.geometry
        // The measurement's line breaks are also the painted paragraph's breaks.
        // CoreText still creates colored lines, but never wraps them a second time.
        let breaks = geometry == spec || spec.lineClamp > 0 ? nil : paragraph(geometry, width: width)
        let p = layout(spec, width: width, breaks: breaks)
        paragraphs.put(key, p)
        return p
    }

    private func layout(_ spec: Spec, width: CGFloat, breaks: Paragraph? = nil) -> Paragraph {
        // The shaped text is independent of width. Keep it while resizing;
        // only line breaking and line placement depend on the offered width.
        let typesetter: CTTypesetter
        if let cached = typesetters.get(spec) { typesetter = cached }
        else {
            typesetter = CTTypesetterCreateWithAttributedString(attributed(spec))
            typesetters.put(spec, typesetter)
        }
        let length = spec.runs.reduce(0) { $0 + ($1.text as NSString).length }
        let strut = spec.strut ?? spec.runs.first
        func extents(_ run: Run) -> (CGFloat, CGFloat) {
            let f = font(size: run.size, weight: run.weight, family: run.family, italic: run.italic)
            let natural = f.ascender - f.descender + f.leading
            let half = ((run.lineHeight ?? natural) - natural) / 2
            return (f.ascender + half, -f.descender + f.leading + half)
        }
        let minimum = strut.map(extents) ?? (0, 0)
        var explicit = false
        var lineBottoms: [CGFloat] = []
        var lines: [CTLine] = []
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
            let text = spec.runs.map(\.text).joined() as NSString
            let tokenizer = CFStringTokenizerCreate(nil, text as CFString, CFRange(location: 0, length: length), kCFStringTokenizerUnitLineBreak, nil)!
            while CFStringTokenizerAdvanceToNextToken(tokenizer).rawValue != 0 {
                let range = CFStringTokenizerGetCurrentTokenRange(tokenizer)
                boundaries.append(range.location + range.length)
            }
            if boundaries.last != length { boundaries.append(length) }
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
                        let (a, b) = extents(authored)
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
                         height: explicit ? y : ceil(y), lineBottoms: lineBottoms)
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
        if let width = minimums.get(spec) { return width }
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
            minimums.put(spec, ceil(widest))
            return ceil(widest)
        }
        for r in spec.runs {
            for word in r.text.split(whereSeparator: { $0.isWhitespace }) {
                var one = spec
                one.runs = [Run(text: String(word), size: r.size, weight: r.weight, family: r.family, italic: r.italic, lineHeight: r.lineHeight, letterSpacing: r.letterSpacing)]
                widest = max(widest, paragraph(one, width: .infinity).width)
            }
        }
        minimums.put(spec, widest)
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
        func run(_ run: ExactTextRun) -> Run {
            Run(text: String(decoding: UnsafeBufferPointer(start: run.text, count: run.len), as: UTF8.self), size: CGFloat(run.font_size), weight: Int(run.font_weight), family: Int(run.font_family), italic: run.italic != 0, lineHeight: run.has_line_height != 0 ? CGFloat(run.line_height) : nil, letterSpacing: CGFloat(run.letter_spacing))
        }
        let runs = UnsafeBufferPointer(start: request.runs, count: request.count).map(run)
        // Metric-only keys match the geometry used by the colored presenter.
        let spec = Spec(runs: runs, align: Int(request.align), lineClamp: Int(request.line_clamp), color: [0, 0, 0, 255], overflowWrap: Int(request.overflow_wrap), strut: run(request.strut))
        let started = CACurrentMediaTime()
        let width: CGFloat = request.width == EXACT_MIN_CONTENT ? minContentWidth(spec) : request.width < 0 ? .infinity : CGFloat(request.width)
        let key = ParagraphKey(spec: spec, width: width)
        let p: Paragraph
        if let cached = paragraphs.get(key) { measureHits += 1; p = cached }
        else { p = paragraph(spec, width: width) }
        measureSeconds += CACurrentMediaTime() - started
        return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
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
