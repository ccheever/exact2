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

    /// A page of `page` and a popover with `props` and `style` on it at
    /// `box`, shown, holding a green child that fills it when `filled`; the
    /// presenter, the popover's node and the window drawn at 3x.
    func paint(_ props: [String: String], dark: Bool, at box: CGRect = CGRect(x: 40, y: 200, width: 304, height: 80),
               page: [Int] = [255, 0, 0], style: [String: Any] = [:], filled: Bool = false) throws -> (Presenter, NodeView, UIImage) {
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let p = Presenter(), window = UIWindow(windowScene: scene)
        self.window = window
        window.frame = scene.coordinateSpace.bounds
        window.overrideUserInterfaceStyle = dark ? .dark : .light
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "props": [String: String](), "style": ["background_color": page + [255]]],
            ["op": "create", "id": 2, "kind": "view", "props": props.merging(["popover": "auto", "id": "p"]) { a, _ in a }, "style": style],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
            ["op": "frame", "id": 2, "x": Double(box.minX), "y": Double(box.minY), "w": Double(box.width), "h": Double(box.height)],
        ]
        if filled {
            ops += [["op": "create", "id": 3, "kind": "view", "props": [String: String](), "style": ["background_color": [0, 255, 0, 255]]],
                    ["op": "children", "id": 2, "ids": [3]],
                    ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": Double(box.width), "h": Double(box.height)]]
        }
        p.apply(wireBatch(ops))
        let node = try XCTUnwrap(p.views[2])
        node.isHidden = false // as open: the painted popover's layer shows it
        return (p, node, shot(window))
    }

    func shot(_ window: UIWindow) -> UIImage {
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        let format = UIGraphicsImageRendererFormat()
        format.scale = 3
        return UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
    }

    private var decoded: [ObjectIdentifier: (image: UIImage, width: Int, bytes: [UInt8])] = [:] // the image kept, so its id is not reused

    /// The pixel at `point` (points), as RGB.
    func pixel(_ image: UIImage, _ point: CGPoint) -> [Int] {
        let key = ObjectIdentifier(image)
        if decoded[key] == nil {
            let cg = image.cgImage!, w = cg.width, h = cg.height
            var buf = [UInt8](repeating: 0, count: w * h * 4)
            let ctx = CGContext(data: &buf, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
            decoded[key] = (image, w, buf)
        }
        let (_, w, buf) = decoded[key]!
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
            let (_, node, ours) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30", "defaultSurfaceShadow": "8 24 0.015"], dark: dark, at: box)
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

    /// Its children are clipped to its corner, scrolled or not; its faint
    /// shadow is the light appearance's, and goes when the same node turns
    /// dark.
    func testItsChildrenTakeItsCornerAndItsShadowIsTheLightAppearances() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "glass draws in a scene's window") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("UIKit's popover is glass from iOS 26") }
        let marked = ["defaultSurface": "true", "defaultSurfaceRadius": "30", "defaultSurfaceShadow": "8 24 0.015"]
        for scrolls in [false, true] {
            let (_, _, image) = try paint(marked, dark: false, style: scrolls ? ["overflow_y": "scroll"] : [:], filled: true)
            XCTAssertEqual(pixel(image, CGPoint(x: 41, y: 201))[1], 0, "its corner clips its child, no green there (scrolls: \(scrolls))")
            XCTAssertEqual(pixel(image, CGPoint(x: 192, y: 240))[1], 255, "the child inside")
        }
        let (p, node, light) = try paint(marked, dark: false, page: [255, 255, 255])
        let below = CGPoint(x: 192, y: 284)
        XCTAssertLessThan(pixel(light, below)[0], 255, "a faint shadow below it in light")
        XCTAssertFalse(node.boxShadows.isEmpty)
        window?.overrideUserInterfaceStyle = .dark
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["testId": "dark"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertTrue(node.boxShadows.isEmpty, "none in dark")
    }

    func testAPopoverItsAuthorSurfacedAMenuAndAnInlineNodeDrawNone() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "glass draws in a scene's window") }
        let (_, surfaced, _) = try paint([:], dark: false, style: ["background_color": [255, 255, 255, 255]])
        XCTAssertNil(surfaced.materialView, "no mark: its author wrote a surface")
        let (_, menu, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30", "accessibilityRole": "menu"], dark: false)
        XCTAssertNil(menu.materialView, "a menu is the platform's")
        let (_, mac, _) = try paint(["defaultSurface": "true"], dark: false)
        XCTAssertNil(mac.materialView, "no corner sent: a platform that draws its own")
        // A popover that stops being one (`popover` bound) is an inline node again.
        let (p, inline, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30"], dark: false)
        XCTAssertNotNil(inline.materialView)
        p.apply(wireBatch([["op": "props", "id": 2, "set": [String: String](), "clear": ["popover"]]]))
        XCTAssertNil(inline.materialView, "no popover, no surface")
        // A scrolling one's scroll view takes the corner, and gives it up with the surface.
        let (q, scrolling, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30"], dark: false, style: ["overflow_y": "scroll"], filled: true)
        XCTAssertEqual(scrolling.scroll?.layer.cornerRadius, 30)
        q.apply(wireBatch([["op": "props", "id": 2, "set": [String: String](), "clear": ["popover"]]]))
        XCTAssertEqual(scrolling.scroll?.layer.cornerRadius, 0, "no popover, no corner on its scroll view")
        // An authored corner, even 0, is the author's.
        let (_, square, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30"], dark: false, style: ["border_radius": 0.0])
        XCTAssertNotNil(square.materialView)
        XCTAssertTrue(square.writesCorner, "its author's 0")
        // A corner the glass can't take (not one circle): no default surface rather than a square one.
        let (_, uneven, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30"], dark: false, style: ["border_radius_top_left": 20.0])
        XCTAssertNil(uneven.materialView, "an uneven authored corner")
        let (_, oneSquare, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30"], dark: false, style: ["border_radius_top_left": 0.0])
        XCTAssertNotNil(oneSquare.materialView, "one corner written 0, the others unwritten (0): one square, the glass square")
        // A percentage as the wire carries one, a row a corner (`{"pct":10}`),
        // on a scrolling popover: neither the glass nor its circular clip.
        let pct: [String: Any] = ["pct": 10.0]
        let (_, percent, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30"], dark: false, at: CGRect(x: 40, y: 200, width: 200, height: 100),
                                        style: ["border_radius_top_left": pct, "border_radius_top_right": pct, "border_radius_bottom_right": pct,
                                                "border_radius_bottom_left": pct, "overflow_y": "scroll"], filled: true)
        XCTAssertNil(percent.materialView, "a percentage: an ellipse on this box")
        XCTAssertEqual(try XCTUnwrap(percent.scroll, "a scrolling popover").layer.cornerRadius, 0, "no circular clip for an elliptical corner")
        // An authored circle on a scrolling popover: its scroll view takes that corner.
        let (_, roundScroll, _) = try paint(["defaultSurface": "true", "defaultSurfaceRadius": "30"], dark: false, style: ["border_radius": 12.0, "overflow_y": "scroll"], filled: true)
        XCTAssertEqual(roundScroll.scroll?.layer.cornerRadius, 12, "the author's corner on its scroll view")
    }
}
#endif
