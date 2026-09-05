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
    var lineHeight: CGFloat
    var letterSpacing: CGFloat
    var color: [Double]? = nil
    var decoration: String = ""
    var href: String = ""
}

/// A paragraph's specification: runs plus paragraph style.
struct Spec: Hashable {
    var runs: [Run]
    var align: Int // 0 left, 1 center, 2 right, 3 justify
    var lineClamp: Int
    var color: [Double] // r g b a, 0–255
}

/// A wrapped paragraph at one width: what is measured is what is painted.
final class Paragraph {
    let lines: [CTLine]
    /// Baseline of each line, measured from the top.
    let baselines: [CGFloat]
    let width: CGFloat
    let height: CGFloat
    var firstBaseline: CGFloat { baselines.first ?? 0 }
    init(lines: [CTLine], baselines: [CGFloat], width: CGFloat, height: CGFloat) {
        self.lines = lines
        self.baselines = baselines
        self.width = width
        self.height = height
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
    var paragraphs: [Int: Paragraph] = [:]
    private var catalog: [Int: [RegisteredFace]] = [:]
    /// Where a declared face's relative source resolves: the app's resolver
    /// (LLP 1031 D1 — the committed complete generation, else the root).
    let resolve: (String) -> URL?
    let read: (String) -> Data?
    private var pendingFonts: [URL] = []
    /// How many times the kernel asked, how many were answered from cache, and
    /// how long the misses took, since this session started.
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
        private let paragraphs: [Int: Paragraph]
        private let catalog: [Int: [RegisteredFace]]

        fileprivate init(_ engine: TextEngine) {
            pendingFonts = engine.pendingFonts
            fonts = engine.fonts
            paragraphs = engine.paragraphs
            catalog = engine.catalog
        }

        fileprivate func restore(into engine: TextEngine) {
            engine.pendingFonts = pendingFonts
            engine.fonts = fonts
            engine.paragraphs = paragraphs
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
        var h = Hasher()
        h.combine(spec)
        h.combine(width)
        let key = h.finalize()
        if let p = paragraphs[key] { return p }
        if paragraphs.count > 4096 { paragraphs.removeAll(keepingCapacity: true) }
        let p = layout(spec, width: width)
        paragraphs[key] = p
        return p
    }

    private func layout(_ spec: Spec, width: CGFloat) -> Paragraph {
        let s = attributed(spec)
        let typesetter = CTTypesetterCreateWithAttributedString(s)
        let length = s.length
        let lineHeight = spec.runs.map(\.lineHeight).max() ?? 0
        var lines: [CTLine] = []
        var baselines: [CGFloat] = []
        var maxWidth: CGFloat = 0
        var y: CGFloat = 0
        var start = 0
        let limit = width.isFinite ? Double(width) : Double.greatestFiniteMagnitude
        while start < length {
            if spec.lineClamp > 0 && lines.count == spec.lineClamp { break }
            var count = CTTypesetterSuggestLineBreak(typesetter, start, limit)
            if count <= 0 { count = length - start }
            var line = CTTypesetterCreateLine(typesetter, CFRangeMake(start, count))
            if spec.lineClamp > 0 && lines.count + 1 == spec.lineClamp && start + count < length,
               let token = CTLineCreateWithAttributedString(NSAttributedString(string: "…", attributes: s.attributes(at: start, effectiveRange: nil))) as CTLine?,
               let truncated = CTLineCreateTruncatedLine(line, limit, .end, token) {
                line = truncated
            }
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            let w = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, &leading))
            // CSS: `line-height: normal` is the font's ascent + descent +
            // line gap; a set line-height centers the glyphs in the box.
            let natural = ascent + descent + leading
            let box = lineHeight > 0 ? lineHeight : natural
            let half = lineHeight > 0 ? (lineHeight - natural) / 2 : 0
            baselines.append(y + half + ascent)
            y += box
            maxWidth = max(maxWidth, w)
            lines.append(line)
            start += count
        }
        if lines.isEmpty {
            // Empty text still has a line box.
            let f0 = spec.runs.first.map { font(size: $0.size, weight: $0.weight, family: $0.family, italic: $0.italic) } ?? PlatformFont.systemFont(ofSize: 16)
            let natural = f0.ascender - f0.descender + f0.leading
            let box = lineHeight > 0 ? lineHeight : natural
            baselines.append(f0.ascender + (lineHeight > 0 ? (lineHeight - natural) / 2 : 0))
            y = box
        }
        return Paragraph(lines: lines, baselines: baselines, width: ceil(maxWidth), height: ceil(y))
    }

    /// As narrow as the content can be: the longest unbreakable piece.
    func minContentWidth(_ spec: Spec) -> CGFloat {
        var widest: CGFloat = 0
        for r in spec.runs {
            for word in r.text.split(whereSeparator: { $0.isWhitespace }) {
                var one = spec
                one.runs = [Run(text: String(word), size: r.size, weight: r.weight, family: r.family, italic: r.italic, lineHeight: r.lineHeight, letterSpacing: r.letterSpacing)]
                widest = max(widest, paragraph(one, width: .infinity).width)
            }
        }
        return widest
    }

    /// Paint a paragraph into a y-down context (a flipped NSView's, a
    /// UIView's): one CTLineDraw per line, baselines snapped to device
    /// pixels, flush by alignment.
    static func draw(_ p: Paragraph, spec: Spec, in bounds: CGRect, context ctx: CGContext) {
        ctx.saveGState()
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        for (line, baseline) in zip(p.lines, p.baselines) {
            let x = CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(bounds.width)))
            ctx.textPosition = CGPoint(x: bounds.minX + x, y: bounds.minY + baseline.rounded())
            CTLineDraw(line, ctx)
        }
        ctx.restoreGState()
    }

    /// The kernel's measurer for one request: called for every paragraph
    /// it lays out. The paragraph wrapped to answer is the one the presenter
    /// paints.
    func measure(_ request: ExactMeasureRequest) -> ExactMetrics {
        measureCount += 1
        let runs = UnsafeBufferPointer(start: request.runs, count: request.count).map { run in
            Run(text: String(decoding: UnsafeBufferPointer(start: run.text, count: run.len), as: UTF8.self), size: CGFloat(run.font_size), weight: Int(run.font_weight), family: Int(run.font_family), italic: run.italic != 0, lineHeight: CGFloat(run.line_height), letterSpacing: CGFloat(run.letter_spacing))
        }
        // Color does not change metrics, but it remains in Spec's paragraph key:
        // measurement uses black while presenters paint with the real color, so
        // they shape separately. Removing color from that key remains owed.
        let spec = Spec(runs: runs, align: Int(request.align), lineClamp: Int(request.line_clamp), color: [0, 0, 0, 255])
        let started = CACurrentMediaTime()
        let width: CGFloat = request.width == EXACT_MIN_CONTENT ? minContentWidth(spec) : request.width < 0 ? .infinity : CGFloat(request.width)
        let before = paragraphs.count
        let p = paragraph(spec, width: width)
        if paragraphs.count == before { measureHits += 1 } else { measureSeconds += CACurrentMediaTime() - started }
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
