#if os(iOS)
import ImageIO
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1123 D11: an image's background extension is a view beside the
/// image, outside its subtree, in its route: it grows on the sides where the
/// image meets the route's safe area, composites as the image does and goes
/// when the image does.
final class BackgroundExtensionIOSTests: XCTestCase {
    private var window: UIWindow?
    private var loader: RasterLoader?

    override func tearDown() {
        loader?.shutdown()
        window?.isHidden = true
        window = nil
        loader = nil
        super.tearDown()
    }

    /// A page (1) holding a box (3) that holds an image (2) at `frame`,
    /// `source` or else red, styled `style`, which says
    /// `-exact-background-extension="auto"`, its raster loaded, in a 300 by
    /// 400 route whose safe area is `insets` in from its edges, in `scene`
    /// where one is given.
    private func mount(_ frame: CGRect, insets: UIEdgeInsets, style: [String: Any] = [:], source: Data? = nil, in scene: UIWindowScene? = nil) throws -> (Presenter, RouteController, NodeView) {
        guard #available(iOS 26.0, *) else { throw XCTSkip("UIBackgroundExtensionView is iOS 26's") }
        let format = UIGraphicsImageRendererFormat(); format.scale = 1
        let png = try source ?? XCTUnwrap(UIGraphicsImageRenderer(size: CGSize(width: 30, height: 10), format: format).image { context in
            UIColor.red.setFill(); context.fill(CGRect(x: 0, y: 0, width: 30, height: 10))
        }.pngData())
        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: URL(fileURLWithPath: NSTemporaryDirectory()))
        self.loader = loader
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 2, "kind": "image", "props": ["backgroundExtension": "auto"], "style": style],
            ["op": "children", "id": 1, "ids": [3]],
            ["op": "children", "id": 3, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 400],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 300, "h": 400],
            ["op": "frame", "id": 2, "x": frame.minX, "y": frame.minY, "w": frame.width, "h": frame.height],
        ]))
        let page = try XCTUnwrap(p.views[1]), image = try XCTUnwrap(p.views[2])
        let route = RouteController(page), window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(x: 0, y: 0, width: 300, height: 400)
        self.window = window
        window.rootViewController = route
        window.makeKeyAndVisible()
        let own = route.view.safeAreaInsets
        route.additionalSafeAreaInsets = UIEdgeInsets(top: insets.top - own.top, left: insets.left - own.left, bottom: insets.bottom - own.bottom, right: insets.right - own.right)
        route.view.addSubview(page)
        route.view.layoutIfNeeded()
        XCTAssertEqual(route.view.safeAreaInsets, insets)
        image.loadGeneration = 1
        loader.load(image, source: "data:\(source == nil ? "image/png" : "image/gif");base64," + png.base64EncodedString(), resolver: resolver)
        let deadline = Date(timeIntervalSinceNow: 5)
        while image.raster == nil, Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertNotNil(image.raster)
        image.syncBackgroundExtension()
        return (p, route, image)
    }

    private func extensions(_ route: RouteController) -> [UIView] {
        guard #available(iOS 26.0, *) else { return [] }
        return route.view.subviews.filter { $0 is UIBackgroundExtensionView }
    }

    /// The window as the window server draws it: each point's red, green and blue.
    private func pixels() throws -> (CGPoint) -> [Int] {
        let window = try XCTUnwrap(self.window)
        let format = UIGraphicsImageRendererFormat(); format.scale = 1
        let cg = try XCTUnwrap(UIGraphicsImageRenderer(size: window.bounds.size, format: format).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }.cgImage)
        var data = [UInt8](repeating: 0, count: cg.width * cg.height * 4)
        let context = try XCTUnwrap(CGContext(data: &data, width: cg.width, height: cg.height, bitsPerComponent: 8, bytesPerRow: cg.width * 4,
                                              space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(cg, in: CGRect(x: 0, y: 0, width: cg.width, height: cg.height))
        return { point in let i = (Int(point.y) * cg.width + Int(point.x)) * 4; return [Int(data[i]), Int(data[i + 1]), Int(data[i + 2])] }
    }

    func testAnImagesExtensionGoesWithTheImage() throws {
        let (p, route, _) = try mount(CGRect(x: 0, y: 50, width: 300, height: 100), insets: UIEdgeInsets(top: 50, left: 0, bottom: 0, right: 0))
        XCTAssertEqual(extensions(route).count, 1, "the image meets the safe area's top")
        p.apply(wireBatch([["op": "destroy", "id": 2], ["op": "children", "id": 3, "ids": []]]))
        XCTAssertTrue(extensions(route).isEmpty, "the image is destroyed, and its extension with it")
    }

    /// The extension grows to the route's edge on each side where the image
    /// meets the route's safe area, and on no other: an image short of the
    /// safe area's edge, or on an edge with no safe area, has none there.
    func testTheExtensionGrowsOnTheSidesTheImageMeetsTheSafeArea() throws {
        let insets = UIEdgeInsets(top: 50, left: 40, bottom: 20, right: 30)
        let (p, route, image) = try mount(CGRect(x: 40, y: 50, width: 230, height: 100), insets: insets)
        let cases: [(CGRect, CGRect?, String)] = [
            (CGRect(x: 40, y: 50, width: 230, height: 100), CGRect(x: 0, y: 0, width: 300, height: 150), "top, left and right"),
            (CGRect(x: 40, y: 80, width: 100, height: 100), CGRect(x: 0, y: 80, width: 140, height: 100), "left"),
            (CGRect(x: 100, y: 280, width: 100, height: 100), CGRect(x: 100, y: 280, width: 100, height: 120), "bottom"),
            (CGRect(x: 41, y: 51, width: 100, height: 100), nil, "none: a point short of the top and left"),
        ]
        for (frame, grown, sides) in cases {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": frame.minX, "y": frame.minY, "w": frame.width, "h": frame.height]]))
            image.syncBackgroundExtension()
            let shown = extensions(route)
            XCTAssertEqual(shown.first?.frame, grown, sides)
            XCTAssertLessThanOrEqual(shown.count, 1, sides)
            // The image keeps its place: the extension's content is laid out at it.
            if #available(iOS 26.0, *), let content = (shown.first as? UIBackgroundExtensionView)?.contentView {
                XCTAssertEqual(content.convert(content.bounds, to: route.view), frame, sides)
            }
        }
        // A side with no safe area has nothing to fill, even where the image reaches the route's edge.
        let added = route.additionalSafeAreaInsets
        route.additionalSafeAreaInsets = UIEdgeInsets(top: added.top - insets.top, left: added.left - insets.left, bottom: added.bottom - insets.bottom, right: added.right - insets.right)
        route.view.layoutIfNeeded()
        XCTAssertEqual(route.view.safeAreaInsets, .zero)
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 100]]))
        image.syncBackgroundExtension()
        XCTAssertTrue(extensions(route).isEmpty)
    }

    /// Within the image's box only the image draws: nothing of the
    /// extension's square picture shows behind the image's rounded corners,
    /// and a faded image is not drawn over a copy of itself. The extension's
    /// content takes the image's corners, so what it mirrors is the image as
    /// it shows.
    func testWithinTheImagesBoxOnlyTheImageDraws() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the extension's pixels are drawn by the window server")
        }
        let corners: [String: Any] = ["border_radius_top_left": 30, "border_radius_top_right": 30, "border_radius_bottom_right": 30, "border_radius_bottom_left": 30]
        let (p, route, image) = try mount(CGRect(x: 0, y: 50, width: 300, height: 100), insets: UIEdgeInsets(top: 50, left: 0, bottom: 0, right: 0), style: corners, in: scene)
        var at = try pixels()
        let ground = at(CGPoint(x: 150, y: 300)), red = [255, 0, 0]
        XCTAssertNotEqual(ground, red)
        XCTAssertEqual(at(CGPoint(x: 150, y: 25)), red, "the extension fills the safe area above the image")
        XCTAssertEqual(at(CGPoint(x: 150, y: 100)), red, "the image")
        XCTAssertEqual(at(CGPoint(x: 3, y: 53)), ground, "the ground behind the image's corner, not a square picture")
        if #available(iOS 26.0, *), let content = (extensions(route).first as? UIBackgroundExtensionView)?.contentView {
            let mask = (content.layer.mask as? CAShapeLayer)?.path
            XCTAssertEqual(mask, image.roundedPath(in: content.bounds).cgPath, "the extension mirrors the image's corners")
        }
        p.apply(wireBatch([["op": "present", "id": 2, "property": "opacity", "x": 0.5, "y": 0.0, "w": 0.0, "h": 0.0]]))
        at = try pixels()
        let half = zip(red, ground).map { ($0 + $1) / 2 }, shown = at(CGPoint(x: 150, y: 100))
        XCTAssertTrue(zip(shown, half).allSatisfy { abs($0 - $1) <= 2 }, "the image at half its opacity over the ground, once: \(shown), not \(half)")
    }

    /// The extension mirrors the pixels the image shows: with a
    /// `-exact-tint-color`, the tint through the picture's alpha, and only
    /// what the image's content box shows of its picture.
    func testTheExtensionMirrorsTheImagesTintAndClip() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the extension's pixels are drawn by the window server")
        }
        let green: [String: Any] = ["tint_color": [0, 255, 0, 255]]
        let (p, route, image) = try mount(CGRect(x: 0, y: 50, width: 300, height: 100), insets: UIEdgeInsets(top: 50, left: 0, bottom: 0, right: 0), style: green, in: scene)
        var at = try pixels()
        let ground = at(CGPoint(x: 150, y: 300))
        XCTAssertEqual(at(CGPoint(x: 150, y: 100)), [0, 255, 0], "the image, tinted")
        XCTAssertEqual(at(CGPoint(x: 150, y: 25)), [0, 255, 0], "its extension, tinted")
        // Untinted and covering a content box 40 points inside the image's
        // top and bottom, the picture overflows into the padding, where the
        // image shows the ground.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["object_fit": "cover"]]]))
        image.applyKernelContentBox(CGRect(x: 0, y: 40, width: 300, height: 20), padding: [40, 0, 40, 0])
        image.syncBackgroundExtension()
        at = try pixels()
        XCTAssertEqual(at(CGPoint(x: 150, y: 100)), [255, 0, 0], "the content box")
        XCTAssertEqual(at(CGPoint(x: 150, y: 55)), ground, "the padding")
        let mirrored = at(CGPoint(x: 150, y: 45))
        let near = { (a: [Int], b: [Int]) in zip(a, b).map { abs($0 - $1) }.reduce(0, +) }
        XCTAssertLessThan(near(mirrored, ground), near(mirrored, [255, 0, 0]), "the extension mirrors the padding: \(mirrored)")
        XCTAssertEqual(extensions(route).count, 1)
    }

    /// An animated image's extension shows the frame the image shows.
    func testAnAnimatedImagesExtensionFollowsItsFrame() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the extension's pixels are drawn by the window server")
        }
        // Red for 50 ms, then blue for a minute.
        let gif = NSMutableData()
        let destination = try XCTUnwrap(CGImageDestinationCreateWithData(gif as CFMutableData, "com.compuserve.gif" as CFString, 2, nil))
        CGImageDestinationSetProperties(destination, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFLoopCount: 0]] as CFDictionary)
        for (color, delay) in [(UIColor.red, 0.05), (UIColor.blue, 60.0)] {
            let format = UIGraphicsImageRendererFormat(); format.scale = 1
            let frame = try XCTUnwrap(UIGraphicsImageRenderer(size: CGSize(width: 30, height: 10), format: format).image { context in
                color.setFill(); context.fill(CGRect(x: 0, y: 0, width: 30, height: 10))
            }.cgImage)
            CGImageDestinationAddImage(destination, frame, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFDelayTime: delay, kCGImagePropertyGIFUnclampedDelayTime: delay]] as CFDictionary)
        }
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        let (_, _, image) = try mount(CGRect(x: 0, y: 50, width: 300, height: 100), insets: UIEdgeInsets(top: 50, left: 0, bottom: 0, right: 0),
                                      source: gif as Data, in: scene)
        XCTAssertNotNil(image.imageLayer, "the image's pixels are a layer's, which its player sets")
        let deadline = Date(timeIntervalSinceNow: 10)
        while AnimatedRasters.shared.frame(for: image) == nil, Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        XCTAssertNotNil(AnimatedRasters.shared.frame(for: image), "the second frame shows")
        let at = try pixels()
        XCTAssertEqual(at(CGPoint(x: 150, y: 100)), [0, 0, 255], "the image's second frame")
        XCTAssertEqual(at(CGPoint(x: 150, y: 25)), [0, 0, 255], "its extension's")
    }

    /// The extension is outside the image's subtree, so it takes the
    /// opacity the image composites with, its own and each ancestor's below
    /// the view the extension is in, as they change; an ancestor that is
    /// `display: none` hides it.
    func testTheExtensionTakesTheOpacityTheImageCompositesWith() throws {
        let (p, route, _) = try mount(CGRect(x: 0, y: 50, width: 300, height: 100), insets: UIEdgeInsets(top: 50, left: 0, bottom: 0, right: 0))
        let view = try XCTUnwrap(extensions(route).first)
        func present(_ id: Int, _ opacity: Double) {
            p.apply(wireBatch([["op": "present", "id": id, "property": "opacity", "x": opacity, "y": 0.0, "w": 0.0, "h": 0.0]]))
        }
        XCTAssertEqual(view.alpha, 1)
        present(2, 0.5)
        XCTAssertEqual(view.alpha, 0.5, accuracy: 1e-6, "the image's own")
        present(3, 0.4)
        XCTAssertEqual(view.alpha, 0.2, accuracy: 1e-6, "the image's and its box's")
        present(2, 1)
        present(1, 0.5)
        XCTAssertEqual(view.alpha, 0.2, accuracy: 1e-6, "the box's and the page's")
        present(2, 0)
        XCTAssertEqual(view.alpha, 0, "an image faded out")
        present(2, 1)
        XCTAssertFalse(view.isHidden)
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["display": "none"]]]))
        XCTAssertTrue(view.isHidden || view.superview == nil, "the box is not displayed")
        p.apply(wireBatch([["op": "style", "id": 3, "style": [:]]]))
        XCTAssertTrue(extensions(route).first.map { !$0.isHidden } ?? false, "the box is displayed again")
    }
}
#endif
