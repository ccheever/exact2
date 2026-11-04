#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A paragraph's ink in a test: the object it is, for a "not painted again"
/// check, and the size of its pixels, or of the store its drawing makes.
extension TextInk {
    var object: AnyObject {
        switch self {
        case .pixels(let image): image
        case .drawn(let drawing): drawing
        }
    }
    var width: Int {
        switch self {
        case .pixels(let image): image.width
        case .drawn(let drawing): Int((drawing.frame.width * drawing.scale).rounded(.up))
        }
    }
    var height: Int {
        switch self {
        case .pixels(let image): image.height
        case .drawn(let drawing): Int((drawing.frame.height * drawing.scale).rounded(.up))
        }
    }
}

extension TextRasterImage {
    /// Its pixels, or what its layer draws (`TextDrawing.painted`)
    var pixels: CGImage? {
        switch content {
        case .pixels(let image): image
        case .drawn(let drawing): drawing.painted()
        }
    }
    var isDrawn: Bool { if case .drawn = content { true } else { false } }
}

extension TextDrawing {
    /// `text` on one line in a bold system font, its pen at the frame's left, its baseline `baseline` points down
    convenience init(_ text: String, size: CGFloat = 16, color: CGColor, baseline: CGFloat, frame: CGRect, scale: CGFloat = 1) {
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: UIFont.boldSystemFont(ofSize: size) as CTFont]))
        self.init([MeasuredGlyphs.Line(line, origin: 0)!], at: [CGPoint(x: frame.minX, y: frame.minY + baseline)], frame: frame, scale: scale, color: color, smooths: true)
    }

    /// What its layer draws, painted as Core Animation hands the layer a context (its points, y down, at its
    /// scale), into pixels like the rasteriser's own: sRGB, four bytes a pixel
    func painted() -> CGImage? {
        let width = Int((frame.width * scale).rounded(.up)), height = Int((frame.height * scale).rounded(.up))
        guard width > 0, height > 0, let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: scale, y: -scale)
        paint(ctx)
        return ctx.makeImage()
    }
}

