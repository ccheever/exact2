#if os(iOS)
import UIKit
import ImageIO
import CExact
import XCTest
@testable import ExactKit

/// A box Core Animation can paint keeps no bitmap: background, one radius
/// over a mask of corners, and a uniform border are layer properties; a
/// node with no paint (a collection's spacer) has no contents at all. Only
/// what the layer cannot say still draws. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class BoxLayerIOSTests: XCTestCase {
    func testAnUnevenImageBackgroundStaysBehindItsDrawnPixels() throws {
        let format = UIGraphicsImageRendererFormat(); format.scale = 1
        let png = try XCTUnwrap(UIGraphicsImageRenderer(size: CGSize(width: 80, height: 40), format: format).image { context in
            UIColor.red.setFill(); context.fill(CGRect(x: 0, y: 0, width: 40, height: 40))
        }.pngData())
        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: URL(fileURLWithPath: NSTemporaryDirectory()))
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        defer { loader.shutdown(); window.isHidden = true }
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        let image = NodeView(id: 1, kind: "image", presenter: p)
        p.views[1] = image; p.viewport.addSubview(image)
        image.frame = CGRect(x: 0, y: 0, width: 80, height: 40)
        image.loadGeneration = 1
        loader.load(image, source: "data:image/png;base64," + png.base64EncodedString(), resolver: resolver)
        let deadline = Date(timeIntervalSinceNow: 5)
        while image.raster == nil, Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertNotNil(image.raster)
        for alpha in [255, 128] {
            image.applyStyle(["background_color": [0, 0, 255, .number(Double(alpha))],
                "border_radius_top_left": 18, "border_radius_top_right": 18,
                "border_radius_bottom_left": 18, "border_radius_bottom_right": 4])
            image.layer.displayIfNeeded()
            let context = try XCTUnwrap(CGContext(data: nil, width: 80, height: 40, bitsPerComponent: 8, bytesPerRow: 320,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            image.layer.render(in: context)
            let bytes = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
            func pixel(_ x: Int, _ y: Int) -> [UInt8] { Array(UnsafeBufferPointer(start: bytes + y * 320 + x * 4, count: 4)) }
            XCTAssertEqual(pixel(20, 20), [255, 0, 0, 255], "the background must not cover or tint the opaque image")
            XCTAssertEqual(pixel(60, 20), [0, 0, UInt8(alpha), UInt8(alpha)], "the transparent image leaves the background visible")
            XCTAssertEqual(pixel(0, 0), [0, 0, 0, 0], "the corner remains transparent")
        }
    }

    func testPercentageRadiusUsesBothAxesAndFollowsResize() {
        var style: NodeStyle = ["background_color": [36, 104, 172, 255]]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        XCTAssertEqual(n.cornerSizes(in: n.bounds), Array(repeating: CGSize(width: 80, height: 40), count: 4))
        XCTAssertFalse(n.roundedPath(in: n.bounds).cgPath.contains(CGPoint(x: 20, y: 5)))
        XCTAssertTrue(n.roundedPath(in: n.bounds).cgPath.contains(CGPoint(x: 80, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        XCTAssertEqual(n.cornerSizes(in: n.bounds), Array(repeating: CGSize(width: 40, height: 80), count: 4))
        XCTAssertNil(FlatPaint(style), "percentage geometry cannot use a fixed-radius flat leaf")
    }

    /// The radius retains its viewport and resolves percentages per axis.
    func testACalculatedRadiusReadsTheKernelOnEachAxis() throws {
        let radius = try XCTUnwrap(CalculatedLength([
            "calc": .number(calc("min(50%, 10vw)")),
            "env": [0, 0, 0, 0, 300, 600, 1, 1, 0, 0]
        ]))
        var style: NodeStyle = ["background_color": [36, 104, 172, 255]]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = .calculation(radius)
        }
        let n = node(style, size: CGSize(width: 160, height: 40))
        XCTAssertEqual(n.cornerSizes(in: n.bounds), Array(repeating: CGSize(width: 30, height: 20), count: 4))
        XCTAssertNil(FlatPaint(style), "a calculated radius needs the final box")
        // Cancellation precedes the final conversion to layout precision.
        let steep = BatchValue.calculation(try XCTUnwrap(CalculatedLength([
            "calc": .number(calc("calc(1e9 * (min(0.1%, 1px) - 0.2px) + 10px)")),
            "env": [0, 0, 0, 0, 300, 600, 1, 1, 0, 0]
        ])))
        XCTAssertEqual(steep.length(basis: 200), 10)
    }

    func testPercentageRadiusClipsMaterialAndOverflowAfterResize() throws {
        var style: NodeStyle = ["overflow_x": "hidden", "overflow_y": "hidden"]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        n.props["backgroundMaterial"] = "ultra-thin"
        n.updateMaterial()
        n.applyBoxLayer()
        let material = try XCTUnwrap(n.materialView)
        let materialMask = try XCTUnwrap(material.layer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(materialMask.path).contains(CGPoint(x: 20, y: 5)))
        XCTAssertTrue(try XCTUnwrap(materialMask.path).contains(CGPoint(x: 80, y: 5)))
        let clip = try XCTUnwrap(n.layer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(clip.path).contains(CGPoint(x: 20, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        n.setNeedsLayout(); n.layoutIfNeeded()
        let resized = try XCTUnwrap(material.layer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 20)))
        XCTAssertTrue(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 80)))
        n.applyStyle([:])
        n.updateMaterial()
        n.applyBoxLayer()
        XCTAssertNil(material.layer.mask, "a later square style removes the elliptical clip")
        XCTAssertNil(n.layer.mask)
    }

    /// Corners that differ (a chat bubble's 4 where it joins its run, 18
    /// elsewhere) clip its children through their outline: one Core
    /// Animation radius cannot say them, and the children overflowed square.
    func testOverflowWithCornersThatDifferClipsThroughTheirOutline() throws {
        var style: NodeStyle = ["overflow_x": "hidden", "overflow_y": "hidden", "background_color": [233, 233, 233, 255]]
        style["border_radius_top_left"] = 4.0
        for corner in ["top_right", "bottom_right", "bottom_left"] { style["border_radius_" + corner] = 18.0 }
        let n = node(style, size: CGSize(width: 264, height: 200))
        n.applyBoxLayer()
        n.applyBoxMask()
        let clip = try XCTUnwrap((n.clipBox?.layer.mask ?? n.layer.mask) as? CAShapeLayer, "the children are clipped by a shaped mask")
        let path = try XCTUnwrap(clip.path)
        XCTAssertFalse(path.contains(CGPoint(x: 1, y: 199)), "the 18-point bottom-left corner is cut")
        XCTAssertTrue(path.contains(CGPoint(x: 1.5, y: 1.5)), "the 4-point corner keeps nearly all")
    }

    func testVideoPercentageRadiusUsesTheContentEdgeAndClears() throws {
        var style: NodeStyle = ["padding_left": 10, "padding_top": 10, "padding_right": 10, "padding_bottom": 10]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        let media = MediaPlatformView()
        n.applyContentBox(padding: [10, 10, 10, 10])
        VideoView.layout(media, in: n)
        XCTAssertEqual(media.frame, CGRect(x: 10, y: 10, width: 140, height: 60))
        let mask = try XCTUnwrap(media.layer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 10, y: 5)))
        XCTAssertTrue(try XCTUnwrap(mask.path).contains(CGPoint(x: 70, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        n.applyContentBox(padding: [10, 10, 10, 10])
        VideoView.layout(media, in: n)
        XCTAssertEqual(media.frame, CGRect(x: 10, y: 10, width: 60, height: 140))
        let resized = try XCTUnwrap(media.layer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 10)))
        n.applyStyle([:])
        VideoView.layout(media, in: n)
        XCTAssertNil(media.layer.mask)
    }

    func testPercentageOverflowClipComposesWithAuthoredClipPath() throws {
        var style: NodeStyle = ["overflow_x": "hidden", "overflow_y": "hidden",
            "clip_path": ["commands": [["M", [0, 0]], ["L", [80, 0]], ["L", [80, 80]], ["L", [0, 80]], ["Z", []]]]]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        n.applyBoxLayer()
        let mask = try XCTUnwrap(n.layer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 20, y: 5)))
        let authored = try XCTUnwrap(mask.mask as? CAShapeLayer)
        XCTAssertTrue(try XCTUnwrap(authored.path).contains(CGPoint(x: 60, y: 40)))
        XCTAssertFalse(try XCTUnwrap(authored.path).contains(CGPoint(x: 100, y: 40)))
        let layer = n.layer
        func renderedAlpha(_ target: CALayer, _ x: Int, _ y: Int) -> UInt8 {
            let width = Int(target.bounds.width), height = Int(target.bounds.height)
            let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            target.render(in: context)
            // Raw CGContext bitmap rows run bottom-up, independently of the
            // UIKit layer's isGeometryFlipped flag. Check an unmasked reference.
            return context.data!.assumingMemoryBound(to: UInt8.self)[((height - 1 - y) * width + x) * 4 + 3]
        }
        let reference = CALayer()
        reference.frame = CGRect(x: 0, y: 0, width: 160, height: 80)
        let top = CALayer()
        top.frame = CGRect(x: 0, y: 0, width: 160, height: 20)
        top.backgroundColor = CGColor(gray: 1, alpha: 1)
        reference.addSublayer(top)
        XCTAssertEqual(renderedAlpha(reference, 40, 10), 255)
        XCTAssertEqual(renderedAlpha(reference, 40, 60), 0)
        func alpha(_ x: Int, _ y: Int) -> UInt8 {
            // An opaque descendant proves both masks clip real rendered content.
            let child = CALayer()
            child.frame = n.bounds
            child.backgroundColor = CGColor(gray: 1, alpha: 1)
            layer.addSublayer(child)
            defer { child.removeFromSuperlayer() }
            return renderedAlpha(layer, x, y)
        }
        XCTAssertEqual(alpha(20, 5), 0, "outside ellipse")
        XCTAssertEqual(alpha(60, 40), 255, "inside both masks")
        XCTAssertEqual(alpha(100, 40), 0, "outside authored clip")
        n.frame.size = CGSize(width: 80, height: 160)
        n.applyBoxLayer()
        XCTAssertEqual(alpha(5, 20), 0, "resized ellipse")
        XCTAssertEqual(alpha(40, 40), 255)
        XCTAssertEqual(alpha(40, 120), 0, "authored clip survives resize")
        n.applyStyle([:])
        n.applyBoxLayer()
        XCTAssertNil(layer.mask)
        XCTAssertEqual(alpha(5, 5), 255, "removing the style clears both masks")
    }

    func testVideoStyleChangesRefreshGeometryWithoutPropChanges() throws {
        let n = node([:], size: CGSize(width: 160, height: 80))
        let video = VideoView(owner: n)
        n.video = video
        guard let media = n.subviews.first else {
            throw missing("module", "the video module is not beside this test runner (EXACT_VIDEO_DYLIB)")
        }
        video.update()
        var style: NodeStyle = ["padding_left": 10, "padding_top": 10, "padding_right": 10, "padding_bottom": 10]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        n.applyStyle(style)
        n.applyContentBox(padding: [10, 10, 10, 10])
        XCTAssertEqual(media.frame, CGRect(x: 10, y: 10, width: 140, height: 60))
        let mask = try XCTUnwrap(media.layer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 10, y: 5)))
        n.applyStyle([:])
        n.applyFieldContent(["rect": NSNull()])
        XCTAssertEqual(media.frame, n.bounds)
        XCTAssertNil(media.layer.mask)
        n.applyStyle(style)
        XCTAssertNotNil(media.layer.mask)
    }

    private let white: BatchValue = [255, 255, 255, 255]
    private let blue: BatchValue = [0, 136, 255, 255]

    /// LLP 1116: a padding that resolves at layout (`min(10%, 20px)`
    /// arrives as a calculation's handle) reads the content box the kernel published,
    /// for the box and for each side; without one, the box's bounds.
    func testAPaddingResolvedAtLayoutReadsTheKernelsContentBox() throws {
        let padding = try XCTUnwrap(CalculatedLength([
            "calc": .number(calc("min(10%, 20px)")),
            "env": [0, 0, 0, 0, 300, 600, 1, 1, 0, 0]
        ]))
        let knots: NodeStyle = ["padding_top": .calculation(padding)]
        var style = knots
        style["padding_left"] = 6; style["border_width"] = 2
        let n = node(style, size: CGSize(width: 300, height: 120))
        n.applyContentBox(padding: [20, 0, 0, 6], border: [2, 2, 2, 2])
        XCTAssertEqual(n.contentBox(), CGRect(x: 8, y: 22, width: 290, height: 96))
        XCTAssertEqual(["top", "right", "bottom", "left"].map { n.padding($0) }, [20, 0, 0, 6])
        n.applyFieldContent(["rect": NSNull()])
        XCTAssertEqual(n.contentBox(), CGRect(x: 0, y: 0, width: 300, height: 120), "a box sent none is its own content box")
    }

    /// The kernel's handle for a CSS length text that is a calculation.
    private func calc(_ text: String) -> Double {
        let bytes = Array(text.utf8)
        return Double(bytes.withUnsafeBufferPointer { exact_calc_parse($0.baseAddress, $0.count) })
    }

    private func node(_ style: NodeStyle, size: CGSize = CGSize(width: 300, height: 120)) -> NodeView {
        let p = Presenter()
        let n = NodeView(id: 1, kind: "view", presenter: p)
        p.views[n.id] = n
        n.frame = CGRect(origin: .zero, size: size)
        n.applyStyle(style)
        n.layer.displayIfNeeded()
        return n
    }

    /// CSS `filter` (LLP 1055.000 D14) arriving in a later style, and
    /// leaving in one after: the box is masked while filtered and its
    /// picture is drawn with each batch; a node that never had one is
    /// neither.
    func testAFilterGainedAfterCreationIsAppliedAndOneLostIsRemoved() throws {
        let sub: [Double] = [-10, -10, 20, 20]
        // One Gaussian blur over the box's own picture (`Filter::encode`).
        let program: [Double] = sub + [1] + [0, -1, -3] + sub + [0, 2, 2]
        guard SvgFilterGPU.runs(program.map(Float.init)) else { throw missing("metal", "no GPU filter path on this host") }
        let filter: BatchValue = .object(["p": .array(program.map { .number($0) })])
        let plain: NodeStyle = ["background_color": white]
        let p = Presenter()
        let n = NodeView(id: 1, kind: "view", presenter: p)
        p.views[n.id] = n
        n.frame = CGRect(x: 0, y: 0, width: 300, height: 120)
        p.viewport.addSubview(n)
        n.applyStyle(plain)
        XCTAssertTrue(p.boxFilters.isEmpty)
        XCTAssertNil(n.layer.mask)
        var filtered = plain; filtered["filter"] = filter
        n.applyStyle(filtered)
        XCTAssertFalse(p.boxFilters.isEmpty, "the style that brings a filter makes the box's picture")
        XCTAssertNotNil(n.layer.mask, "the box itself is hidden behind its picture")
        n.applyStyle(plain)
        XCTAssertTrue(p.boxFilters.isEmpty)
        XCTAssertNil(n.layer.mask)
    }

    func testAnUnpaintedNodeHasNoBitmapHoweverTall() {
        let spacer = node([:], size: CGSize(width: 402, height: 865_678))
        XCTAssertNil(spacer.layer.contents)
        XCTAssertNil(spacer.layer.backgroundColor)
    }

    func testBackgroundAndMaskedRadiusAreLayerProperties() {
        let card = node(["background_color": white, "border_radius_top_left": 24, "border_radius_top_right": 24])
        XCTAssertNil(card.layer.contents)
        XCTAssertEqual(card.layer.backgroundColor?.alpha, 1)
        XCTAssertEqual(card.layer.cornerRadius, 24)
        XCTAssertEqual(card.layer.maskedCorners, [.layerMinXMinYCorner, .layerMaxXMinYCorner])
        XCTAssertFalse(card.layer.masksToBounds, "rounding clips nothing while the overflow is visible")
    }

    func testAUniformBorderStaysUnderTheChildrenUnlessTheyAreClipped() {
        let style: NodeStyle = ["border_width": 2, "border_color_top": blue, "border_radius_top_left": 11,
            "border_radius_top_right": 11, "border_radius_bottom_right": 11, "border_radius_bottom_left": 11]
        let open = node(style, size: CGSize(width: 22, height: 22))
        XCTAssertNil(open.layer.contents)
        XCTAssertEqual(open.layer.borderWidth, 0)
        let under = open.layer.sublayers?.first
        XCTAssertEqual(under?.borderWidth, 2)
        XCTAssertEqual(under?.cornerRadius, 11)
        var clipped = style; clipped["overflow_x"] = "hidden"; clipped["overflow_y"] = "hidden"
        let closed = node(clipped, size: CGSize(width: 22, height: 22))
        XCTAssertNil(closed.layer.contents)
        XCTAssertEqual(closed.layer.borderWidth, 2, "a clipping box's border covers what it clips")
        XCTAssertTrue(closed.layer.masksToBounds)
        XCTAssertNil(closed.boxBorder)
    }

    /// A row's separator (`border-bottom`) is a rectangle on a shape layer
    /// under the children: no backing store of the row's size for a hairline.
    func testSidesInOneColourAreAShapeLayer() throws {
        let row = node(["background_color": white, "border_width_bottom": 0.5, "border_width_left": 2, "border_color_top": blue])
        XCTAssertNil(row.layer.contents, "sides in one colour keep no bitmap")
        XCTAssertEqual(row.layer.backgroundColor?.alpha, 1)
        let edges = try XCTUnwrap(row.boxBorder as? CAShapeLayer)
        XCTAssertTrue(row.layer.sublayers?.first === edges, "under the children")
        XCTAssertEqual(edges.fillColor?.components, row.color("border_color_top", .clear).cgColor.components)
        let path = try XCTUnwrap(edges.path)
        XCTAssertTrue(path.contains(CGPoint(x: 150, y: 119.75)), "the bottom side")
        XCTAssertTrue(path.contains(CGPoint(x: 1, y: 60)), "the left side")
        XCTAssertFalse(path.contains(CGPoint(x: 150, y: 60)), "nothing inside")
        XCTAssertFalse(path.contains(CGPoint(x: 299, y: 60)), "no right side")
    }

    func testWhatTheLayerCannotSayStillDraws() {
        let sides = node(["background_color": white, "border_width_bottom": 1, "border_width_top": 1,
            "border_color_top": blue, "border_color_bottom": white])
        XCTAssertNotNil(sides.layer.contents, "sides in two colours draw")
        XCTAssertNil(sides.layer.backgroundColor, "and so does its background, once")
        let corners = node(["background_color": white, "border_radius_top_left": 8, "border_radius_bottom_right": 20])
        XCTAssertNil(corners.layer.contents, "unequal radii use a shape mask")
        XCTAssertEqual(corners.layer.cornerRadius, 0)
    }

    /// An image's decoded pixels are a sublayer's contents: the visible part
    /// of the fitted image (`contentsRect`), carrying the radius, with no
    /// bitmap painted for the view. An image whose clip the sublayer cannot
    /// say (padding under a radius) draws as before.
    func testAnImagesPixelsAreASublayersContents() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-image-layer-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        for (name, width, height) in [("wide.png", 200, 100), ("skewed.png", 4000, 1001)] {
            let context = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.setFillColor(red: 1, green: 0, blue: 0, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
            let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(root.appendingPathComponent(name) as CFURL, "public.png" as CFString, 1, nil))
            CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), nil)
            XCTAssertTrue(CGImageDestinationFinalize(destination))
        }

        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: root)
        defer { loader.shutdown(); withExtendedLifetime(resolver) {} }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        func image(_ id: UInt32, _ style: NodeStyle, _ source: String = "wide.png") -> NodeView {
            let n = NodeView(id: id, kind: "image", presenter: p)
            p.views[id] = n; p.viewport.addSubview(n)
            n.frame = CGRect(x: 0, y: 0, width: 40, height: 40)
            n.applyStyle(style)
            n.loadGeneration = 1
            loader.load(n, source: source, resolver: resolver)
            return n
        }
        let avatar = image(1, ["object_fit": .string("cover"), "border_radius_top_left": 20, "border_radius_top_right": 20,
            "border_radius_bottom_right": 20, "border_radius_bottom_left": 20])
        let padded = image(2, ["object_fit": .string("cover"), "padding_left": 4, "border_radius_top_left": 20,
            "border_radius_top_right": 20, "border_radius_bottom_right": 20, "border_radius_bottom_left": 20])
        padded.applyContentBox(padding: [0, 0, 0, 4])
        let skewed = image(3, ["object_fit": .string("cover")], "skewed.png")
        let letterboxed = image(4, ["object_fit": .string("contain"), "border_radius_top_left": 8, "border_radius_top_right": 8,
            "border_radius_bottom_right": 8, "border_radius_bottom_left": 8])
        let end = Date(timeIntervalSinceNow: 5)
        while (avatar.raster == nil || padded.raster == nil || skewed.raster == nil || letterboxed.raster == nil) && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
        avatar.layer.displayIfNeeded(); padded.layer.displayIfNeeded(); skewed.layer.displayIfNeeded(); letterboxed.layer.displayIfNeeded()

        let sub = try XCTUnwrap(avatar.imageLayer)
        let whole = try XCTUnwrap(sub.sublayers?.first)
        XCTAssertTrue((whole.contents as AnyObject?) === avatar.raster?.image.image)
        XCTAssertNil(avatar.layer.contents, "no bitmap of the view's size")
        XCTAssertEqual(sub.frame, avatar.bounds)
        XCTAssertEqual(whole.frame, CGRect(x: -20, y: 0, width: 80, height: 40))
        XCTAssertEqual(whole.contentsGravity, .resize)
        XCTAssertEqual(sub.cornerRadius, 20); XCTAssertTrue(sub.masksToBounds)

        XCTAssertNil(padded.imageLayer)
        XCTAssertNotNil(padded.layer.contents, "a radius over a padded content box draws")

        // A rounded `contain` image that leaves part of its box empty is
        // clipped by the box's corners, not drawn.
        let boxed = try XCTUnwrap(letterboxed.imageLayer)
        XCTAssertNil(letterboxed.layer.contents)
        XCTAssertEqual(boxed.frame, letterboxed.bounds)
        XCTAssertEqual(boxed.sublayers?.first?.frame, CGRect(x: 0, y: 10, width: 40, height: 20))
        XCTAssertEqual(boxed.cornerRadius, 8); XCTAssertTrue(boxed.masksToBounds)

        // The decoded aspect differs; the fitted rectangle retains the natural one.
        let reduced = try XCTUnwrap(skewed.raster?.image.image)
        XCTAssertLessThan(reduced.width, 4000, "decoded at the box's size")
        XCTAssertNotEqual(Double(reduced.width) / Double(reduced.height), 4000.0 / 1001, accuracy: 0.01)
        let skewedSub = try XCTUnwrap(skewed.imageLayer)
        let skewedWhole = try XCTUnwrap(skewedSub.sublayers?.first)
        XCTAssertEqual(skewedWhole.contentsGravity, .resize)
        let fitted = 40 * 4000.0 / 1001
        XCTAssertEqual(skewedWhole.bounds.width, fitted, accuracy: 0.0001)
        XCTAssertEqual(skewedWhole.frame.midX, 20, accuracy: 0.0001)

        window.layoutIfNeeded(); CATransaction.flush()
        // UIKit's animated bounds and the pixels must cover the same box throughout a resize.
        UIView.animate(withDuration: 0.8) {
            avatar.frame.size = CGSize(width: 160, height: 80)
            avatar.applyImageLayer()
        }
        let viewBounds = try XCTUnwrap((avatar.layer.animation(forKey: "bounds") ?? avatar.layer.animation(forKey: "bounds.size")) as? CABasicAnimation)
        let pixelsBounds = try XCTUnwrap((sub.animation(forKey: "bounds") ?? sub.animation(forKey: "bounds.size")) as? CABasicAnimation)
        XCTAssertEqual(pixelsBounds.duration, viewBounds.duration, accuracy: 0.001)
        XCTAssertEqual(pixelsBounds.keyPath, viewBounds.keyPath)
        XCTAssertEqual(pixelsBounds.fromValue as? NSValue, viewBounds.fromValue as? NSValue)
        XCTAssertEqual(sub.bounds.size, avatar.bounds.size)
        XCTAssertEqual(sub.animation(forKey: "position")?.duration ?? 0, viewBounds.duration, accuracy: 0.001, "its centre moves with the box")
        XCTAssertEqual(whole.bounds.width / whole.bounds.height, 2, accuracy: 0.0001)
        XCTAssertFalse(sub.animationKeys()?.contains { $0.hasPrefix("contentsRect") } ?? false)
        UIView.animate(withDuration: 0.8) {
            skewed.frame.size = CGSize(width: 160, height: 80)
            skewed.applyImageLayer()
        }
        func halfwayBounds(_ layer: CALayer) throws -> CGSize {
            let animation = try XCTUnwrap((layer.animation(forKey: "bounds") ?? layer.animation(forKey: "bounds.size")) as? CABasicAnimation)
            let from = try XCTUnwrap(animation.fromValue as? NSValue)
            let size = animation.keyPath == "bounds.size" ? from.cgSizeValue : from.cgRectValue.size
            return CGSize(width: (size.width + layer.bounds.width) / 2, height: (size.height + layer.bounds.height) / 2)
        }
        let fittedHalfway: CGSize
        if let whole = skewedSub.sublayers?.first, whole.contents != nil {
            fittedHalfway = try halfwayBounds(whole)
        } else {
            let visible = try halfwayBounds(skewedSub), crop = skewedSub.contentsRect
            fittedHalfway = CGSize(width: visible.width / crop.width, height: visible.height / crop.height)
        }
        XCTAssertEqual(fittedHalfway.width / fittedHalfway.height, 4000.0 / 1001, accuracy: 0.0001,
                       "a decoded cover keeps its natural aspect halfway through UIKit's resize")
        avatar.layer.removeAllAnimations(); sub.removeAllAnimations()
        UIView.performWithoutAnimation {
            avatar.frame.size = CGSize(width: 80, height: 80)
            avatar.applyImageLayer()
        }
        XCTAssertNil(sub.animationKeys(), "an ordinary layout must not introduce an implicit animation")

        avatar.raster = nil
        XCTAssertNil(avatar.imageLayer, "no pixels outlive the lease")
        XCTAssertNil(sub.superlayer)
        padded.raster = nil; skewed.raster = nil; letterboxed.raster = nil
    }

    /// A `-exact-tint-color` draws the bitmap as a template from the same decoded
    /// pixels, through `draw(_:)` (a canvas capture drops a mask layer): the
    /// opaque half takes the tint, the transparent half stays clear, and a
    /// `light-dark()` tint follows a live appearance change. Without the row
    /// the pixels are the sublayer's contents again.
    func testATintedImageIsItsAlphaInTheTint() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-image-tint-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        let context = try XCTUnwrap(CGContext(data: nil, width: 200, height: 100, bitsPerComponent: 8, bytesPerRow: 800,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(red: 0, green: 0, blue: 0, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: 100, height: 100))
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(root.appendingPathComponent("mark.png") as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))

        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: root)
        defer { loader.shutdown(); withExtendedLifetime(resolver) {} }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        let tint: BatchValue = [[0, 0, 0, 255], [255, 255, 255, 255]]
        let mark = NodeView(id: 1, kind: "image", presenter: p)
        p.views[1] = mark; p.viewport.addSubview(mark)
        mark.frame = CGRect(x: 0, y: 0, width: 80, height: 40)
        mark.applyStyle(["object_fit": .string("contain"), "tint_color": tint])
        mark.loadGeneration = 1
        loader.load(mark, source: "mark.png", resolver: resolver)
        let end = Date(timeIntervalSinceNow: 5)
        while mark.raster == nil && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
        window.overrideUserInterfaceStyle = .light
        window.layoutIfNeeded(); mark.layer.displayIfNeeded()

        // The view's paint inside the opaque left half and the transparent right half.
        func samples() throws -> [UInt8] {
            let ctx = try XCTUnwrap(CGContext(data: nil, width: 80, height: 40, bitsPerComponent: 8, bytesPerRow: 320,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            mark.layer.render(in: ctx)
            let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
            return [20, 60].flatMap { x in (0..<4).map { bytes[20 * 320 + x * 4 + $0] } }
        }
        XCTAssertNil(mark.imageLayer, "the template is drawn")
        XCTAssertTrue(mark.drawsPaint)
        XCTAssertEqual(try samples(), [0, 0, 0, 255, 0, 0, 0, 0])

        window.overrideUserInterfaceStyle = .dark
        window.layoutIfNeeded(); mark.layer.displayIfNeeded()
        XCTAssertEqual(try samples(), [255, 255, 255, 255, 0, 0, 0, 0], "light-dark() follows the appearance")

        mark.applyStyle(["object_fit": .string("contain")])
        mark.layer.displayIfNeeded()
        let sub = try XCTUnwrap(mark.imageLayer)
        XCTAssertTrue((sub.sublayers?.first?.contents as AnyObject?) === mark.raster?.image.image)
        mark.raster = nil
    }
}
#endif
