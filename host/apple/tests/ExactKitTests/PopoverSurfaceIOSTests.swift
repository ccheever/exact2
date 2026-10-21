#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A popover's default surface on UIKit (LLP 1021; theme2 2026-10-21
/// 19:55): a popover the compiler marked (`defaultSurface`, its author having
/// written no surface) draws UIKit's popover glass under its children, its
/// corner the host's `defaultSurfaceRadius` (iOS's metric, 30); one its
/// author surfaced, and a menu, draw none. Hosted: glass draws in a scene's
/// window.
/// Keeps a popover a popover on an iPhone.
final class Adapt: NSObject, UIPopoverPresentationControllerDelegate {
    static let none = Adapt()
    func adaptivePresentationStyle(for controller: UIPresentationController, traitCollection: UITraitCollection) -> UIModalPresentationStyle { .none }
}

final class PopoverSurfaceIOSTests: XCTestCase {
    var window: UIWindow?

    /// A red page and a popover with `props` on it at `box`, shown; the
    /// popover's node and the window drawn at 3x.
    func paint(_ props: [String: String], dark: Bool, at box: CGRect = CGRect(x: 40, y: 200, width: 304, height: 80)) throws -> (NodeView, UIImage) {
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let p = Presenter(), window = UIWindow(windowScene: scene)
        self.window = window
        window.frame = scene.coordinateSpace.bounds
        window.overrideUserInterfaceStyle = dark ? .dark : .light
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": [String: String](), "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 2, "kind": "view", "props": props.merging(["popover": "auto", "id": "p"]) { a, _ in a }, "style": [String: Any]()],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
            ["op": "frame", "id": 2, "x": Double(box.minX), "y": Double(box.minY), "w": Double(box.width), "h": Double(box.height)],
        ]))
        let node = try XCTUnwrap(p.views[2])
        node.isHidden = false // as open: the painted popover's layer shows it
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        let format = UIGraphicsImageRendererFormat()
        format.scale = 3
        let image = UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        return (node, image)
    }

    private var decoded: [ObjectIdentifier: (width: Int, bytes: [UInt8])] = [:]

    /// The pixel at `point` (points), as RGB.
    func pixel(_ image: UIImage, _ point: CGPoint) -> [Int] {
        let key = ObjectIdentifier(image)
        if decoded[key] == nil {
            let cg = image.cgImage!, w = cg.width, h = cg.height
            var buf = [UInt8](repeating: 0, count: w * h * 4)
            let ctx = CGContext(data: &buf, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
            decoded[key] = (w, buf)
        }
        let (w, buf) = decoded[key]!
        let i = (Int(point.y * 3) * w + Int(point.x * 3)) * 4
        return [Int(buf[i]), Int(buf[i + 1]), Int(buf[i + 2])]
    }

    /// UIKit's popover of an empty 304 by 80 content over the same page: the
    /// content's frame in the window and the window drawn at 3x.
    func uikitPopover(dark: Bool) throws -> (CGRect, UIImage) {
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        self.window = window
        window.frame = scene.coordinateSpace.bounds
        window.overrideUserInterfaceStyle = dark ? .dark : .light
        let root = UIViewController()
        root.view.backgroundColor = .red
        window.rootViewController = root
        window.makeKeyAndVisible()
        let anchor = UIView(frame: CGRect(x: 100, y: 200, width: 44, height: 44))
        root.view.addSubview(anchor)
        let content = UIViewController()
        content.preferredContentSize = CGSize(width: 304, height: 80)
        content.modalPresentationStyle = .popover
        content.popoverPresentationController?.sourceView = anchor
        content.popoverPresentationController?.sourceRect = anchor.bounds
        content.popoverPresentationController?.delegate = Adapt.none
        let done = expectation(description: "presented")
        root.present(content, animated: false) { done.fulfill() }
        wait(for: [done], timeout: 5)
        RunLoop.main.run(until: Date().addingTimeInterval(0.8))
        let frame = content.view.convert(content.view.bounds, to: window)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 3
        let image = UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        root.dismiss(animated: false)
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        window.isHidden = true
        return (frame, image)
    }

    /// Each 3x row's first pixel from the box's left that is plainly not the
    /// page (a channel 24 off: a shadow's faint edge is not the shape), over
    /// the box's top 34 points (its corner), from its left edge.
    func profile(_ image: UIImage, _ box: CGRect) -> [Int] {
        (0..<102).map { row in
            let y = box.minY + CGFloat(row) / 3
            return (0..<120).first { col in
                zip(pixel(image, CGPoint(x: box.minX - 1 + CGFloat(col) / 3, y: y)), [255, 0, 0]).map { abs($0 - $1) }.max() ?? 0 > 24
            } ?? -1
        }
    }

    func testAPopoverWithNoSurfaceOfItsAuthorsDrawsUIKitsPopoverGlass() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "glass draws in a scene's window") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("UIKit's popover is glass from iOS 26") }
        for dark in [false, true] {
            let scheme = dark ? "dark" : "light"
            let (box, theirs) = try uikitPopover(dark: dark)
            let (node, ours) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30", "defaultSurfaceShadow": "8 24 0.015"], dark: dark, at: box)
            let glass = try XCTUnwrap(node.materialView, scheme)
            XCTAssertTrue(glass.effect is UIGlassEffect, "UIKit's popover glass")
            // Its shape is UIKit's popover's, row by row round the corner,
            // and its fill the same over the same page.
            // Within 4 pixels at 3x (a point and a third): the rims' light
            // differs that much in dark. A glass's rim catches a stray pixel
            // now and then: three rows of the 102 may stray.
            let a = profile(theirs, box), b = profile(ours, box)
            XCTAssertLessThanOrEqual(zip(a, b).filter { abs($0 - $1) > 4 }.count, 3, "\(scheme): UIKit \(a), ours \(b)")
            let middle = CGPoint(x: box.midX, y: box.midY)
            XCTAssertLessThanOrEqual(zip(pixel(theirs, middle), pixel(ours, middle)).map { abs($0 - $1) }.max() ?? 99, 12,
                                     "\(scheme): UIKit \(pixel(theirs, middle)), ours \(pixel(ours, middle))")
        }
    }

    func testAPopoverItsAuthorSurfacedAndAMenuDrawNone() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "glass draws in a scene's window") }
        let (surfaced, _) = try paint([:], dark: false)
        XCTAssertNil(surfaced.materialView, "no mark: its author wrote a surface")
        let (menu, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30", "accessibilityRole": "menu"], dark: false)
        XCTAssertNil(menu.materialView, "a menu is the platform's")
        let (mac, _) = try paint(["defaultSurface": "true"], dark: false)
        XCTAssertNil(mac.materialView, "no corner sent: a platform that draws its own")
    }
}
#endif
