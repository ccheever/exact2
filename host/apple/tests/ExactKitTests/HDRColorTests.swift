import XCTest
import CoreGraphics
import QuartzCore
import CoreText
#if canImport(IOSurface)
import IOSurface
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

    /// A `text-shadow` keeps its colour's space (`textChannels`' nine after
    /// the offset and blur). An HDR one is painted into a bitmap of its own,
    /// with its headroom, and the SDR ink's stays SDR.
    func testAnHDRTextShadowIsPaintedInTheRaster() throws {
        let nine = try XCTUnwrap(fourTimesWhite.textChannels(dark: false))
        XCTAssertEqual(nine.count, 9)
        let shadow = [3, 3, 0] + nine
        XCTAssertTrue(TextEngine.isShadow(shadow))
        XCTAssertTrue(ColorRange.isHDR(TextRunShadow(shadow).color))
        XCTAssertFalse(ColorRange.isHDR(TextEngine.shadowColor([0, 0, 6, 255, 255, 255, 255])))
        let font = CTFontCreateWithName("Helvetica" as CFString, 20, nil)
        let source = NSAttributedString(string: "HDR", attributes: [.font: font, .foregroundColor: CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1)])
        let box = CGRect(x: 0, y: 0, width: 80, height: 30)
        let plain = try XCTUnwrap(TextRasterJob(source: source, ranges: [CFRange(location: 0, length: 3)], baselines: [22],
                                                flush: 0, box: box, size: box.size, scale: 2, crop: true).render())
        let cast = try XCTUnwrap(TextRasterJob(source: source, ranges: [CFRange(location: 0, length: 3)], baselines: [22],
                                               flush: 0, box: box, size: box.size, scale: 2, crop: true, shadow: TextRunShadow(shadow)).render())
        XCTAssertNil(plain.cast)
        XCTAssertEqual(cast.headroom, 0, "the ink")
        XCTAssertNotNil(cast.cast)
        XCTAssertEqual(cast.castHeadroom, 4, accuracy: 0.001)
        XCTAssertGreaterThan(cast.frame.maxX, plain.frame.maxX, "the frame reaches the shadow")
        let ink = CALayer(), parent = CALayer()
        parent.addSublayer(ink)
        ink.applyTextCast(cast.cast, headroom: cast.castHeadroom, limit: "standard")
        XCTAssertTrue(parent.sublayers?.first === ink.textCast, "under the ink")
        ink.dropTextCast()
        XCTAssertEqual(parent.sublayers?.count, 1)
    }

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
