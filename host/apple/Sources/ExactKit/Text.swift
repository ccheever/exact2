import ExactCore
import CExact
import CoreText
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

final class PresentedText: TextPresentation {
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
    var fieldChrome: FieldChromeCache?
    var buttonMeasurements: ButtonMeasureCache?
    var platformControlID: UInt16?
    var platformControlFont: PlatformFont?
    var platformControlName = NSData()
    func reset() {
        #if os(macOS)
        readerParagraphs.removeAll()
        #endif
        platformControlID = nil
    }
    func checkpoint() -> () -> Void {
        let id = platformControlID, font = platformControlFont, name = platformControlName
        return { [self] in platformControlID = id; platformControlFont = font; platformControlName = name }
    }
    func controlFont(_ engine: TextEngine, size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont? {
        engine.controlFont(size: size, weight: weight, family: family, italic: italic)
    }
    func measure(_ engine: TextEngine, _ request: ExactMeasureRequest) -> ExactMetrics? {
        #if os(macOS)
        return engine.readerMeasureOnUI(request)
        #else
        return nil
        #endif
    }
    func markdown(_ engine: TextEngine, source: Run) -> [Run]? {
        MarkdownLink.installed?.expand(source.text, base: source, color: nil, boldText: engine.boldTextSteps(source.family))
    }
    func attributes(_ r: Run, color: [Double], attributes a: inout [NSAttributedString.Key: Any]) {
        a[.foregroundColor] = r.color.map(TextEngine.color) ?? TextEngine.color(color)
        if r.hidden { a[.exactHidden] = true }
        if r.fixedInk, r.color != nil { a[.exactFixedInk] = true }
        // A centred stroke over the fill: Core Text's negative width,
        // in percent of the run's size (LLP 1077 D7). Each run's own.
        if let st = r.stroke, st.count == 5, st[0] > 0, r.size > 0 {
            a[.strokeWidth] = -st[0] / Double(r.size) * 100
            a[.strokeColor] = TextEngine.color(Array(st[1...]))
        }
        if let sh = r.shadow, TextEngine.isShadow(sh) { a[.exactShadow] = TextRunShadow(sh) }
        if r.letterSpacing != 0 { a[.kern] = r.letterSpacing }
        if r.underlined { a[.underlineStyle] = NSUnderlineStyle.single.rawValue }
        if r.decoration.contains("line-through") { a[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
        if !r.hidden, let fill = r.background.map(TextEngine.color), fill.cgColor.alpha > 0 {
            let f = a[.font] as! PlatformFont
            let (ascent, descent) = CSSLineBox.content(f as CTFont)
            a[.exactBackground] = InlineBackground(color: fill.cgColor, ascent: ascent, descent: descent)
        }
    }
}

extension TextEngine {
    private var presented: PresentedText { presentation as! PresentedText }
    convenience init(resolve: @escaping (String) -> URL?, read: ((String) -> Data?)? = nil,
                     bundled: @escaping (String) -> URL? = { _ in nil },
                     coldTextTargetBytes: Int = TextResidency.defaultSoftTargetBytes) {
        self.init(assets: resolve, read: read, bundled: bundled, coldTextTargetBytes: coldTextTargetBytes, presentation: PresentedText())
    }
    /// A painting engine with its measurer.
    static func pair(resolve: @escaping (String) -> URL?, read: ((String) -> Data?)? = nil,
                     bundled: @escaping (String) -> URL? = { _ in nil },
                     answerBytes: Int = TextAnswers.defaultLimit) -> TextEngine {
        let painter = TextEngine(resolve: resolve, read: read, bundled: bundled)
        let measurer = TextEngine(resolve: resolve, read: read, bundled: bundled)
        painter.connect(measurer: measurer, answerBytes: answerBytes)
        return painter
    }
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
    var fieldChrome: FieldChromeCache? { get { presented.fieldChrome } set { presented.fieldChrome = newValue } }
    var buttonMeasurements: ButtonMeasureCache? { get { presented.buttonMeasurements } set { presented.buttonMeasurements = newValue } }
    var platformControlID: UInt16? { get { presented.platformControlID } set { presented.platformControlID = newValue } }
    var platformControlFont: PlatformFont? { get { presented.platformControlFont } set { presented.platformControlFont = newValue } }
    var platformControlName: NSData { get { presented.platformControlName } set { presented.platformControlName = newValue } }
    #if os(macOS)
    var readerParagraphs: [UInt32: RegionReaderParagraph] { get { presented.readerParagraphs } set { presented.readerParagraphs = newValue } }
    func readerHolds(_ view: UInt32) -> Bool { presented.readerHolds(view) }
    #endif
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
