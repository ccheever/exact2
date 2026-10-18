#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// Held states on UIKit and AppKit (LLP 1122 D8, LLP 1121 stage 8 E2): a hold
/// is the real state on the host's own tracking: `active` the press of the
/// node the press resolves to (its press paint), `hover` (the Mac) and
/// `focus-visible` that node's look. A disabled node refuses one; real input
/// on the node, its disabling and its going let it go, said why; iOS holds no
/// hover. `layout` names what is held, and its rows.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class HeldStateTests: XCTestCase {
    #if os(iOS)
    private var window: UIWindow!
    override func tearDown() { MainActor.assumeIsolated { window = nil; HeldState.letGo("replaced") } }
    #else
    private var window: NSWindow!
    override func tearDown() { MainActor.assumeIsolated { window?.close(); window = nil; HeldState.letGo("replaced") } }
    #endif

    private let blue: [Double] = [37, 99, 235, 255], navy: [Double] = [29, 78, 216, 255], green: [Double] = [0, 255, 0, 255]
    private func ink(_ c: [Double]) -> BatchValue { .array(c.map(BatchValue.number)) }

    private func fixture() throws -> (Presenter, NodeView, NodeView) {
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
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "style": ["background_color": blue, "press_background_color": navy, "looks": ["hover": ["background_color": green], "focus_visible": ["background_color": green]]]],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "inner"], "style": ["background_color": green]], // (a prop: no flat leaf)
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 50.0, "y": 50.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 50.0, "h": 50.0],
        ]))
        return (p, try XCTUnwrap(p.views[1], "views \(p.views.keys.sorted())"), try XCTUnwrap(p.views[2], "views \(p.views.keys.sorted())"))
    }

    func testAHeldPressIsTheRealPressUntilARealRelease() throws {
        let (_, card, inner) = try fixture()
        var dropped: [String] = []
        // Named on the node inside: the press it resolves to.
        let r = inner.hold("active") { dropped.append($0) }
        XCTAssertEqual(r.held, "active")
        XCTAssertTrue(card.pressed && card.holds("active"))
        XCTAssertEqual(card.style["background_color"], ink(navy), "the press paint shows")
        XCTAssertEqual(card.heldRows?["state"] as? String, "active")
        XCTAssertEqual((card.heldRows?["rows"] as? [[String: Any]])?.first?["name"] as? String, "press_background_color")
        // A real release lets it go.
        card.pressPaint.at -= 1; card.press.start -= 1
        card.pressed = false
        XCTAssertEqual(dropped, ["input"])
        XCTAssertNil(card.heldRows)
    }

    func testADisabledNodeRefusesAndItsDisablingAndGoingLetGo() throws {
        let (p, card, _) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        XCTAssertEqual(card.style["background_color"], ink(green), "the focus's look")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"], "clear": []]]))
        XCTAssertEqual(dropped, ["disabled"])
        XCTAssertEqual(card.style["background_color"], ink(blue))
        XCTAssertEqual(card.hold("active", dropped: nil).reason, "disabled")
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["disabled"]]]))
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        p.apply(wireBatch([["op": "destroy", "id": 1]]))
        XCTAssertEqual(dropped, ["disabled", "gone"])
    }

    /// A dev loop's new generation takes the tree: the held node going is
    /// let go unsaid (the build channel's frame is taken again after).
    func testANewGenerationLetsAHoldGoUnsaid() throws {
        let (p, card, _) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        MainActor.assumeIsolated { HeldState.swapping = true }
        p.apply(wireBatch([["op": "destroy", "id": 1]]))
        MainActor.assumeIsolated { HeldState.swapping = false }
        XCTAssertEqual(dropped, [])
        XCTAssertTrue(MainActor.assumeIsolated { HeldState.view == nil })
    }

    #if os(macOS)
    func testAHeldHoverGoesWhenThePointerComesOrLeavesNotAsItMovesWithin() throws {
        let (p, card, _) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("hover") { dropped.append($0) }.held, "hover")
        XCTAssertEqual(card.style["background_color"], ink(green))
        p.setHoverPath([])
        XCTAssertEqual(card.style["background_color"], ink(green), "the pointer elsewhere leaves a held hover")
        p.setHoverPath([card])
        XCTAssertEqual(dropped, ["input"], "a real hover takes over")
        XCTAssertEqual(card.style["background_color"], ink(green), "and shows the same look")
        // Held under the pointer: moving within keeps it, leaving lets it go.
        XCTAssertEqual(card.hold("hover") { dropped.append($0) }.held, "hover")
        p.setHoverPath([card])
        XCTAssertEqual(dropped, ["input"])
        p.setHoverPath([])
        XCTAssertEqual(dropped, ["input", "input"])
        XCTAssertEqual(card.style["background_color"], ink(blue))
    }
    #else
    func testIOSHoldsNoHover() throws {
        let (_, card, _) = try fixture()
        XCTAssertTrue(card.hold("hover", dropped: nil).unsupported)
        XCTAssertNil(card.heldRows)
    }
    #endif
}
#endif
