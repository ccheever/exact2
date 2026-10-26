#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// LLP 1021 D3 in the page (theme2 2026-10-22 06:53): a custom (non-native)
/// button opening a popover that is no menu opens it as itself, anchored on
/// the button, as a native button's does, and a second press closes it; one
/// opening a menu keeps UIKit's pull-down over the button.
final class InPagePopoverIOSTests: XCTestCase {
    private var window: UIWindow?
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    func testACustomButtonsTextPopoverOpensAsItselfAndAMenuStaysThePullDown() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        defer { p.reset() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "about", "accessibilityLabel": "About", "appearance": "none"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["popover": "auto", "id": "about"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Words, not rows"]],
            ["op": "create", "id": 5, "kind": "button", "props": ["popovertarget": "menu", "accessibilityLabel": "More", "appearance": "none"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["popover": "auto", "id": "menu", "accessibilityRole": "menu"]],
            ["op": "create", "id": 7, "kind": "button", "props": [String: String](), "handlers": ["press"]],
            ["op": "create", "id": 8, "kind": "text", "props": ["text": "Bump"]],
            ["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 7, "ids": [8]], ["op": "children", "id": 6, "ids": [7]],
            ["op": "children", "id": 1, "ids": [2, 5, 3, 6]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 2, "x": 20, "y": 100, "w": 80, "h": 44],
            ["op": "frame", "id": 5, "x": 200, "y": 100, "w": 80, "h": 44],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 200, "h": 60],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 200, "h": 20],
            ["op": "frame", "id": 6, "x": 0, "y": 0, "w": 200, "h": 44],
            ["op": "frame", "id": 7, "x": 0, "y": 0, "w": 200, "h": 44],
        ]))
        let about = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[3]), more = try XCTUnwrap(p.views[5])
        XCTAssertFalse(about.subviews.contains { $0 is UIButton && ($0 as! UIButton).showsMenuAsPrimaryAction }, "no pull-down over a text popover's invoker")
        XCTAssertTrue(more.subviews.contains { $0 is UIButton && ($0 as! UIButton).showsMenuAsPrimaryAction }, "a menu's invoker keeps UIKit's pull-down")
        p.press(about.id)
        let end = Date().addingTimeInterval(3)
        while !p.menus.isOpen(pop), Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(p.menus.isOpen(pop), "the press opens it as itself")
        XCTAssertFalse(pop.isHidden)
        let anchor = about.convert(about.bounds, to: nil), frame = pop.convert(pop.bounds, to: nil)
        XCTAssertLessThan(abs(frame.minY - anchor.maxY), 20, "anchored under the button: \(frame) \(anchor)")
        p.press(about.id)
        let shut = Date().addingTimeInterval(3)
        while p.menus.isOpen(pop), Date() < shut { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
        XCTAssertFalse(p.menus.isOpen(pop), "a second press closes it")
    }

    /// A popover with no authored placement whose invoker is at the bottom
    /// (Harmony's emoji button under the composer) opens above it, as the
    /// web's default flips, rather than held over its own invoker.
    func testAPopoverFromTheBottomOpensAboveItsInvoker() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        defer { p.reset() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "emoji", "accessibilityLabel": "Emoji", "appearance": "none"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["popover": "auto", "id": "emoji"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Pick one"]],
            ["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 2, "x": 12, "y": 740, "w": 40, "h": 40],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 300, "h": 240],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 300, "h": 20],
        ]))
        let button = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[3])
        p.press(button.id)
        let end = Date().addingTimeInterval(3)
        while !p.menus.isOpen(pop), Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(p.menus.isOpen(pop))
        let anchor = button.convert(button.bounds, to: nil), frame = pop.convert(pop.bounds, to: nil)
        XCTAssertLessThanOrEqual(frame.maxY, anchor.minY + 0.5, "above its invoker: \(frame) over \(anchor)")
        XCTAssertEqual(frame.minX, anchor.minX, accuracy: 0.5, "from its left edge")
    }
}
#endif
