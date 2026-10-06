import XCTest
import CoreGraphics
import QuartzCore
import CoreText
#if canImport(IOSurface)
import IOSurface
#endif
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

/// LLP 1100 D2, D8: a colour in an extended or PQ/HLG space is tagged with
/// its exposure over SDR white, and every layer a node paints an HDR colour
/// on asks for the range its `dynamic-range-limit` allows.
final class HDRColorTests: XCTestCase {
    private let fourTimesWhite: BatchValue = ["cs": [["s": "srgb-linear", "v": [4, 4, 4, 1]]], "c": [255, 255, 255, 255]]

    func testOnlyAColourPastWhiteIsHDRAndTaggedWithItsExposure() throws {
        let c = try XCTUnwrap(fourTimesWhite.cgColor(dark: false))
        XCTAssertTrue(ColorRange.isHDR(c))
        XCTAssertEqual(c.components, [4, 4, 4, 1], "components unchanged")
        if #available(iOS 26, macOS 26, tvOS 26, *) { XCTAssertEqual(c.contentHeadroom, 4, accuracy: 0.001) }
        // PQ: reference white is SDR, tagged 1 (untagged, CG implies 1000
        // nits and draws it grey); 1000 nits is HDR.
        let pq = try XCTUnwrap(CGColorSpace(name: CGColorSpace.itur_2100_PQ))
        let white = ColorRange.tagged(try XCTUnwrap(CGColor(colorSpace: pq, components: [0.58, 0.58, 0.58, 1])))
        XCTAssertFalse(ColorRange.isHDR(white))
        if #available(iOS 26, macOS 26, tvOS 26, *) { XCTAssertEqual(white.contentHeadroom, 1, accuracy: 0.05) }
        XCTAssertTrue(ColorRange.isHDR(try XCTUnwrap(CGColor(colorSpace: pq, components: [0.75, 0.75, 0.75, 1]))))
        let p3: BatchValue = ["cs": [["s": "display-p3", "v": [1, 0, 0, 1]]], "c": [255, 0, 0, 255]]
        XCTAssertFalse(ColorRange.isHDR(try XCTUnwrap(p3.cgColor(dark: false))), "wide, within white")
        XCTAssertFalse(ColorRange.isHDR(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1)))
    }

    func testALayerAsksForTheRangeTheLimitAllows() throws {
        guard #available(iOS 26, macOS 26, tvOS 26, *) else { throw XCTSkip("preferredDynamicRange is iOS/macOS 26") }
        let layer = CALayer()
        layer.backgroundColor = try XCTUnwrap(fourTimesWhite.cgColor(dark: false))
        for (limit, range) in [("no-limit", CALayer.DynamicRange.high), ("constrained", .constrainedHigh), ("standard", .standard)] {
            layer.applyColorRange(limit: limit)
            XCTAssertEqual(layer.preferredDynamicRange, range, limit)
        }
        layer.applyColorRange(limit: nil)
        XCTAssertEqual(layer.preferredDynamicRange, .high, "CSS's initial value is no-limit")
        layer.backgroundColor = CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)
        layer.applyColorRange(limit: "no-limit")
        XCTAssertEqual(layer.preferredDynamicRange, .standard, "an SDR fill is never marked")
        // A gradient to four times SDR white is mapped to the range only under ifSupported.
        let gradient = try XCTUnwrap(Gradient(["linear": 90, "space": "srgb-linear", "stops": [0, 1, 1, 1, 1, 1, 4, 4, 4, 1]] as BatchValue))
        let g = CAGradientLayer()
        gradient.apply(g, bounds: CGRect(x: 0, y: 0, width: 100, height: 10), box: CGRect(x: 0, y: 0, width: 100, height: 10), dark: false, limit: "constrained")
        XCTAssertEqual(g.toneMapMode, .ifSupported)
    }

    /// Text in an HDR colour keeps its light: the raster is half float in
    /// extended sRGB and says its headroom; SDR text stays four bytes.
    func testHDRTextRastersAtHalfFloatWithItsHeadroom() throws {
        func raster(_ color: CGColor) throws -> TextRasterImage {
            let font = CTFontCreateWithName("Helvetica" as CFString, 20, nil)
            let source = NSAttributedString(string: "HDR", attributes: [.font: font, .foregroundColor: color])
            let box = CGRect(x: 0, y: 0, width: 80, height: 30)
            let job = TextRasterJob(source: source, ranges: [CFRange(location: 0, length: 3)], baselines: [22],
                                    flush: 0, box: box, size: box.size, scale: 2)
            return try XCTUnwrap(job.render())
        }
        let hdr = try raster(try XCTUnwrap(fourTimesWhite.cgColor(dark: false)))
        XCTAssertEqual(hdr.headroom, 4, accuracy: 0.001)
        let sdr = try raster(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1))
        XCTAssertEqual(sdr.headroom, 0)
        #if os(macOS)
        XCTAssertEqual(IOSurfaceGetBytesPerElement(hdr.surface), 8)
        XCTAssertEqual(TextRasterJob.headroom(of: hdr.surface), 4, accuracy: 0.001, "kIOSurfaceContentHeadroom")
        XCTAssertEqual(TextRasterJob.headroom(of: sdr.surface), 0)
        #else
        XCTAssertEqual(hdr.image.bitsPerComponent, 16)
        if #available(iOS 18, tvOS 18, *) { XCTAssertEqual(hdr.image.contentHeadroom, 4, accuracy: 0.001) }
        XCTAssertEqual(sdr.image.bitsPerComponent, 8)
        #endif
    }

    /// A text layer, a border or a shadow, and an SVG scene's shapes take the
    /// node's limit for their HDR colours; SDR ones stay standard.
    func testEveryPaintLayerTakesTheLimit() throws {
        guard #available(iOS 26, macOS 26, tvOS 26, *) else { throw XCTSkip("preferredDynamicRange is iOS/macOS 26") }
        let hdr = try XCTUnwrap(fourTimesWhite.cgColor(dark: false))
        let ink = CALayer()
        ink.applyTextRange(headroom: 4, limit: "constrained")
        XCTAssertEqual(ink.preferredDynamicRange, .constrainedHigh)
        XCTAssertEqual(ink.contentsHeadroom, 4, accuracy: 0.001)
        ink.applyTextRange(limit: "standard")
        XCTAssertEqual(ink.preferredDynamicRange, .standard, "a later limit, the same ink")
        let border = CALayer()
        border.borderWidth = 2; border.borderColor = hdr
        border.applyColorRange(limit: "no-limit")
        XCTAssertEqual(border.preferredDynamicRange, .high)
        let caster = CALayer(), shadow = CALayer()
        shadow.shadowOpacity = 1; shadow.shadowColor = hdr
        caster.addSublayer(shadow)
        caster.applyColorRange(limit: "constrained", deep: true)
        XCTAssertEqual(shadow.preferredDynamicRange, .constrainedHigh)
        XCTAssertEqual(caster.preferredDynamicRange, .standard, "the container paints nothing")
        let scene = CALayer(), stroke = CAShapeLayer(), plain = CAShapeLayer()
        stroke.fillColor = nil; stroke.strokeColor = hdr
        plain.fillColor = CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)
        scene.addSublayer(stroke); scene.addSublayer(plain)
        scene.applyColorRange(limit: "no-limit", deep: true)
        XCTAssertEqual(stroke.preferredDynamicRange, .high)
        XCTAssertEqual(plain.preferredDynamicRange, .standard)
    }

    /// An SVG fill or stroke in a colour's own space is a colour, not a paint server.
    func testAnSVGColourInItsOwnSpaceIsNotAPaintServer() {
        let wide: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4, 4, 4, 1]]]]
        XCTAssertNil(SvgPaint.server(wide))
        XCTAssertFalse(SvgPaint.needsParts(["f": wide, "s": NSNull()]))
        XCTAssertNotNil(SvgPaint.server(["x1": 0, "stops": []]))
        XCTAssertTrue(SvgPaint.needsParts(["f": ["x1": 0, "stops": []]]))
    }

    /// One channel of a raster's premultiplied pixels (0 red … 3 alpha), row by row.
    #if os(macOS)
    private func channel(_ index: Int, of pixels: IOSurface) -> (w: Int, h: Int, v: [Float]) {
        pixels.lock(options: .readOnly, seed: nil); defer { pixels.unlock(options: .readOnly, seed: nil) }
        return channel(index, base: pixels.baseAddress, w: pixels.width, h: pixels.height, row: pixels.bytesPerRow, deep: pixels.bytesPerElement == 8)
    }
    #else
    private func channel(_ index: Int, of pixels: CGImage) -> (w: Int, h: Int, v: [Float]) {
        let data = pixels.dataProvider!.data! as Data
        return data.withUnsafeBytes { channel(index, base: $0.baseAddress!, w: pixels.width, h: pixels.height, row: pixels.bytesPerRow, deep: pixels.bitsPerComponent == 16) }
    }
    #endif
    private func channel(_ index: Int, base: UnsafeRawPointer, w: Int, h: Int, row: Int, deep: Bool) -> (w: Int, h: Int, v: [Float]) {
        var out = [Float](repeating: 0, count: w * h)
        for y in 0..<h {
            for x in 0..<w {
                // Half float is RGBA; four bytes is BGRA.
                out[y * w + x] = deep
                    ? Float(Float16(bitPattern: base.loadUnaligned(fromByteOffset: y * row + x * 8 + index * 2, as: UInt16.self)))
                    : Float(base.load(fromByteOffset: y * row + x * 4 + (index == 3 ? 3 : 2 - index), as: UInt8.self)) / 255
            }
        }
        return (w, h, out)
    }

    /// How a cast's pixels compare with its ink moved by a hard shadow's
    /// offset (`by` pixels): lit, lit where no ink casts, and unlit where ink does.
    private func compare(_ image: TextRasterImage, by offset: Int) throws -> (lit: Int, stray: Int, missing: Int) {
        #if os(macOS)
        let ink = channel(3, of: image.surface)
        #else
        let ink = channel(3, of: image.image)
        #endif
        let cast = channel(3, of: try XCTUnwrap(image.cast))
        XCTAssertEqual(ink.w, cast.w); XCTAssertEqual(ink.h, cast.h)
        var lit = 0, stray = 0, missing = 0
        for y in 0..<cast.h {
            for x in 0..<cast.w {
                let c = cast.v[y * cast.w + x]
                let i = x >= offset && y >= offset ? ink.v[(y - offset) * ink.w + x - offset] : 0
                if c > 0.5 { lit += 1; if i < 0.05 { stray += 1 } }
                if i > 0.95 && c < 0.05 { missing += 1 }
            }
        }
        return (lit, stray, missing)
    }

    private func shadowJob(_ text: NSAttributedString, shadow: TextRunShadow? = nil) -> TextRasterJob {
        let box = CGRect(x: 0, y: 0, width: 80, height: 30)
        return TextRasterJob(source: text, ranges: [CFRange(location: 0, length: text.length)], baselines: [22],
                             flush: 0, box: box, size: box.size, scale: 2, crop: true, shadow: shadow)
    }
    private func white(_ text: String) -> NSMutableAttributedString {
        NSMutableAttributedString(string: text, attributes: [.font: CTFontCreateWithName("Helvetica" as CFString, 20, nil),
                                                             .foregroundColor: CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1)])
    }

    /// A `text-shadow` keeps its colour's space (`textChannels`' nine after
    /// the offset and blur). An HDR one is painted into a bitmap of its own,
    /// with its light, and the SDR ink's stays SDR.
    func testAnHDRTextShadowIsPaintedInTheRaster() throws {
        let nine = try XCTUnwrap(fourTimesWhite.textChannels(dark: false))
        XCTAssertEqual(nine.count, 9)
        let shadow = [3, 3, 0] + nine
        XCTAssertTrue(TextEngine.isShadow(shadow))
        XCTAssertTrue(ColorRange.isHDR(TextRunShadow(shadow).color))
        XCTAssertFalse(ColorRange.isHDR(TextEngine.shadowColor([0, 0, 6, 255, 255, 255, 255])))
        let plain = try XCTUnwrap(shadowJob(white("HDR")).render())
        let cast = try XCTUnwrap(shadowJob(white("HDR"), shadow: TextRunShadow(shadow)).render())
        XCTAssertNil(plain.cast)
        XCTAssertEqual(cast.headroom, 0, "the ink")
        XCTAssertEqual(cast.castHeadroom, 4, accuracy: 0.001)
        XCTAssertGreaterThan(cast.frame.maxX, plain.frame.maxX, "the frame reaches the shadow")
        // The cast is the ink's shape, three points (six pixels) down and right, at four times white.
        let pixels = try compare(cast, by: 6)
        XCTAssertGreaterThan(pixels.lit, 500)
        XCTAssertEqual(pixels.stray, 0)
        XCTAssertEqual(pixels.missing, 0)
        // Four times white, as extended sRGB encodes it.
        XCTAssertEqual(try XCTUnwrap(channel(0, of: try XCTUnwrap(cast.cast)).v.max()), 1.055 * pow(4, 1 / 2.4) - 0.055, accuracy: 0.01)
        // A line running far past its box casts only what its pixels hold.
        let long = try XCTUnwrap(shadowJob(white(String(repeating: "The quick brown fox jumps over the lazy dog. ", count: 40)),
                                           shadow: TextRunShadow(shadow)).render())
        let far = try compare(long, by: 6)
        XCTAssertGreaterThan(far.lit, 5000)
        XCTAssertEqual(far.stray, 0, "no glyph's ink in the shadow's bitmap")
        XCTAssertEqual(far.missing, 0)
        // A blurred shadow spreads past the glyphs it is under.
        let soft = try XCTUnwrap(shadowJob(white("HDR"), shadow: TextRunShadow([0, 0, 8] + nine)).render())
        let alpha = channel(3, of: try XCTUnwrap(soft.cast)).v
        XCTAssertGreaterThan(try XCTUnwrap(alpha.max()), 0.2)
        XCTAssertGreaterThan(alpha.filter { $0 > 0.02 }.count, pixels.lit * 2)
    }

    /// A run's own HDR shadow is in the cast too, and the ink stays SDR:
    /// under `standard` the other runs' white is not mapped with it.
    func testARunsOwnHDRShadowIsPaintedApartFromTheInk() throws {
        let nine = try XCTUnwrap(fourTimesWhite.textChannels(dark: false))
        let text = white("HDR sdr")
        text.addAttribute(.exactShadow, value: TextRunShadow([3, 3, 0] + nine), range: NSRange(location: 0, length: 3))
        let image = try XCTUnwrap(shadowJob(text).render())
        XCTAssertEqual(image.headroom, 0, "the ink is four bytes a pixel")
        XCTAssertEqual(image.castHeadroom, 4, accuracy: 0.001)
        let pixels = try compare(image, by: 6)
        XCTAssertGreaterThan(pixels.lit, 500)
        XCTAssertEqual(pixels.stray, 0, "only ink casts")
        XCTAssertGreaterThan(pixels.missing, 200, "the run with no shadow casts none")
        // An SDR shadow of a run's own stays in the ink.
        let sdr = white("HDR sdr")
        sdr.addAttribute(.exactShadow, value: TextRunShadow([3, 3, 0, 255, 0, 0, 255]), range: NSRange(location: 0, length: 3))
        XCTAssertNil(try XCTUnwrap(shadowJob(sdr).render()).cast)
    }

    /// The cast's layer sits under the ink's and takes the limit on its own.
    func testAnHDRTextShadowsLayerFollowsTheLimit() throws {
        let nine = try XCTUnwrap(fourTimesWhite.textChannels(dark: false))
        let image = try XCTUnwrap(shadowJob(white("HDR"), shadow: TextRunShadow([3, 3, 0] + nine)).render())
        let ink = CALayer(), parent = CALayer()
        parent.addSublayer(ink)
        ink.applyTextRange(headroom: image.headroom, limit: "standard")
        ink.applyTextCast(image.cast, headroom: image.castHeadroom, limit: "standard")
        let cast = try XCTUnwrap(ink.textCast)
        XCTAssertTrue(parent.sublayers?.first === cast, "under the ink")
        if #available(iOS 18, macOS 15, tvOS 18, *) {
            XCTAssertEqual(cast.toneMapMode, .ifSupported)
            XCTAssertEqual(ink.toneMapMode, .automatic, "SDR ink is not mapped")
        }
        if #available(iOS 26, macOS 26, tvOS 26, *) {
            XCTAssertEqual(cast.preferredDynamicRange, .standard)
            XCTAssertEqual(cast.contentsHeadroom, 4, accuracy: 0.001)
            ink.applyTextRange(limit: "no-limit")
            XCTAssertEqual(cast.preferredDynamicRange, .high, "a later limit reaches the cast")
            XCTAssertEqual(ink.preferredDynamicRange, .standard)
        }
        ink.dropTextCast()
        XCTAssertEqual(parent.sublayers?.count, 1)
    }

    /// A tall paragraph's band admits an HDR paragraph shadow, which is in
    /// its pixels; an SDR one is the layer's, cast outside them.
    func testATallParagraphsBandAdmitsItsHDRShadow() throws {
        let nine = try XCTUnwrap(fourTimesWhite.textChannels(dark: false))
        let run = Run(text: "tall", size: 16, weight: 400, family: 0, italic: false, lineHeight: 20, letterSpacing: 0)
        var spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [255, 255, 255, 255])
        let port = CGRect(x: 0, y: 5000, width: 300, height: 800)
        func band() -> CGRect { TextRasterJob.band(spec, width: 300, port: port, scale: 2, maximumBytes: 16 * 1024 * 1024) }
        let none = band()
        spec.shadow = [80, 0, 0, 255, 255, 255, 255]
        XCTAssertEqual(band(), none, "an SDR shadow is the layer's")
        spec.shadow = [80, 0, 0] + nine
        XCTAssertEqual(band().maxX, none.maxX + 81, accuracy: 0.001)
        XCTAssertEqual(band().minX, none.minX, accuracy: 0.001)
    }

    #if os(macOS)
    /// On macOS a paragraph too small to repay a surface draws in its layer,
    /// unless it is HDR: only a raster's layers follow the limit.
    func testSmallHDRTextRastersOnMacOS() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-small-hdr")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        let hdr: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4.0, 4.0, 4.0, 1.0]]], "c": [255.0, 255.0, 255.0, 255.0]]
        presenter.apply(batchFixture(ops: [
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "SDR"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "HDR"], "style": ["text_color": hdr]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "HDR"], "style": ["text_shadow": ["o": [3.0, 3.0], "b": 0.0, "c": hdr]]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 500.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 48.0, "h": 24.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 48.0, "h": 24.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 80.0, "w": 48.0, "h": 24.0],
        ], timers: false, motion: false, clock: nil, error: nil))
        let sdr = try XCTUnwrap(presenter.views[2]), ink = try XCTUnwrap(presenter.views[3]), shadow = try XCTUnwrap(presenter.views[4])
        XCTAssertTrue(sdr.textIsSmall)
        XCTAssertFalse(sdr.rastersText)
        XCTAssertTrue(ink.rastersText, "HDR ink")
        XCTAssertTrue(shadow.rastersText, "an HDR shadow")
        XCTAssertTrue(presenter.textRasters.ensure(shadow, urgent: true))
        let layer = try XCTUnwrap(shadow.textRasterOverflowLayer)
        XCTAssertNotNil(layer.textCast, "the shadow's own layer")
        if #available(macOS 15, *) { XCTAssertEqual(layer.textCast?.toneMapMode, .ifSupported) }
    }
    #endif

    /// What a view draws itself: an HDR colour makes its backing store half
    /// float, its layer saying the headroom; back to SDR, eight bits again.
    func testADrawnHDRColourGetsAHalfFloatStore() {
        let layer = CALayer()
        XCTAssertTrue(layer.applyDrawnRange(headroom: 4, limit: "constrained"))
        XCTAssertEqual(layer.contentsFormat, .RGBA16Float)
        if #available(iOS 26, macOS 26, tvOS 26, *) {
            XCTAssertEqual(layer.preferredDynamicRange, .constrainedHigh)
            XCTAssertEqual(layer.contentsHeadroom, 4, accuracy: 0.001)
        }
        XCTAssertFalse(layer.applyDrawnRange(headroom: 4, limit: "constrained"), "no change, no redraw")
        XCTAssertTrue(layer.applyDrawnRange(headroom: 0, limit: "constrained"))
        XCTAssertEqual(layer.contentsFormat, .RGBA8Uint)
        let other = CALayer()
        other.contentsFormat = .gray8Uint
        XCTAssertFalse(other.applyDrawnRange(headroom: 0, limit: nil), "a format it didn't set is left alone")
        XCTAssertEqual(other.contentsFormat, .gray8Uint)
    }
    func testGradientLayersPreserveWideAndHDRStops() throws {
        guard #available(iOS 26, macOS 26, tvOS 26, *) else { throw XCTSkip("layer range needs 26") }
        for components: [Double] in [[1.225, -0.042, -0.02, 1], [4, 4, 4, 1]] {
            let stops = [0.0] + components + [1.0] + components
            let row: BatchValue = ["linear": 180, "space": "srgb-linear", "stops": .array(stops.map(BatchValue.number))]
            let gradient = try XCTUnwrap(Gradient(row)), layer = CAGradientLayer()
            let rect = CGRect(x: 0, y: 0, width: 100, height: 100)
            gradient.apply(layer, bounds: rect, box: rect, dark: false)
            XCTAssertEqual(layer.preferredDynamicRange, .high)
            let color = try XCTUnwrap((layer.colors as? [CGColor])?.first)
            XCTAssertEqual(color.components, components.map { CGFloat($0) })
            XCTAssertGreaterThanOrEqual(color.contentHeadroom, 1)
            for (limit, range) in [("standard", CALayer.DynamicRange.standard), ("constrained", .constrainedHigh)] {
                layer.applyColorRange(limit: limit)
                XCTAssertEqual(layer.preferredDynamicRange, range)
            }
            let drawn = CALayer()
            XCTAssertTrue(drawn.applyDrawnRange(headroom: ColorRange.headroom(color), extended: ColorRange.needsExtendedComponents(color), limit: "no-limit"))
            XCTAssertEqual(drawn.contentsFormat, .RGBA16Float)
            XCTAssertEqual(drawn.preferredDynamicRange, .high)
        }
    }

    func testDrawnGradientAndBoxGradientTakeTheNodesLimit() throws {
        guard #available(iOS 26, macOS 26, tvOS 26, *) else { throw XCTSkip("layer range needs 26") }
        let presenter = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: presenter)
        node.frame = CGRect(x: 0, y: 0, width: 100, height: 100)
        #if os(macOS)
        node.wantsLayer = true
        node.layer = CALayer()
        #endif
        presenter.root.addSubview(node)
        let linear: BatchValue = ["linear": 180, "space": "srgb-linear", "stops": [0, 1.225, -0.042, -0.02, 1, 1, 1.225, -0.042, -0.02, 1]]
        node.applyStyle(["background_image": linear, "dynamic_range_limit": "constrained"])
        #if os(macOS)
        node.applyLayerPaint()
        #else
        node.applyGradientLayer()
        #endif
        node.applyColorRanges()
        XCTAssertEqual(try XCTUnwrap(node.boxGradient).preferredDynamicRange, .constrainedHigh)
        XCTAssertEqual(try XCTUnwrap(node.boxGradient).toneMapMode, .automatic, "wide but SDR")
        let conic: BatchValue = ["conic": [0, 50, 0, 50, 0], "space": "srgb-linear", "stops": [0, 1.225, -0.042, -0.02, 1, 1, 1.225, -0.042, -0.02, 1]]
        node.applyStyle(["background_image": .array([conic, linear]), "dynamic_range_limit": "no-limit"])
        node.applyColorRanges()
        #if os(macOS)
        let layer = try XCTUnwrap(node.layer)
        #else
        let layer = node.layer
        #endif
        XCTAssertEqual(layer.contentsFormat, .RGBA16Float)
        XCTAssertEqual(layer.preferredDynamicRange, .high)
    }

}
