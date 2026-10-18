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
    override func tearDown() { MainActor.assumeIsolated { window = nil; HeldState.letGo("replaced"); HeldState.swapping = false; HeldState.heard = nil; HeldState.frame = nil; HeldState.testStream = nil } }
    #else
    private var window: NSWindow!
    override func tearDown() { MainActor.assumeIsolated { window?.close(); window = nil; HeldState.letGo("replaced"); HeldState.swapping = false; HeldState.heard = nil; HeldState.frame = nil; HeldState.testStream = nil; tableFixture?.close(); tableFixture = nil } }
    private var tableFixture: TableFixture?
    #endif

    private let blue: [Double] = [37, 99, 235, 255], navy: [Double] = [29, 78, 216, 255], green: [Double] = [0, 255, 0, 255]
    private func ink(_ c: [Double]) -> BatchValue { .array(c.map(BatchValue.number)) }

    /// A card (1) that presses, with a press paint and hover and focus looks;
    /// a node inside it with a view (2) and one with none (3: a flat leaf on
    /// iOS); a node beside it (4) that takes the focus; a native button (5);
    /// a button whose press is its default link (6) over a label (7); a
    /// button with a key shortcut (8); a press with no press rows of its own
    /// (9), which shows the platform's (iOS's dim, the Mac's tint).
    @MainActor private func fixture(session: ExactSession? = nil) throws -> (Presenter, NodeView, NodeView) {
        let p = session?.presenter ?? Presenter()
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
            ["op": "create", "id": 5, "kind": "control", "handlers": ["press"], "props": ["type": "button", "testId": "native"]],
            ["op": "create", "id": 6, "kind": "button", "props": ["href": "/elsewhere", "testId": "link"]],
            ["op": "create", "id": 7, "kind": "view", "props": ["testId": "link-label"]],
            ["op": "create", "id": 8, "kind": "button", "handlers": ["press"], "props": ["accessibilityKeyShortcuts": "Escape", "testId": "escape"]],
            ["op": "create", "id": 9, "kind": "view", "handlers": ["press"], "props": ["testId": "plain"], "style": ["background_color": blue]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "roots", "ids": [1, 4, 5, 6, 8, 9]],
            ["op": "frame", "id": 5, "x": 270.0, "y": 50.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 6, "x": 270.0, "y": 110.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 50.0, "h": 20.0],
            ["op": "frame", "id": 8, "x": 270.0, "y": 170.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 9, "x": 50.0, "y": 280.0, "w": 100.0, "h": 40.0],
            ["op": "create", "id": 10, "kind": "textarea", "props": ["value": "notes", "testId": "notes"]],
            ["op": "create", "id": 11, "kind": "control", "handlers": ["change"], "props": ["type": "checkbox", "testId": "check"]],
            ["op": "children", "id": 4, "ids": [10, 11]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 60.0, "h": 40.0],
            ["op": "frame", "id": 11, "x": 64.0, "y": 0.0, "w": 30.0, "h": 30.0],
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

    /// A key shortcut (the Mac's `routeKey`, UIKit's key command) on the
    /// focused held node lets its hold go before the shortcut takes the key.
    @MainActor func testAShortcutOnTheFocusedHeldNodeLetsItGoBeforeItPresses() throws {
        let (p, _, _) = try fixture()
        let beside = try XCTUnwrap(p.views[4])
        var dropped: [String] = []
        #if os(macOS)
        XCTAssertTrue(window.makeFirstResponder(beside))
        #else
        XCTAssertTrue(beside.becomeFirstResponder())
        #endif
        XCTAssertTrue(p.focusedNode === beside)
        XCTAssertEqual(beside.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        #if os(macOS)
        let escape = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                                    context: nil, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53))
        XCTAssertTrue(p.routeKey(escape, focused: true, in: window), "the shortcut took the key")
        #else
        p.performShortcut(UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(UIResponder.becomeFirstResponder)))
        #endif
        XCTAssertEqual(dropped, ["input"])
    }

    /// A held press on a native button is its highlight, and on a press with
    /// no press rows the platform's own; each beside a real press, which a
    /// hold landing as it is down takes over.
    @MainActor func testANativeButtonAndAPlainPressShowAHeldPressBesideTheRealOne() throws {
        let (p, _, _) = try fixture()
        let native = try XCTUnwrap(p.views[5]), plain = try XCTUnwrap(p.views[9])
        #if os(macOS)
        let button = try XCTUnwrap(p.controls.controls[5] as? NSButton)
        func highlighted() -> Bool { button.cell?.isHighlighted == true }
        func physical(_ on: Bool) { button.highlight(on) }
        func platform() -> Bool { plain.pressTinted }
        #else
        let button = try XCTUnwrap(p.controls.controls[5] as? UIButton)
        func highlighted() -> Bool { button.isHighlighted }
        func physical(_ on: Bool) { button.isHighlighted = on }
        func platform() -> Bool { plain.press.dimmedAt != nil }
        #endif
        XCTAssertEqual(native.hold("active", dropped: nil).held, "active")
        XCTAssertTrue(highlighted(), "held: the button's highlight")
        XCTAssertNil(native.hold(nil, dropped: nil).held)
        XCTAssertFalse(highlighted())
        // Down (the platform's tracking), held, up: the highlight stays till the hold goes.
        physical(true)
        XCTAssertEqual(native.hold("active", dropped: nil).held, "active")
        physical(false)
        XCTAssertTrue(highlighted(), "a release of a press taken over")
        XCTAssertNil(native.hold(nil, dropped: nil).held)
        XCTAssertFalse(highlighted())
        // A press with no press rows: the platform's, held and taken over the same.
        var dropped: [String] = []
        XCTAssertEqual(plain.hold("active") { dropped.append($0) }.held, "active")
        XCTAssertTrue(platform())
        XCTAssertNil(plain.hold(nil, dropped: nil).held)
        XCTAssertFalse(platform())
        down(plain, press: plain)
        XCTAssertTrue(platform())
        XCTAssertEqual(plain.hold("active") { dropped.append($0) }.held, "active")
        up(plain)
        XCTAssertTrue(platform(), "a release of a press taken over")
        XCTAssertNil(plain.hold(nil, dropped: nil).held)
        XCTAssertFalse(platform())
        XCTAssertEqual(dropped, ["replaced", "replaced"])
    }

    /// `active` resolves to the host's real press: a button's default link is one (theme2's 11:48).
    @MainActor func testAButtonsDefaultLinkIsThePressAnActiveHoldResolvesTo() throws {
        let (p, _, _) = try fixture()
        let link = try XCTUnwrap(p.views[6])
        XCTAssertNotNil(link.defaultLink)
        XCTAssertEqual(try XCTUnwrap(p.views[7]).hold("active", dropped: nil).held, "active")
        XCTAssertTrue(link.holds("active"))
    }

    /// A held node that starts to leave (an exit) is gone, said once; its
    /// destroy after says nothing more; and a presenter reset (its session
    /// ending) is gone too, unsaid in a generation swap.
    @MainActor func testAnExitOrAResetLetsAHoldGoOnceAsGone() throws {
        let (p, card, _) = try fixture()
        var dropped: [String] = []
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        p.apply(wireBatch([["op": "exit", "id": 1]]))
        XCTAssertEqual(dropped, ["gone"])
        XCTAssertNil(HeldState.view)
        p.apply(wireBatch([["op": "destroy", "id": 1]]))
        XCTAssertEqual(dropped, ["gone"])
        XCTAssertEqual(try XCTUnwrap(p.views[4]).hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        p.reset()
        XCTAssertEqual(dropped, ["gone", "gone"])
    }

    #if EXACT_INSPECT
    /// The fixture plan booted in a session in a window: the card and its child (a flat leaf on iOS).
    @MainActor private func booted(_ label: String) throws -> (ExactSession, Presenter, NodeView, UInt32) {
        let session = ExactApp.shared.makeSession(label: label)
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_HELD_STATE_PLAN"], "build.mjs --test compiles tests/fixtures/held-state.contract")))
        #if os(iOS)
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        session.presenter.viewport.frame = window.bounds
        window.addSubview(session.presenter.viewport)
        window.makeKeyAndVisible()
        #else
        _ = NSApplication.shared
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        #endif
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 400, height: 400)).error)
        let p = session.presenter
        let card = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "card" })
        let child = try XCTUnwrap(Inspect.sites(session).views.map(\.id).first { id in
            let node = (try? JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"node\",\"id\":\(id)}").utf8))) as? [String: Any]
            return node?["parent"] as? NSNumber == NSNumber(value: card.id)
        })
        return (session, p, card, child)
    }
    private func hold(_ app: ExactApp, _ session: ExactSession, _ id: UInt32, revision: Int, plan: String? = nil) {
        let node = (try? JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"node\",\"id\":\(id)}").utf8))) as? [String: Any] ?? [:]
        var frame: [String: Any] = ["kind": "hold", "revision": revision, "hold": ["site": node["site"] ?? NSNull(), "instance": node["instance"] ?? [], "state": "active"]]
        if let plan { frame["plan"] = plan }
        app.holdCommand(frame)
    }

    /// On the build channel: an exit then a destroy, and a session's end, each say `gone` once, at the held revision.
    @MainActor func testALeavingNodeAndAnEndingSessionSayGoneOnceAtTheHeldRevision() throws {
        var heard: [[String: Any]] = []
        HeldState.heard = { heard.append($0) }
        let (first, p, card, _) = try booted("held-exit")
        defer { first.destroy() }
        hold(ExactApp.shared, first, card.id, revision: 9)
        XCTAssertTrue(card.holds("active"), "\(heard)")
        p.apply(wireBatch([["op": "exit", "id": Int(card.id)]]))
        p.apply(wireBatch([["op": "destroy", "id": Int(card.id)]]))
        XCTAssertEqual(heard.count, 2, "\(heard)")
        XCTAssertEqual(heard.last?["reason"] as? String, "gone")
        XCTAssertEqual(heard.last?["revision"] as? Int, 9)
        first.destroy()
        heard = []
        let (second, _, again, _) = try booted("held-end")
        defer { second.destroy() }
        hold(ExactApp.shared, second, again.id, revision: 10)
        XCTAssertTrue(again.holds("active"), "\(heard)")
        second.destroy()
        XCTAssertEqual(heard.map { $0["reason"] as? String }, [nil, "gone"], "\(heard)")
        XCTAssertEqual(heard.last?["revision"] as? Int, 10)
    }

    /// Holds are their stream's: after a reconnect (a new nonce; a restarted
    /// server numbers from where the old one did) an old hold's let-go says
    /// nothing, its ended revisions are forgotten, and the same revision on
    /// the new stream is taken.
    @MainActor func testAHoldIsItsStreamsAndAReconnectReusingItsRevisionIsTaken() throws {
        var heard: [[String: Any]] = []
        HeldState.heard = { heard.append($0) }
        let (session, _, card, _) = try booted("held-stream")
        defer { session.destroy() }
        HeldState.testStream = "a"
        hold(ExactApp.shared, session, card.id, revision: 5)
        XCTAssertTrue(card.holds("active"))
        // Reconnected: the old hold's late release goes to no one.
        HeldState.testStream = "b"
        HeldState.input(at: card)
        XCTAssertEqual(heard.count, 1, "\(heard)")
        hold(ExactApp.shared, session, card.id, revision: 5)
        XCTAssertTrue(card.holds("active"), "the new stream's revision 5 is taken: \(heard)")
        HeldState.input(at: card)
        XCTAssertEqual(heard.map { $0["reason"] as? String }, [nil, nil, "input"], "\(heard)")
        // Ended on stream b; a stream c that reuses revision 5 takes it.
        HeldState.testStream = "c"
        hold(ExactApp.shared, session, card.id, revision: 5)
        XCTAssertTrue(card.holds("active"), "\(heard)")
        XCTAssertEqual(heard.count, 4, "\(heard)")
        // A frame for another plan leaves the hold and its kept frame as they are.
        hold(ExactApp.shared, session, card.id, revision: 6, plan: "another plan")
        XCTAssertTrue(card.holds("active"))
        XCTAssertEqual(HeldState.frame?.frame["revision"] as? Int, 5)
        XCTAssertEqual(heard.count, 4, "\(heard)")
    }

    #if os(iOS)
    /// The agent's tap lets a hold go at its down, on the press or on its
    /// flat child; and the agent's `layout` of that flat child answers, with
    /// the press's held rows.
    @MainActor func testAnAgentTapLetsAHoldGoAndLayoutAnswersForAFlatChild() throws {
        let (session, p, card, leaf) = try booted("held-agent")
        defer { session.destroy() }
        XCTAssertNil(p.views[leaf], "a flat leaf")
        let agent = Agent(session: session)
        var dropped: [String] = []
        XCTAssertEqual(HeldState.hold("active", on: leaf, in: p) { dropped.append($0) }.held, "active")
        XCTAssertTrue(card.holds("active"))
        let reply = agent.layout(["id": Int(leaf)])
        let node = try XCTUnwrap(reply["node"] as? [String: Any], "\(reply)")
        XCTAssertEqual((node["native"] as? [String: Any])?["flatLeaf"] as? Bool, true)
        let rows = try XCTUnwrap((node["held"] as? [String: Any])?["rows"] as? [[String: Any]], "\(node)")
        XCTAssertEqual(rows.first?["value"] as? String, "#0000ff")
        // A tap refused (its point outside the node) lets nothing go.
        let refused = agent.tap(["id": Int(card.id), "at": [900.0, 900.0]])
        XCTAssertNotNil(refused["error"], "\(refused)")
        XCTAssertEqual(dropped, [])
        XCTAssertTrue(card.holds("active"))
        // A long press or a double tap nothing there hears (no handler, no text to select) is refused, and lets nothing go.
        for event in ["contextmenu", "dblclick"] {
            let unheard = agent.tap(["id": Int(card.id), event: true])
            XCTAssertNotNil(unheard["error"], "\(event): \(unheard)")
        }
        XCTAssertEqual(dropped, [])
        XCTAssertTrue(card.holds("active"))
        func count() -> Bool { session.agent("{\"op\":\"tree\"}").contains("n 1") }
        // The flat child is no view to tap (refused, nothing let go); a tap on the press at its point lands there.
        XCTAssertNotNil(agent.tap(["id": Int(leaf)])["error"])
        XCTAssertEqual(dropped, [])
        let tapped = agent.tap(["id": Int(card.id), "at": [10.0, 10.0]])
        XCTAssertNil(tapped["error"], "\(tapped)")
        XCTAssertEqual(dropped, ["input"], "the agent's tap on the flat child's point, where its finger landed")
        p.settlePump()
        XCTAssertTrue(count(), "and the press ran: \(session.agent("{\"op\":\"tree\"}"))")
        XCTAssertEqual(card.hold("active") { dropped.append($0) }.held, "active")
        let again = agent.tap(["id": Int(card.id)])
        XCTAssertNil(again["error"], "\(again)")
        XCTAssertEqual(dropped, ["input", "input"], "the agent's tap on the press")
    }

    /// A host activation (a checkbox's, which UIKit's control takes) lets a
    /// hold go once it took, and only then; and the textarea's backspace,
    /// even one that deletes nothing.
    @MainActor func testAHostActivationAndABackspaceLetAHoldGo() throws {
        let session = ExactApp.shared.makeSession(label: "held-activation")
        defer { session.destroy() }
        let (p, _, _) = try fixture(session: session)
        let check = try XCTUnwrap(p.views[11]), notes = try XCTUnwrap(p.views[10])
        var dropped: [String] = []
        XCTAssertEqual(check.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        let reply = Agent(session: session).tap(["id": 11])
        XCTAssertEqual(reply["delivery"] as? String, "host-activation", "\(reply)")
        XCTAssertEqual(reply["native"] as? String, "control", "\(reply)")
        // (The control's own action is UIKit's target-action, which a bare test bundle has no application to send.)
        XCTAssertEqual(dropped, ["input"])
        let area = try XCTUnwrap(notes.textArea)
        XCTAssertEqual(notes.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        area.deleteBackward()
        XCTAssertEqual(dropped, ["input", "input"])
        area.text = ""
        XCTAssertEqual(notes.hold("focus-visible") { dropped.append($0) }.held, "focus-visible")
        area.deleteBackward()
        XCTAssertEqual(dropped, ["input", "input", "input"], "a backspace that deletes nothing")
    }
    #endif
    #endif

    #if os(macOS)
    @MainActor func testAHeldHoverGoesWhenThePointerComesOrLeavesNotAsItMovesWithin() throws {
        let (p, card, inner) = try fixture()
        var dropped: [String] = []
        // (The agent's pointer, as the hold hit-tests it: over the node beside.)
        let beside = p.views[4]!
        p.agentPointer = beside.convert(NSPoint(x: beside.bounds.midX, y: beside.bounds.midY), to: nil)
        XCTAssertEqual(card.hold("hover") { dropped.append($0) }.held, "hover")
        XCTAssertEqual(card.style["background_color"], ink(green))
        p.setHoverPath([beside])
        XCTAssertEqual(dropped, [], "the pointer elsewhere leaves a held hover")
        p.setHoverPath([inner, card])
        XCTAssertEqual(dropped, ["input"], "the pointer coming over it, on a node inside")
        XCTAssertEqual(card.style["background_color"], ink(green), "and the real hover shows the same look")
        // Held under the pointer: moving within keeps it, leaving lets it go.
        p.agentPointer = card.convert(NSPoint(x: card.bounds.midX, y: card.bounds.midY), to: nil)
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

    /// Events from the application's queue, delivered, until `done`.
    private func pump(_ seconds: Double = 2, until done: () -> Bool = { true }) {
        let limit = Date().addingTimeInterval(seconds)
        while Date() < limit {
            if let event = NSApp.nextEvent(matching: .any, until: Date().addingTimeInterval(0.01), inMode: .default, dequeue: true) { NSApp.sendEvent(event) }
            else if done() { return }
        }
    }

    /// A pressable table row (its hit test gives the table): a real click on
    /// it lets its held press go and presses it; a hover held on it keeps as
    /// the pointer moves within it.
    @MainActor func testATableRowsClickLetsItsHoldGoAndAMoveWithinKeepsItsHover() throws {
        let f = try TableFixture(count: 20)
        tableFixture = f
        NSApp.setActivationPolicy(.accessory)
        NSApp.activate(ignoringOtherApps: true)
        f.window.makeKeyAndOrderFront(nil)
        pump(3) { f.window.isKeyWindow && NSApp.isActive }
        try XCTSkipUnless(f.window.isKeyWindow && NSApp.isActive, "this process cannot bring a window to the front and make it key")
        XCTAssertTrue(f.window.makeFirstResponder(f.table))
        f.settle()
        let row = try f.node("row-12"), title = try f.node("title-12")
        let point = title.convert(NSPoint(x: title.bounds.midX, y: title.bounds.midY), to: nil)
        XCTAssertTrue(f.window.contentView?.hitTest(f.window.contentView?.superview?.convert(point, from: nil) ?? point) === f.table, "AppKit's hit test gives the table")
        var dropped: [String] = []
        XCTAssertEqual(row.hold("active") { dropped.append($0) }.held, "active")
        let mouse = { (type: NSEvent.EventType) in
            NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber,
                               context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        var up = false
        let timer = Timer(timeInterval: 0.05, repeats: false) { _ in NSApp.postEvent(mouse(.leftMouseUp), atStart: false); up = true }
        RunLoop.main.add(timer, forMode: .common)
        NSApp.sendEvent(mouse(.leftMouseDown))
        pump { up }
        f.settle()
        XCTAssertEqual(dropped, ["input"], "the click on the row")
        XCTAssertTrue(f.status.hasPrefix("opened 12 presses 1 "), f.status)
        // A hover held on the row with the pointer on its text: a move within keeps it.
        f.presenter.agentPointer = point
        XCTAssertEqual(row.hold("hover") { dropped.append($0) }.held, "hover")
        XCTAssertTrue(HeldState.over, "the pointer is over the row")
        var path: [NodeView] = []
        var at: NSView? = title
        while let v = at { if let n = v as? NodeView { path.append(n) }; at = v.superview }
        f.presenter.setHoverPath(path)
        XCTAssertTrue(row.holds("hover"))
        XCTAssertEqual(dropped, ["input"])
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

    /// A hover held under the pointer through a same-plan swap: the new
    /// card's hover is taken from a live hit test, so a move within it after
    /// says nothing.
    @MainActor func testAHeldHoverKeepsThroughASamePlanSwapWhileThePointerMovesWithin() throws {
        _ = NSApplication.shared
        let app = ExactApp.shared, session = app.makeSession(label: "held-hover-swap")
        defer { session.destroy() }
        let first = try plan("Label")
        XCTAssertNil(session.boot(plan: first, size: CGSize(width: 400, height: 300)).error)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        let p = session.presenter
        let card = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "card" })
        p.agentPointer = card.convert(NSPoint(x: card.bounds.midX, y: card.bounds.midY), to: nil)
        let node = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"node\",\"id\":\(card.id)}").utf8)) as? [String: Any])
        var heard: [[String: Any]] = []
        HeldState.heard = { heard.append($0) }
        app.holdCommand(["kind": "hold", "revision": 8, "hold": ["site": node["site"]!, "instance": node["instance"] ?? [], "state": "hover"]])
        XCTAssertTrue(card.holds("hover"), "\(heard)")
        XCTAssertTrue(app.apply(first))
        let again = try XCTUnwrap(HeldState.view)
        XCTAssertFalse(again === card)
        var path: [NodeView] = []
        var at: NSView? = again.subviews.compactMap { $0 as? NodeView }.first ?? again
        while let v = at { if let n = v as? NodeView { path.append(n) }; at = v.superview }
        p.setHoverPath(path)
        p.setHoverPath(Array(path.drop { $0 !== again }))
        XCTAssertTrue(again.holds("hover"), "a move within: \(heard)")
        XCTAssertEqual(heard.map { $0["reason"] as? String }, [nil, nil], "\(heard)")
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
