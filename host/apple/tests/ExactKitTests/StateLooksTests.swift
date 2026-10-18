#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// State parts on UIKit and AppKit (LLP 1121 D8, stage 8 B2): `:active`
/// arrives as the press paint (LLP 1061 D7), its opacity an absolute target
/// (`press_opacity_to`) the press eases to from what shows, with no platform
/// dim; rows it doesn't name show the base, a base the engine moves while
/// pressed included. On the Mac a `:hover` and a `:focus-visible` look stand
/// in at once while they hold (the focus's over the hover's), their opacity
/// an additive layer opacity, under the press's paint; a disabled node takes
/// none.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class StateLooksTests: XCTestCase {
    #if os(iOS)
    private var window: UIWindow!
    #else
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }
    #endif

    private let blue: [Double] = [37, 99, 235, 255], navy: [Double] = [29, 78, 216, 255]
    private let white: [Double] = [255, 255, 255, 255], red: [Double] = [255, 0, 0, 255], green: [Double] = [0, 255, 0, 255]
    private func ink(_ c: [Double]) -> BatchValue { .array(c.map(BatchValue.number)) }
    private var looks: [String: Any] {
        ["hover": ["background_color": green, "opacity": 0.8], "focus_visible": ["border_color_top": green]]
    }
    private func base(_ fill: [Double], right: [Double] = [0, 0, 0, 255]) -> [String: Any] {
        ["background_color": fill, "border_width_top": 2, "border_width_right": 2, "border_color_top": [0, 0, 0, 255],
         "border_color_right": right, "looks": looks, "press_background_color": navy, "press_opacity_to": 0.6]
    }

    private func fixture(props: [String: String] = [:]) throws -> (Presenter, NodeView) {
        let p = Presenter()
        #if os(iOS)
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        #else
        _ = NSApplication.shared
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        #endif
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "props": props, "style": base(blue)],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 50.0, "y": 50.0, "w": 200.0, "h": 100.0],
        ]))
        return (p, try XCTUnwrap(p.views[1]))
    }
    private func layer(_ v: NodeView) -> CALayer? { v.layer }
    private func fade(_ v: NodeView) -> CABasicAnimation? { layer(v)?.animation(forKey: "pressOpacity") as? CABasicAnimation }
    private func lookFade(_ v: NodeView) -> Float? { (layer(v)?.animation(forKey: "lookOpacity") as? CABasicAnimation)?.toValue as? Float }

    func testActiveIsThePressPaintAndTheBaseShowsBeneath() throws {
        let (p, v) = try fixture()
        XCTAssertTrue(v.authorsPress, "the author's press")
        #if os(iOS)
        XCTAssertFalse(v.platformPressDims, "no dim on top")
        #else
        XCTAssertFalse(v.platformPressTints, "no wash on top")
        #endif
        v.pressed = true
        XCTAssertEqual(v.style["background_color"], ink(navy))
        XCTAssertEqual(v.style["border_color_right"], ink([0, 0, 0, 255]), "a row it doesn't name is the base's")
        XCTAssertEqual(try XCTUnwrap(fade(v)?.toValue as? Float), -0.4, accuracy: 1e-6, "0.6 in place of 1, not a share")
        // The engine moves the base while pressed: the look stays over it.
        p.apply(wireBatch([["op": "style", "id": 1, "style": base(white, right: red)]]))
        XCTAssertEqual(v.style["background_color"], ink(navy))
        XCTAssertEqual(v.style["border_color_right"], ink(red), "the new base beneath")
        v.press.start -= 1; v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual(v.style["background_color"], ink(white), "the engine's newer value")
    }

    #if os(macOS)
    func testHoverAndFocusVisibleLooksStandInUnderThePress() throws {
        let (p, v) = try fixture()
        p.setHoverPath([v])
        XCTAssertEqual(v.style["background_color"], ink(green), ":hover, though it hears no hover")
        XCTAssertEqual(try XCTUnwrap(lookFade(v)), -0.2, accuracy: 1e-6, "its opacity, 0.8, at once")
        v.focusVisible = true
        XCTAssertEqual(v.style["border_color_top"], ink(green), ":focus-visible")
        v.pressed = true
        XCTAssertEqual(v.style["background_color"], ink(navy), "the press paint over :hover")
        XCTAssertEqual(try XCTUnwrap(fade(v)?.toValue as? Float), -0.2, accuracy: 1e-6, "from the hover's 0.8 to 0.6")
        v.press.start -= 1; v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual(v.style["background_color"], ink(green), "the hover again")
        p.setHoverPath([])
        v.focusVisible = false
        XCTAssertEqual(v.style["background_color"], ink(blue))
        XCTAssertEqual(v.style["border_color_top"], ink([0, 0, 0, 255]))
        XCTAssertNil(lookFade(v))
    }

    func testADisabledNodeTakesNoLook() throws {
        let (p, v) = try fixture(props: ["disabled": "true"])
        p.setHoverPath([v])
        XCTAssertEqual(v.style["background_color"], ink(blue))
    }
    #endif
}
#endif
