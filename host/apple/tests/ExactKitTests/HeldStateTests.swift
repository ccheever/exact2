#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// Held states on UIKit and AppKit (LLP 1122 D8, LLP 1121 stage 8 E2): a hold
/// is shown as the real state, kept apart from it: `active` the press of the
/// node the press resolves to through the logical tree (its press paint),
/// `hover` (the Mac) and `focus-visible` that node's look. A real press down
/// as it is held is taken over; one after lets it go at its down, as a key, the
/// pointer coming or going and the focus coming or going do. A disabled node,
/// or one under a disabled node, refuses one; disabling and going let it go.
/// A new generation lets it go unsaid and takes it again. `layout` names what
/// is held, and its rows' CSS values.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class HeldStateTests: XCTestCase {
    #if os(iOS)
    private var window: UIWindow!
    override func tearDown() { MainActor.assumeIsolated { window = nil; HeldState.letGo("replaced"); HeldState.swapping = false; HeldState.heard = nil; HeldState.frame = nil } }
    #else
    private var window: NSWindow!
    override func tearDown() { MainActor.assumeIsolated { window?.close(); window = nil; HeldState.letGo("replaced"); HeldState.swapping = false; HeldState.heard = nil; HeldState.frame = nil } }
    #endif

    private let blue: [Double] = [37, 99, 235, 255], navy: [Double] = [29, 78, 216, 255], green: [Double] = [0, 255, 0, 255]
    private func ink(_ c: [Double]) -> BatchValue { .array(c.map(BatchValue.number)) }

    /// A card (1) that presses, with a press paint and hover and focus looks;
    /// a node inside it with a view (2) and one with none (3: a flat leaf on
    /// iOS); a node beside it (4).
    @MainActor private func fixture() throws -> (Presenter, NodeView, NodeView) {
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
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": green]],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "beside"], "handlers": ["focus"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1, 4]],
            ["op": "frame", "id": 1, "x": 50.0, "y": 50.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 50.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 60.0, "y": 0.0, "w": 50.0, "h": 50.0],
            ["op": "frame", "id": 4, "x": 50.0, "y": 200.0, "w": 100.0, "h": 50.0],
        ]))
        return (p, try XCTUnwrap(p.views[1], "views \(p.views.keys.sorted())"), try XCTUnwrap(p.views[2], "views \(p.views.keys.sorted())"))
    }

    #if os(macOS)
    private func mouse(_ type: NSEvent.EventType, on view: NSView) -> NSEvent {
        let at = view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)
        return NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                  windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
    }
    #endif
    /// A real press going down on `view` (the Mac's mouse; on iOS what the
    /// window's `sendEvent` and the node's `touchesBegan` do with a contact).
    @MainActor private func down(_ view: NodeView, press card: NodeView) {
        #if os(macOS)
        view.mouseDown(with: mouse(.leftMouseDown, on: view))
        #else
        HeldState.input(at: view)
        card.pressed = true
        #endif
    }
    @MainActor private func up(_ card: NodeView) {
        card.pressPaint.at -= 1; card.press.start -= 1 // (seen: a release does not wait for the press to show)
        #if os(macOS)
        card.mouseUp(with: mouse(.leftMouseUp, on: card))
        #else
        card.pressed = false
        #endif
    }

    @MainActor func testAHeldPressShowsThePressAndARealPressAfterItLetsItGoAtItsDown() throws {
        let (p, card, inner) = try fixture()
        var dropped: [String] = [], pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        // Named on the node inside: the press it resolves to, painted, its real press untouched.
        XCTAssertEqual(inner.hold("active") { dropped.append($0) }.held, "active")
        XCTAssertTrue(card.holds("active"))
        XCTAssertFalse(card.pressed, "the real press is its own")
        XCTAssertEqual(card.style["background_color"], ink(navy), "the press paint shows")
        // A real press on it lets it go at its down, and is the real press from there.
        down(inner, press: card)
        XCTAssertEqual(dropped, ["input"])
        XCTAssertTrue(card.pressed)
        XCTAssertEqual(card.style["background_color"], ink(navy), "the real press's paint")
        up(card)
        XCTAssertEqual(card.style["background_color"], ink(blue))
        #if os(macOS)
        XCTAssertEqual(pressed, [1], "and its click")
        #endif
        XCTAssertEqual(dropped, ["input"])
    }

    @MainActor func testARealPressDownAsItIsHeldIsTakenOverAndLettingGoKeepsAPressStillDown() throws {
        let (p, card, _) = try fixture()
        var dropped: [String] = [], pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        // Down, then held: its release shows nothing and lets nothing go.
        down(card, press: card)
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        up(card)
        XCTAssertEqual(dropped, [], "a release of a press taken over is no input")
        XCTAssertEqual(card.style["background_color"], ink(navy), "the hold keeps its paint")
        XCTAssertNotNil(HeldState.rows(for: 1, in: p))
        XCTAssertEqual(card.hold(nil, dropped: nil).held, nil)
        XCTAssertEqual(dropped, ["replaced"])
        XCTAssertEqual(card.style["background_color"], ink(blue), "let go, nothing holds it down")
        // Down, held, let go: the press still down keeps its paint, and its release still clicks.
        down(card, press: card)
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        XCTAssertEqual(card.hold(nil, dropped: nil).held, nil)
        XCTAssertTrue(card.pressed)
        XCTAssertEqual(card.style["background_color"], ink(navy), "the real press is still down")
        up(card)
        XCTAssertEqual(card.style["background_color"], ink(blue))
        #if os(macOS)
        XCTAssertEqual(pressed, [1, 1], "each release a click")
        #endif
        // Down, dragged off, held, let go: off the box, the paint goes.
        down(card, press: card)
        card.pressFollows(inside: false)
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        XCTAssertEqual(card.style["background_color"], ink(navy))
        card.pressPaint.at -= 1; card.press.start -= 1
        XCTAssertEqual(card.hold(nil, dropped: nil).held, nil)
        XCTAssertEqual(card.style["background_color"], ink(blue), "a press off its box shows nothing")
    }

    @MainActor func testAKeyOnTheHeldNodeOrInsideItLetsItGoAndOneElsewhereDoesNot() throws {
        let (p, card, inner) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        _ = p.keyDown(at: p.views[4], "a")
        XCTAssertEqual(dropped, [])
        _ = p.keyDown(at: inner, "a")
        XCTAssertEqual(dropped, ["input"])
        XCTAssertEqual(card.style["background_color"], ink(blue), "the look goes with it")
    }

    @MainActor func testAHeldFocusGoesWhenTheFocusComesIntoItsNodeOrLeavesIt() throws {
        let (p, card, _) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        XCTAssertEqual(card.style["background_color"], ink(green), "the focus's look")
        p.focus(4)
        XCTAssertEqual(dropped, [], "the focus elsewhere")
        p.focus(2)
        XCTAssertEqual(dropped, ["input"], "the focus coming into the node, inside it")
        // Held with the focus inside: moving within keeps it, leaving lets it go.
        XCTAssertEqual(card.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        HeldState.focused = true // (as `hold` reads it from the window's first responder, which this test's window gives no node)
        p.focus(2)
        XCTAssertEqual(dropped, ["input"])
        p.focus(4)
        XCTAssertEqual(dropped, ["input", "input"])
    }

    @MainActor func testADisabledNodeOrAncestorRefusesAndDisablingEitherLetsGo() throws {
        let (p, card, inner) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"], "clear": []]]))
        XCTAssertEqual(dropped, ["disabled"])
        XCTAssertEqual(card.style["background_color"], ink(blue))
        XCTAssertEqual(card.hold("active", dropped: nil).reason, "disabled")
        // Under a disabled ancestor: a press stops there, and a look is refused.
        XCTAssertEqual(inner.hold("active", dropped: nil).reason, "disabled")
        XCTAssertEqual(inner.hold("focus-visible", dropped: nil).reason, "disabled")
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["disabled"]]]))
        XCTAssertEqual(inner.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"], "clear": []]]))
        XCTAssertEqual(dropped, ["disabled", "disabled"], "an ancestor disabled")
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["disabled"]]]))
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        p.apply(wireBatch([["op": "destroy", "id": 1]]))
        XCTAssertEqual(dropped, ["disabled", "disabled", "gone"])
    }

    /// `layout`'s rows are CSS values (no runner here: the batch's, as the runner writes them), on the node named and the press it resolved to.
    @MainActor func testTheHeldRowsAreCSSValuesOnTheNodeNamedAndItsPress() throws {
        let (p, card, inner) = try fixture()
        XCTAssertEqual(inner.hold("active", dropped: nil).held, "active")
        for id: UInt32 in [1, 2] {
            let held = try XCTUnwrap(HeldState.rows(for: id, in: p))
            XCTAssertEqual(held["state"] as? String, "active")
            let rows = try XCTUnwrap(held["rows"] as? [[String: Any]])
            XCTAssertEqual(rows.map { $0["name"] as? String }, ["press_background_color"])
            XCTAssertEqual(rows.first?["value"] as? String, "#1d4ed8")
        }
        XCTAssertNil(HeldState.rows(for: 4, in: p))
        XCTAssertEqual(card.hold("focus-visible", dropped: nil).held, "focus-visible")
        let rows = try XCTUnwrap(HeldState.rows(for: 1, in: p)?["rows"] as? [[String: Any]])
        XCTAssertEqual(rows.first?["name"] as? String, "background_color")
        XCTAssertEqual(rows.first?["value"] as? String, "#00ff00")
        XCTAssertNil(HeldState.rows(for: 2, in: p), "a look is the node's own")
    }

    /// A child with no view of its own (a flat leaf on iOS) presses its press;
    /// a look needs a view. A state no host knows is refused, unpainted.
    @MainActor func testAChildWithNoViewResolvesItsPressAndAnUnknownStateIsRefused() throws {
        let (p, card, _) = try fixture()
        #if os(iOS)
        XCTAssertNil(p.views[3], "a flat leaf")
        #endif
        XCTAssertEqual(HeldState.hold("active", on: 3, in: p, dropped: nil).held, "active")
        XCTAssertTrue(card.holds("active"))
        XCTAssertNotNil(HeldState.rows(for: 3, in: p))
        card.pressPaint.at -= 1; card.press.start -= 1 // (seen: its release does not wait for the press to show)
        if p.views[3] == nil { XCTAssertEqual(HeldState.hold("focus-visible", on: 3, in: p, dropped: nil).reason, "gone") }
        let r = card.hold("pressed", dropped: nil)
        XCTAssertTrue(r.unsupported)
        XCTAssertNil(HeldState.view)
        XCTAssertEqual(card.style["background_color"], ink(blue))
    }

    /// A dev loop's new generation: the hold let go unsaid before the
    /// presenter resets, real input none while it does.
    @MainActor func testANewGenerationLetsAHoldGoUnsaidBeforeThePresenterResets() throws {
        let (p, card, inner) = try fixture()
        var dropped: [String] = []
        down(card, press: card)
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        HeldState.swap()
        XCTAssertNil(HeldState.view)
        HeldState.input(at: inner)
        p.reset()
        HeldState.swapping = false
        XCTAssertEqual(dropped, [])
    }

    #if os(macOS)
    @MainActor func testAHeldHoverGoesWhenThePointerComesOrLeavesNotAsItMovesWithin() throws {
        let (p, card, inner) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("hover") { dropped.append($0) }.held, "hover")
        XCTAssertEqual(card.style["background_color"], ink(green))
        p.setHoverPath([p.views[4]!])
        XCTAssertEqual(dropped, [], "the pointer elsewhere leaves a held hover")
        p.setHoverPath([inner, card])
        XCTAssertEqual(dropped, ["input"], "the pointer coming over it, on a node inside")
        XCTAssertEqual(card.style["background_color"], ink(green), "and the real hover shows the same look")
        // Held under the pointer: moving within keeps it, leaving lets it go.
        XCTAssertEqual(card.hold("hover") { dropped.append($0) }.held, "hover")
        p.setHoverPath([card])
        p.setHoverPath([inner, card])
        XCTAssertEqual(dropped, ["input"])
        p.setHoverPath([])
        XCTAssertEqual(dropped, ["input", "input"])
        XCTAssertEqual(card.style["background_color"], ink(blue))
        // A node with no hover look is held and let go the same.
        XCTAssertEqual(inner.hold("hover") { dropped.append($0) }.held, "hover")
        p.setHoverPath([inner, card])
        XCTAssertEqual(dropped, ["input", "input", "input"])
    }

    private func plan(_ label: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try """
        component App
          state n = 0
          action go
            n = n + 1
          view
            column width="100%" height="100%"
              column testId="card" press=go background-color="#ffffff"
                :active
                  background-color = "#0000ff"
                text "\(label)"

        """.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    #if EXACT_INSPECT
    /// The real path (`ExactApp.applyTogether`): under the same plan the hold
    /// is taken again, one `held` at the same revision, nothing gone; under
    /// another nothing is said (the server's replay corresponds it).
    @MainActor func testANewGenerationTakesTheHoldAgainUnderTheSamePlanAndSaysNothingUnderAnother() throws {
        _ = NSApplication.shared
        let app = ExactApp.shared, session = app.makeSession(label: "held-state")
        defer { session.destroy() }
        let first = try plan("Label")
        XCTAssertNil(session.boot(plan: first, size: CGSize(width: 400, height: 300)).error)
        let view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        let p = session.presenter
        func node(_ id: UInt32) throws -> [String: Any] {
            try XCTUnwrap(JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"node\",\"id\":\(id)}").utf8)) as? [String: Any])
        }
        let card = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "card" })
        let label = try XCTUnwrap(Inspect.sites(session).views.map(\.id).first { id in (try? node(id))?["parent"] as? NSNumber == NSNumber(value: card.id) && id != card.id })
        let named = try node(label)
        var heard: [[String: Any]] = []
        HeldState.heard = { heard.append($0) }
        // Named on the label inside (with or without a view of its own): the card's press.
        app.holdCommand(["kind": "hold", "revision": 7, "hold": ["site": named["site"]!, "instance": named["instance"] ?? [], "state": "active"]])
        XCTAssertEqual(heard.map { $0["state"] as? String }, ["active"])
        XCTAssertTrue(card.holds("active"))
        let rows = try XCTUnwrap(HeldState.rows(for: label, in: p)?["rows"] as? [[String: Any]], "the label named answers too")
        XCTAssertEqual(rows.first?["name"] as? String, "press_background_color")
        XCTAssertEqual(rows.first?["value"] as? String, "#0000ff", "the runner's CSS value")
        XCTAssertTrue(app.apply(first))
        XCTAssertEqual(heard.count, 2, "\(heard)")
        XCTAssertEqual(heard.last?["state"] as? String, "active")
        XCTAssertEqual(heard.last?["revision"] as? Int, 7)
        XCTAssertNil(heard.last?["reason"])
        let again = try XCTUnwrap(HeldState.view)
        XCTAssertFalse(again === card, "the new generation's card")
        XCTAssertEqual(again.props["testId"], "card")
        XCTAssertTrue(app.apply(try plan("Another")))
        XCTAssertEqual(heard.count, 2, "\(heard)")
        XCTAssertNil(HeldState.view)
    }
    #endif
    #else
    @MainActor func testIOSHoldsNoHover() throws {
        let (p, card, _) = try fixture()
        defer { withExtendedLifetime(p) {} }
        XCTAssertTrue(card.hold("hover", dropped: nil).unsupported)
        XCTAssertNil(HeldState.view)
    }
    #endif
}
#endif