/// One-colour text is not painted by the rasteriser: it comes back as a
/// drawing its layer paints itself (`TextInk.drawn`, `DrawnInk`), on the
/// measured-glyph path and the general one, in its colour at its opacity.
final class OneColorInkIOSTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })
    private let spec = Spec(runs: [Run(text: "Ink", size: 32, weight: 700, family: 0, italic: false, lineHeight: 38, letterSpacing: 0)],
                            align: 0, lineClamp: 0, color: [255, 0, 0, 128])

    private func job(_ spec: Spec? = nil, measured: Bool) throws -> TextRasterJob {
        let spec = spec ?? self.spec
        let paragraph = engine.paragraph(spec, width: 160)
        let geometry = try XCTUnwrap(engine.measuredBreaks(spec, width: 160))
        let box = CGRect(x: 8, y: 8, width: 160, height: paragraph.height)
        let size = CGSize(width: 180, height: paragraph.height + 16)
        return TextRasterJob(source: engine.attributed(spec), ranges: geometry.ranges,
            baselines: geometry.baselines, flush: 0, box: box, size: size, scale: 1,
            measuredGlyphs: measured ? try XCTUnwrap(MeasuredGlyphs(paragraph, spec: spec, width: 160)) : nil,
            measuredColor: TextEngine.color(spec.color).cgColor)
    }

    /// The drawing painted as Core Animation hands a layer its context: the layer's points, y down
    private func painted(_ drawing: TextDrawing) throws -> (bytes: [UInt8], width: Int, height: Int) {
        let width = Int(drawing.frame.width.rounded(.up)), height = Int(drawing.frame.height.rounded(.up))
        let ctx = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: 1, y: -1)
        drawing.paint(ctx)
        let data = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        return (Array(UnsafeBufferPointer(start: data, count: width * height * 4)), width, height)
    }

    func testOneColourTextIsADrawingInItsColourAtItsOpacityOnBothPaths() throws {
        for measured in [true, false] {
            let result = try XCTUnwrap(try job(measured: measured).render())
            guard case .drawn(let drawing) = result.content else { return XCTFail("one colour is drawn by its layer (measured: \(measured))") }
            XCTAssertEqual(drawing.frame, result.frame, "the layer's frame is the ink's")
            let (bytes, width, height) = try painted(drawing)
            XCTAssertEqual(bytes[3], 0, "an uncovered corner stays clear")
            let alpha = stride(from: 3, to: width * height * 4, by: 4).map { bytes[$0] }
            XCTAssertGreaterThan(alpha.filter { $0 > 16 }.count, 50, "the glyphs paint (measured: \(measured))")
            let peak = try XCTUnwrap(alpha.indices.max(by: { alpha[$0] < alpha[$1] }))
            XCTAssertLessThanOrEqual(abs(Int(alpha[peak]) - 128), 1, "at the colour's opacity")
            XCTAssertLessThanOrEqual(abs(Int(bytes[peak * 4]) - Int(alpha[peak])), 1, "in the premultiplied red")
            XCTAssertEqual(bytes[peak * 4 + 1], 0)
            XCTAssertEqual(bytes[peak * 4 + 2], 0)
        }
    }

    /// On screen, Core Animation keeps a one-colour paragraph's layer in a store of one byte a pixel,
    /// as it keeps a `UILabel`'s, and the layer has no mask
    func testOnScreenTheParagraphsStoreIsOneByteAPixelWithNoMask() throws {
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "Core Animation displays a layer only in a scene's window")
        }
        let (p, window) = hosted("drawn-ink", in: scene)
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Drawn by its layer"], "style": ["font_size": 24.0, "text_color": [26.0, 51.0, 153.0, 255.0]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 380.0, "h": 40.0],
            ["op": "fieldContent", "id": 2, "rect": [0.0, 0.0, 380.0, 40.0]],
        ]))
        let node = try XCTUnwrap(p.views[2])
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        let ink = try XCTUnwrap(node.textRasterLayer as? DrawnInk, "one colour on nothing: its layer draws it")
        XCTAssertNil(ink.mask, "no mask")
        XCTAssertEqual(ink.contentsScale, window.screen.scale, "at the screen's pixels")
        until { ink.contents != nil }
        // Its description ends in its format: `(buffer [786 87] A8)`, not `BGRA8888`
        let store = ink.contents.map { "\(CFCopyDescription($0 as CFTypeRef) ?? "" as CFString)" } ?? "none"
        XCTAssertTrue(store.hasSuffix(" A8)>"), "Core Animation keeps it at one byte a pixel: \(store)")
    }

    /// The context Core Animation hands the layer is its points, y down, as a
    /// drawing paints (`TextDrawing`): glyphs it paints near its top show there
    func testTheLayersContextIsItsPointsYDown() throws {
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "Core Animation displays a layer only in a scene's window")
        }
        let (_, window) = hosted("drawn-ink-axes", in: scene)
        defer { window.isHidden = true }
        let ink = DrawnInk()
        ink.frame = CGRect(x: 0, y: 0, width: 40, height: 40)
        ink.contentsScale = window.screen.scale
        ink.drawing = TextDrawing("HH", size: 12, color: UIColor.black.cgColor, baseline: 11, frame: ink.frame, scale: ink.contentsScale)
        window.layer.addSublayer(ink)
        defer { ink.removeFromSuperlayer() }
        ink.setNeedsDisplay()
        until { ink.contents != nil }
        let shot = try luminance(window)
        let width = Int(window.bounds.width)
        func inked(_ rows: Range<Int>) -> Int { rows.reduce(0) { total, y in total + (0..<40).filter { shot[y * width + $0] < 128 }.count } }
        XCTAssertGreaterThan(inked(0..<14), 10, "the glyphs at the top, where their baseline is")
        XCTAssertEqual(inked(20..<40), 0, "below them, none")
    }

    /// The raster fallback for `-exact-content-transition: numeric` rolls one
    /// colour's numerals as it rolls pixels': its layer drawn again under the
    /// roll, which shows the old number leaving and the new one coming in
    func testOneColoursNumeralsRollOnScreen() throws {
        try XCTSkipIf(UIAccessibility.isReduceMotionEnabled, "Reduce Motion: nothing rolls")
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "Core Animation displays a layer only in a scene's window")
        }
        let (p, window) = hosted("drawn-ink-roll", in: scene)
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "41"], "style": ["font_size": 120.0, "content_transition": "numeric"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 160.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        XCTAssertFalse(node.rollsNumerals, "no Numerals capability in this host: the raster rolls")
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        let ink = try XCTUnwrap(node.textRasterLayer as? DrawnInk)
        until { ink.contents != nil }
        let old = try luminance(window)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["text": "42"]]]))
        _ = p.refreshVisibleText()
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        XCTAssertTrue(node.textRasterLayer === ink, "the same layer, drawn again")
        let roll = try XCTUnwrap(ink.animation(forKey: kCATransition) as? CATransition, "it rolls")
        XCTAssertEqual(roll.subtype, .fromTop, "a rising number from above")
        CATransaction.flush()
        let start = ink.convertTime(CACurrentMediaTime(), from: nil)
        p.viewport.layer.speed = 0
        p.viewport.layer.timeOffset = start + roll.duration / 2
        CATransaction.flush()
        let middle = try luminance(window)
        p.viewport.layer.timeOffset = start + roll.duration + 1
        CATransaction.flush()
        let new = try luminance(window)
        var apart = 0, unlikeOld = 0, unlikeNew = 0
        for i in old.indices {
            apart += abs(Int(old[i]) - Int(new[i])); unlikeOld += abs(Int(middle[i]) - Int(old[i])); unlikeNew += abs(Int(middle[i]) - Int(new[i]))
        }
        func inked(_ shot: [UInt8]) -> Int { shot.filter { $0 < 128 }.count }
        XCTAssertGreaterThan(apart, 0, "the number changed")
        XCTAssertGreaterThan(unlikeOld, apart / 5, "half way, not the old number")
        XCTAssertGreaterThan(unlikeNew, apart / 5, "half way, not the new number")
        XCTAssertGreaterThan(inked(middle), min(inked(old), inked(new)) / 2, "half way, the numbers' ink, not a blank frame")
    }

    /// A layer shadow (`text-shadow`) falls from what the layer draws, as a label's does
    func testALayerShadowFallsFromTheDrawnInk() throws {
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "Core Animation displays a layer only in a scene's window")
        }
        let (p, window) = hosted("drawn-ink-shadow", in: scene)
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "Shadow"],
             "style": ["font_size": 48.0, "text_color": [0.0, 0.0, 0.0, 255.0], "text_shadow": ["o": [6.0, 6.0], "b": 0.0, "c": [255.0, 0.0, 0.0, 255.0]]]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        let ink = try XCTUnwrap(node.textRasterLayer as? DrawnInk, "one colour with a layer shadow: its layer draws it")
        XCTAssertEqual(ink.shadowOpacity, 1)
        until { ink.contents != nil }
        let shot = try rgba(window)
        let pixels = stride(from: 0, to: shot.count, by: 4)
        XCTAssertGreaterThan(pixels.filter { shot[$0] < 40 && shot[$0 + 1] < 40 && shot[$0 + 2] < 40 }.count, 100, "the words")
        XCTAssertGreaterThan(pixels.filter { shot[$0] > 200 && shot[$0 + 1] < 60 && shot[$0 + 2] < 60 }.count, 100, "their shadow, red past them")
    }

    /// A system colour on a blur is vibrant, as UIKit draws a label there: the ink is drawn by its layer in
    /// the vibrancy view's content
    func testVibrantWordsAreDrawnInkInTheVibrancyView() throws {
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "Core Animation displays a layer only in a scene's window")
        }
        let (p, window) = hosted("drawn-ink-vibrant", in: scene)
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["backgroundMaterial": "ultra-thin"], "style": ["overflow_x": "hidden", "overflow_y": "hidden"]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Vibrant words"],
             "style": ["font_size": 32.0, "text_color": [0.0, 0.0, 0.0, 255.0], "system_colors": ["text_color": "-apple-system-label"]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 380.0, "h": 60.0],
        ]))
        let node = try XCTUnwrap(p.views[2])
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        let vibrancy = try XCTUnwrap(node.vibrancyView, "a system colour on a blur is vibrant")
        let ink = try XCTUnwrap(node.textRasterLayer as? DrawnInk, "its layer draws it")
        XCTAssertTrue(sequence(first: ink as CALayer, next: { $0.superlayer }).contains { $0 === vibrancy.contentView.layer }, "in the vibrancy view's content")
        until { ink.contents != nil }
        let shot = try luminance(window), width = Int(window.bounds.width)
        let area = (15..<65).flatMap { y in (15..<385).map { shot[y * width + $0] } }
        let ground = Int(area.sorted()[area.count / 2])
        XCTAssertGreaterThan(area.filter { abs(Int($0) - ground) > 40 }.count, 100, "the words show on the material")
    }

    /// Each colour shows as it is: one sRGB colour at half its opacity is drawn by its layer and blends
    /// with what is under it, and a wide colour, which Core Animation's one-byte store was seen to narrow,
    /// is pixels in Display P3
    func testEachColourShowsAsItIs() throws {
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "Core Animation displays a layer only in a scene's window")
        }
        let (p, window) = hosted("drawn-ink-colours", in: scene)
        defer { window.isHidden = true }
        let p3Red: [String: Any] = ["cs": [["s": "display-p3", "v": [1.0, 0.0, 0.0, 1.0]]], "c": [255.0, 0.0, 0.0, 255.0]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Faint"], "style": ["font_size": 48.0, "text_color": [255.0, 0.0, 0.0, 128.0]]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Wide"], "style": ["font_size": 48.0, "text_color": p3Red]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 400.0, "h": 80.0],
        ]))
        p.paintVisibleText()
        p.textRasters.settleVisible([2, 3].compactMap { p.views[$0] })
        let faint = try XCTUnwrap(p.views[2]?.textRasterLayer as? DrawnInk, "one sRGB colour: its layer draws it")
        let wide = try XCTUnwrap(p.views[3]?.textRasterLayer)
        XCTAssertFalse(wide is DrawnInk, "a wide colour: pixels")
        XCTAssertEqual((wide.contents as! CGImage).colorSpace?.name, CGColorSpace.displayP3, "in Display P3")
        until { faint.contents != nil }
        let shot = try rgba(window), width = Int(window.bounds.width)
        func count(_ rows: Range<Int>, _ match: (UInt8, UInt8, UInt8) -> Bool) -> Int {
            rows.reduce(0) { total, y in total + (0..<width).filter { x in let i = (y * width + x) * 4; return match(shot[i], shot[i + 1], shot[i + 2]) }.count }
        }
        XCTAssertGreaterThan(count(0..<80) { r, g, b in r > 230 && (100...160).contains(g) && (100...160).contains(b) }, 100, "red at half opacity over white")
        XCTAssertGreaterThan(count(100..<180) { r, g, b in r > 200 && g < 60 && b < 60 }, 100, "red")
        XCTAssertEqual(count(0..<200) { r, g, b in r < 60 && g < 60 && b < 60 }, 0, "never black")
    }

    /// A session's presenter in `scene`'s window, whose layers Core Animation commits and displays
    private func hosted(_ label: String, in scene: UIWindowScene) -> (Presenter, UIWindow) {
        let session = ExactApp.shared.makeSession(label: label)
        addTeardownBlock { session.destroy() }
        let p = session.presenter
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 200)
        window.backgroundColor = .white
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        return (p, window)
    }

    private func until(_ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(3)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
    }

    /// The window's pixels as the screen shows them, one a point, RGBA
    private func rgba(_ window: UIWindow) throws -> [UInt8] {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1; format.opaque = true
        var drawn = false
        let shot = try XCTUnwrap(UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            drawn = window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }.cgImage)
        XCTAssertTrue(drawn, "the screen's pixels")
        var bytes = [UInt8](repeating: 0, count: shot.width * shot.height * 4)
        let context = try XCTUnwrap(CGContext(data: &bytes, width: shot.width, height: shot.height, bitsPerComponent: 8, bytesPerRow: shot.width * 4,
                                              space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(shot, in: CGRect(x: 0, y: 0, width: shot.width, height: shot.height))
        return bytes
    }

    /// The window's pixels as the screen shows them, one a point: their luminance
    private func luminance(_ window: UIWindow) throws -> [UInt8] {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1; format.opaque = true
        var drawn = false
        let shot = try XCTUnwrap(UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            drawn = window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }.cgImage)
        XCTAssertTrue(drawn, "the screen's pixels")
        var gray = [UInt8](repeating: 0, count: shot.width * shot.height)
        let context = try XCTUnwrap(CGContext(data: &gray, width: shot.width, height: shot.height, bitsPerComponent: 8, bytesPerRow: shot.width,
                                              space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue))
        context.draw(shot, in: CGRect(x: 0, y: 0, width: shot.width, height: shot.height))
        return gray
    }

    /// One colour off the measured-glyph path (a clamped line, a long one) is drawn from the lines' glyph
    /// values, taken where the lines were laid out; an underline, which CoreText draws with its line, keeps
    /// pixels
    func testTheGeneralPathDrawsGlyphValuesAndAnUnderlineKeepsPixels() throws {
        var clamped = spec
        clamped.lineClamp = 1
        clamped.runs[0].text = "Ink that the clamp cuts to one line of the box"
        let paragraph = engine.paragraph(clamped, width: 160)
        let geometry = try XCTUnwrap(engine.measuredBreaks(clamped, width: 160))
        func render(_ spec: Spec) throws -> TextRasterImage {
            let box = CGRect(x: 8, y: 8, width: 160, height: paragraph.height)
            return try XCTUnwrap(TextRasterJob(source: engine.attributed(spec), ranges: geometry.ranges, baselines: geometry.baselines, flush: 0,
                box: box, size: CGSize(width: 180, height: paragraph.height + 16), scale: 1, clamped: geometry.clamped).render())
        }
        let drawn = try render(clamped)
        guard case .drawn(let drawing) = drawn.content else { return XCTFail("one colour, clamped: its layer draws it") }
        XCTAssertFalse(drawing.lines.isEmpty)
        XCTAssertGreaterThan(try painted(drawing).bytes.enumerated().filter { $0.offset % 4 == 3 && $0.element > 16 }.count, 50, "its glyphs paint")
        var underlined = clamped
        underlined.runs[0].decoration = "underline"
        XCTAssertFalse(try render(underlined).isDrawn, "an underline keeps pixels")
    }

    /// A drawing keeps the lines whose ink reaches its frame, as a band of a tall paragraph does, and counts
    /// the bytes of the glyph values it keeps
    func testADrawingKeepsTheLinesThatReachItsFrame() throws {
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: "Ink", attributes: [.font: UIFont.boldSystemFont(ofSize: 16) as CTFont]))
        let glyphs = try XCTUnwrap(MeasuredGlyphs.Line(line, origin: 0))
        let drawing = TextDrawing([glyphs, glyphs], at: [CGPoint(x: 0, y: 20), CGPoint(x: 0, y: 2000)], frame: CGRect(x: 0, y: 0, width: 100, height: 100),
                                  scale: 3, color: UIColor.black.cgColor, smooths: true)
        XCTAssertEqual(drawing.positions, [CGPoint(x: 0, y: 20)], "the line at 2,000 points is past the band")
        XCTAssertEqual(drawing.bytes, 32 + glyphs.bytes + MemoryLayout<CGPoint>.stride)
    }

    /// Text in two colours is the rasteriser's pixels
    func testTwoColoursArePixels() throws {
        var red = spec.runs[0], blue = spec.runs[0]
        red.text = "Red "; red.color = [255, 0, 0, 255]
        blue.text = "blue"; blue.color = [0, 0, 255, 255]
        let result = try XCTUnwrap(try job(Spec(runs: [red, blue], align: 0, lineClamp: 0, color: [0, 0, 0, 255]), measured: false).render())
        XCTAssertFalse(result.isDrawn, "pixels")
    }
}
#endif
