#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class CustomInvokerIOSTests: XCTestCase {
    private final class Controller: UIViewController {
        var presentations = 0
        override func present(_ controller: UIViewController, animated: Bool, completion: (() -> Void)? = nil) {
            presentations += 1
            super.present(controller, animated: animated, completion: completion)
        }
    }
    private final class Touch: UITouch {
        let target: UIView
        var point: CGPoint?
        init(_ target: UIView) { self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint {
            target.convert(point ?? CGPoint(x: target.bounds.midX, y: target.bounds.midY), to: view)
        }
    }
    private final class Key: UIKey {
        var text = "\r"
        var code = UIKeyboardHIDUsage.keyboardReturnOrEnter
        override var characters: String { text }
        override var charactersIgnoringModifiers: String { text }
        override var keyCode: UIKeyboardHIDUsage { code }
        override var modifierFlags: UIKeyModifierFlags { [] }
    }
    private final class Press: UIPress {
        let value = Key()
        override var key: UIKey? { value }
    }

    private var window: UIWindow?
    private var session: ExactSession?
    private var kept: Presenter?
    private var hasScene: Bool { UIApplication.shared.connectedScenes.contains { $0 is UIWindowScene } }

    override func tearDown() {
        if let p = kept, let controller = window?.rootViewController {
            if window?.windowScene != nil { settle { !p.menus.inTransition } }
            p.menus.reset()
            if window?.windowScene != nil { settle { controller.presentedViewController == nil && !p.menus.inTransition } }
        }
        session?.destroy()
        session = nil
        kept = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func settle(_ done: () -> Bool, file: StaticString = #filePath, line: UInt = #line) {
        let end = Date().addingTimeInterval(5)
        while !done(), Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(done(), "the presentation settled", file: file, line: line)
    }
    private func delivered() {
        let turn = expectation(description: "activation and its toggle task delivered")
        DispatchQueue.main.async { DispatchQueue.main.async { turn.fulfill() } }
        wait(for: [turn], timeout: 5)
    }

    private func tap(_ agent: Agent, _ id: Int) -> [String: Any] {
        // Bare XCTest runs without agent mode. Give its recognized tap the
        // authored hit tree that agentPaints supplies; test the overlay separately.
        // Sync can reattach a removed overlay, so leave it mounted but noninteractive.
        if !ExactEnv.agentPaints, let p = session?.presenter {
            for node in p.views.values where node.kind == "button" {
                for case let overlay as UIButton in node.subviews { overlay.isUserInteractionEnabled = false }
            }
        }
        return agent.tap(["id": id])
    }

    /// A pressable page (1), its custom invoker (2) and text (3), and two popovers (4, 6).
    private func page(session: ExactSession? = nil) -> Presenter {
        self.session = session
        let p = session?.presenter ?? Presenter()
        kept = p
        // Agent needs a session, but this wire tree has no corresponding runner plan.
        // Layout covers, intrinsic reports and events must not apply that plan here.
        p.session = nil
        p.onIntrinsic = nil; p.onCovers = nil; p.onClipboard = nil; p.onClose = nil
        p.onPress = nil; p.onFocus = nil; p.onBlur = nil; p.onFocusEvent = nil; p.onKey = nil
        p.buttonFace = { _ in ButtonFace() }
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        let controller = Controller()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        controller.view.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 2, "kind": "button", "props": ["id": "opener", "popovertarget": "choices"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Open choices"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "choices", "popover": "auto"], "handlers": ["toggle"]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Choice"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["id": "other", "popover": "auto"], "handlers": ["toggle"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 4, 6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 2, "x": 20, "y": 40, "w": 160, "h": 44],
            ["op": "frame", "id": 3, "x": 10, "y": 10, "w": 120, "h": 24],
            ["op": "frame", "id": 4, "x": 20, "y": 120, "w": 200, "h": 160],
            ["op": "frame", "id": 5, "x": 20, "y": 20, "w": 160, "h": 40],
            ["op": "frame", "id": 6, "x": 20, "y": 320, "w": 200, "h": 160],
        ]))
        p.viewport.layoutIfNeeded()
        return p
    }

    func testAHandlerlessPopoverInvokerShowsItsPressWhileForwardingTheContact() throws {
        let p = page(), source = try XCTUnwrap(p.views[2]), parent = try XCTUnwrap(p.views[1])
        let pop = try XCTUnwrap(p.views[4])
        XCTAssertFalse(p.menus.pullsDown(pop))
        XCTAssertNil(source.subviews.first { $0 is InvokerButton })
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let touch = Touch(source)
        source.touchesBegan([touch], with: nil)
        XCTAssertFalse(source.pressed, "the invoker does not take its ancestor's press")
        XCTAssertTrue(parent.pressed)
        XCTAssertNotNil(source.press.dimmedAt, "the custom invoker still shows the platform's press")
        touch.point = CGPoint(x: source.bounds.maxX + 20, y: source.bounds.midY)
        source.touchesMoved([touch], with: nil)
        XCTAssertNil(source.press.dimmedAt, "the finger left the invoker")
        touch.point = nil
        source.touchesMoved([touch], with: nil)
        XCTAssertNotNil(source.press.dimmedAt, "the finger came back")
        source.touchesEnded([touch], with: nil)
        delivered()
        XCTAssertEqual(presses, [1], "the ancestor hears the press once")
        XCTAssertFalse(parent.pressed)
        XCTAssertFalse(source.pressed)
        XCTAssertNil(source.press.dimmedAt)
        XCTAssertTrue(p.menus.isOpen(pop))
    }

    func testACancelledInvokerContactClearsItsLookAndTheForwardedPress() throws {
        let p = page(), source = try XCTUnwrap(p.views[2]), parent = try XCTUnwrap(p.views[1])
        var presses = 0
        p.onPress = { _ in presses += 1 }
        let touch = Touch(source)
        source.touchesBegan([touch], with: nil)
        XCTAssertNotNil(source.press.dimmedAt)
        source.touchesCancelled([touch], with: nil)
        XCTAssertNil(source.press.dimmedAt)
        XCTAssertFalse(parent.pressed)
        XCTAssertEqual(presses, 0)
        source.touchesBegan([touch], with: nil)
        XCTAssertNotNil(source.press.dimmedAt)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["disabled": "true"]]]))
        XCTAssertNil(source.press.dimmedAt, "disabling retires the look with its contact")
        XCTAssertFalse(parent.pressed)
    }

    /// The activation target is the nearest button naming a target, resolved
    /// after the press handler, which may mount it; a link's activation is its
    /// own; an agent tap at a point light-dismisses by what the point lands on.
    func testActivationResolvesAfterTheHandlerSkipsLinksAndDismissesByTheLanding() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["popovertarget": "late"]]]))
        p.onPress = { id in
            guard id == 1, p.views[30] == nil else { return }
            p.apply(wireBatch([
                ["op": "create", "id": 30, "kind": "view", "props": ["id": "late", "popover": "auto"]],
                ["op": "create", "id": 31, "kind": "text", "props": ["text": "Late"]],
                ["op": "children", "id": 30, "ids": [31]],
                ["op": "children", "id": 1, "ids": [2, 4, 6, 30]],
                ["op": "frame", "id": 30, "x": 20, "y": 500, "w": 200, "h": 80],
                ["op": "frame", "id": 31, "x": 10, "y": 10, "w": 160, "h": 40],
            ]))
        }
        let opener = try XCTUnwrap(p.views[2])
        let contact = Touch(opener)
        opener.touchesBegan([contact], with: nil)
        XCTAssertNil(p.views[30], "the ancestor has not mounted the target yet")
        XCTAssertFalse(opener.pressed, "the ancestor still owns the press")
        XCTAssertTrue(try XCTUnwrap(p.views[1]).pressed)
        XCTAssertNotNil(opener.press.dimmedAt, "a declared opening invocation shows contact before its target exists")
        opener.touchesEnded([contact], with: nil)
        delivered()
        XCTAssertNil(opener.press.dimmedAt)
        XCTAssertTrue(p.menus.isOpen(try XCTUnwrap(p.views[30])), "the target its ancestor's press mounted opens")
        p.menus.action("hidePopover", ["late"])
        p.onPress = nil

        // A tap at a point on the opener's text, addressed to the page.
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["popovertarget": "choices"]]]))
        let pop = try XCTUnwrap(p.views[4])
        p.menus.action("showPopover", ["choices", "opener"])
        XCTAssertTrue(p.menus.isOpen(pop))
        // The overlay `sync` laid over the newly resolvable invoker, as the `tap` helper does.
        for case let overlay as UIButton in try XCTUnwrap(p.views[2]).subviews { overlay.isUserInteractionEnabled = false }
        // Under agent mode `agentTap` light-dismisses by the view the point lands on.
        let landing = try XCTUnwrap(agent.tapLanding(["id": 1, "at": [80.0, 60.0]], node: try XCTUnwrap(p.views[1])))
        XCTAssertTrue(landing.isDescendant(of: try XCTUnwrap(p.views[3])), "the opener's text, not the page it was addressed to")
        p.menus.lightDismiss(landing)
        XCTAssertTrue(p.menus.isOpen(pop), "a touch inside its invoker does not light-dismiss it")
        let at = agent.tap(["id": 1, "at": [80.0, 60.0]])
        XCTAssertNil(at["error"], "\(at)")
        delivered()
        XCTAssertFalse(p.menus.isOpen(pop), "kept by light dismiss inside its invoker, then toggled closed once: \(at)")

        // A link is its own activation target: its popover attributes invoke nothing.
        let source = try XCTUnwrap(p.views[2])
        source.props["href"] = "https://example.com/"
        source.handlers = ["press"]
        p.press(2)
        XCTAssertFalse(p.menus.isOpen(pop), "a link opens no popover")
    }

    func testPresenterPressTogglesCustomInvokersWithAndWithoutHandlers() throws {
        let p = page()
        let source = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[4])
        var presses: [UInt32] = [], events: [String] = []
        p.onPress = { id in
            XCTAssertEqual(p.menus.isOpen(pop), presses.count % 2 == 1, "the handler precedes activation")
            presses.append(id)
        }
        p.onClipboard = { id, kind, value in if id == 4 && kind == 50 { events.append(value) } }
        for hasHandler in [false, true] {
            source.handlers = hasHandler ? ["press"] : []
            p.press(source.id)
            XCTAssertTrue(p.menus.isOpen(pop))
            XCTAssertEqual(p.menus.presented.filter { $0 === pop }.count, 1)
            delivered()
            XCTAssertTrue(p.menus.isOpen(pop), "no deferred second toggle")
            p.press(source.id)
            XCTAssertFalse(p.menus.isOpen(pop))
            delivered()
        }
        XCTAssertEqual(presses, [2, 2, 2, 2])
        XCTAssertEqual(events, ["closed,open", "open,closed", "closed,open", "open,closed"])
        source.props["commandfor"] = "choices"
        source.props["command"] = "toggle-popover"
        p.press(source.id)
        XCTAssertTrue(p.menus.isOpen(pop), "commandfor uses the same custom activation")
        p.press(source.id)
        XCTAssertFalse(p.menus.isOpen(pop))
    }

    func testAgentTapsInvokerAndChildKeepTheAncestorsPressAndToggleOnce() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        let source = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[4])
        XCTAssertFalse(source.takesPress, "an invoker without a handler still passes the press")
        var presses: [UInt32] = [], events: [String] = []
        p.onPress = { id in
            XCTAssertEqual(p.menus.isOpen(pop), presses.count % 2 == 1)
            presses.append(id)
        }
        p.onClipboard = { id, kind, value in if id == 4 && kind == 50 { events.append(value) } }
        for id in [2, 3] {
            for open in [true, false] {
                let reply = tap(agent, id)
                XCTAssertNil(reply["error"], "\(reply)")
                XCTAssertEqual(reply["pressed"] as? Int, 1, "the page still hears the press")
                delivered()
                XCTAssertEqual(p.menus.isOpen(pop), open, "\(reply)")
            }
        }
        XCTAssertEqual(presses, [1, 1, 1, 1])
        XCTAssertEqual(events, ["closed,open", "open,closed", "closed,open", "open,closed"])
        try XCTUnwrap(p.views[1]).handlers = []
        let reply = tap(agent, 3)
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(reply["pressed"] is NSNull)
        delivered()
        XCTAssertTrue(p.menus.isOpen(pop), "default activation also runs when no ancestor handles the press")
        XCTAssertEqual(presses, [1, 1, 1, 1])
    }

    /// With no press target the lift still activates the invoker, but a
    /// node on its way disabled and re-enabled since the down ends it, as
    /// it ends a real touch's contact.
    func testAHeldAgentContactWithOnlyAnInvokerStaysCancelledOnceDisabled() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        let pop = try XCTUnwrap(p.views[4])
        try XCTUnwrap(p.views[1]).handlers = []
        _ = tap(agent, 3) // leaves the overlays noninteractive, and opens it
        delivered()
        XCTAssertTrue(p.menus.isOpen(pop))
        p.menus.action("hidePopover", ["choices"])
        for disables in [false, true] {
            let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 3]))
            XCTAssertNil(down["error"], "\(down)")
            if disables {
                p.apply(wireBatch([["op": "props", "id": 2, "set": ["disabled": "true"]]]))
                p.apply(wireBatch([["op": "props", "id": 2, "clear": ["disabled"]]]))
            }
            _ = try XCTUnwrap(agent.recognizedPan("up", [:]))
            delivered()
            XCTAssertEqual(p.menus.isOpen(pop), !disables, disables ? "disabled and back: the lift opens nothing" : "the lift opens it")
            p.menus.action("hidePopover", ["choices"])
        }
    }

    /// A field takes its tap itself: nothing above it is pressed or invoked,
    /// and a cancelled contact with no press target moves no focus.
    func testAFieldInsideAnInvokerInvokesNothingAndACancelledContactKeepsTheFocus() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "input", "props": ["value": "inside"]],
            ["op": "frame", "id": 9, "x": 10, "y": 4, "w": 140, "h": 36],
            ["op": "children", "id": 2, "ids": [9]],
            ["op": "create", "id": 10, "kind": "input", "props": ["value": "page"]],
            ["op": "frame", "id": 10, "x": 20, "y": 600, "w": 200, "h": 40],
            ["op": "children", "id": 1, "ids": [2, 4, 6, 10]],
        ]))
        try XCTUnwrap(p.views[1]).handlers = []
        let pop = try XCTUnwrap(p.views[4])
        let reply = tap(agent, 9)
        XCTAssertNil(reply["error"], "\(reply)")
        delivered()
        XCTAssertFalse(p.menus.isOpen(pop), "the field consumed the tap")
        // With a `press` of its own, that press runs and still nothing above it is invoked.
        let field = try XCTUnwrap(p.views[9])
        field.handlers = ["press"]
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let own = tap(agent, 9)
        XCTAssertNil(own["error"], "\(own)")
        delivered()
        XCTAssertEqual(presses, [9])
        XCTAssertFalse(p.menus.isOpen(pop), "the field consumed the tap, its own press included")
        p.menus.action("hidePopover", ["choices"])
        XCTAssertTrue(field.activate(at: field.convert(CGPoint(x: field.bounds.midX, y: field.bounds.midY), to: nil)) === field)
        XCTAssertEqual(presses, [9, 9])
        XCTAssertFalse(p.menus.isOpen(pop), "VoiceOver's activation at the field invokes nothing above it either")
        field.handlers = []

        p.apply(wireBatch([["op": "children", "id": 2, "ids": [3]]]))
        let page = try XCTUnwrap(p.views[10]?.field)
        XCTAssertTrue(page.becomeFirstResponder())
        let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 3]))
        XCTAssertNil(down["error"], "\(down)")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["disabled": "true"]]]))
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["disabled"]]]))
        _ = try XCTUnwrap(agent.recognizedPan("up", [:]))
        delivered()
        XCTAssertFalse(p.menus.isOpen(pop))
        XCTAssertTrue(page.isFirstResponder, "a cancelled contact settles no focus")
    }

    /// A touch in a lifted popover authored inside the invoker of another is
    /// inside that invoker (HTML's light dismiss walks the DOM ancestors).
    func testLightDismissKeepsAPopoverWhoseInvokerHoldsTheTouch() throws {
        let p = page()
        p.apply(wireBatch([
            ["op": "props", "id": 6, "set": ["popover": "manual"]],
            ["op": "create", "id": 7, "kind": "text", "props": ["text": "Inside the invoker"]],
            ["op": "frame", "id": 7, "x": 10, "y": 10, "w": 120, "h": 24],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "children", "id": 2, "ids": [3, 6]],
            ["op": "children", "id": 1, "ids": [2, 4]],
        ]))
        let pop = try XCTUnwrap(p.views[4]), inner = try XCTUnwrap(p.views[6]), row = try XCTUnwrap(p.views[7])
        p.menus.action("showPopover", ["other"])
        p.menus.action("showPopover", ["choices", "opener"])
        XCTAssertTrue(p.menus.isOpen(inner) && p.menus.isOpen(pop))
        XCTAssertFalse(row.isDescendant(of: try XCTUnwrap(p.views[2])), "the inner popover is lifted out of the invoker")
        p.menus.lightDismiss(row)
        XCTAssertTrue(p.menus.isOpen(pop), "the touch is inside the invoker of `choices`")
        p.menus.lightDismiss(try XCTUnwrap(p.views[1]))
        XCTAssertFalse(p.menus.isOpen(pop), "a touch outside both dismisses it")
    }

    func testAgentTapOnInvokerWithHandlerDispatchesAndOpensOnce() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        let source = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[4])
        source.handlers = ["press"]
        var presses: [UInt32] = []
        p.onPress = { id in
            XCTAssertFalse(p.menus.isOpen(pop), "invocation follows the handler")
            presses.append(id)
        }
        let reply = tap(agent, 2)
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["pressed"] as? Int, 2)
        XCTAssertTrue(p.menus.isOpen(pop), "activation finishes with the accepted press")
        delivered()
        XCTAssertTrue(p.menus.isOpen(pop), "agentTap must not toggle it again")
        XCTAssertEqual(presses, [2])
    }

    func testPassThroughActivationUsesOnlyTheNearestInvokerAndItsLiveTarget() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        p.apply(wireBatch([
            ["op": "create", "id": 8, "kind": "button", "props": ["popovertarget": "other"], "handlers": ["press"]],
            ["op": "children", "id": 8, "ids": [2]],
            ["op": "children", "id": 1, "ids": [8, 4, 6]],
            ["op": "frame", "id": 8, "x": 0, "y": 0, "w": 300, "h": 100],
        ]))
        let source = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[4]), other = try XCTUnwrap(p.views[6])
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let reply = tap(agent, 3)
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["pressed"] as? Int, 8)
        delivered()
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertFalse(p.menus.isOpen(other), "the farther invoker hears the press but does not activate")
        p.menus.action("hidePopover", ["choices"])
        p.onPress = { id in presses.append(id); source.props["popovertarget"] = "other" }
        _ = tap(agent, 3)
        delivered()
        XCTAssertFalse(p.menus.isOpen(pop))
        XCTAssertTrue(p.menus.isOpen(other), "read the nearest invoker's target after its ancestor's handler")
        XCTAssertEqual(presses, [8, 8])
    }

    /// The overlay holds the invoker's press look; a second contact's press
    /// drops `popovertarget`, and with it the overlay. Once that contact
    /// retires, nothing holds the look.
    func testAContactWhosePressDropsTheOverlayLeavesNoPressLook() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session)
        let source = try XCTUnwrap(p.views[2])
        source.handlers = ["press"]
        // A menu's invoker keeps the pull-down overlay (a text popover has none, LLP 1021 D3).
        p.apply(wireBatch([["op": "create", "id": 7, "kind": "button", "props": [String: String](), "handlers": ["press"]],
                           ["op": "children", "id": 4, "ids": [7]]]))
        p.menus.sync()
        let overlay = try XCTUnwrap(source.subviews.compactMap { $0 as? InvokerButton }.first, "\(source.subviews)")
        overlay.isHighlighted = true
        XCTAssertTrue(source.pressed, "the overlay's highlight holds the look")
        p.onPress = { id in
            guard id == 2 else { return }
            p.apply(wireBatch([["op": "props", "id": 2, "set": [String: String](), "clear": ["popovertarget"]]]))
        }
        let touch = Touch(source)
        source.touchesBegan([touch], with: nil)
        source.touchesEnded([touch], with: nil)
        XCTAssertNil(source.subviews.first { $0 is InvokerButton }, "the press dropped the overlay")
        XCTAssertFalse(source.hasPressContact)
        XCTAssertFalse(source.pressed, "no holder is left")
    }

    /// A press crossing out of a popover keeps the path `PointerRecognizer`
    /// took before it sent `pointerdown`: a node there that handler disables
    /// and restores cancels the agent's press, as it does a finger's.
    func testAPointerdownCancellingACrossingPressCancelsTheAgentsToo() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        let root = try XCTUnwrap(p.views[1]), row = try XCTUnwrap(p.views[5])
        row.handlers = ["pointerdown"]
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.onPointer = { id, phase, _ in
            guard id == row.id, phase == .down else { return }
            p.apply(wireBatch([["op": "props", "id": root.id, "set": ["disabled": "true"]]]))
            p.apply(wireBatch([["op": "props", "id": root.id, "clear": ["disabled"]]]))
        }
        p.menus.action("showPopover", ["choices"])
        let reply = tap(agent, 5)
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(reply["pressed"] is NSNull, "\(reply)")
        XCTAssertEqual(presses, [])
    }

    func testPopoverBubblingInvokesWithAndWithoutAnInvokerHandler() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        p.apply(wireBatch([
            ["op": "props", "id": 2, "set": ["popovertarget": "other"]],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "children", "id": 1, "ids": [2, 6]],
        ]))
        let source = try XCTUnwrap(p.views[2]), row = try XCTUnwrap(p.views[5]), other = try XCTUnwrap(p.views[6])
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        for hasHandler in [true, false] {
            source.handlers = hasHandler ? ["press"] : []
            let expected: UInt32 = hasHandler ? 2 : 1
            p.menus.action("showPopover", ["choices"])
            let touch = Touch(row)
            XCTAssertEqual(row.activationTarget(at: touch.location(in: nil))?.id, expected)
            row.touchesBegan([touch], with: nil)
            XCTAssertTrue(p.views[expected]?.pressed == true)
            row.touchesEnded([touch], with: nil)
            XCTAssertTrue(p.menus.isOpen(other), "a bubbled page invocation may open another popover")
            p.menus.action("hidePopover", ["other"])
            p.menus.action("showPopover", ["choices"])
            let reply = tap(agent, 5)
            XCTAssertNil(reply["error"], "\(reply)")
            XCTAssertEqual(reply["pressed"] as? Int, Int(expected))
            delivered()
            XCTAssertTrue(p.menus.isOpen(other))
            p.menus.action("hidePopover", ["other"])
        }
        XCTAssertEqual(presses, [2, 2, 1, 1])
        p.apply(wireBatch([
            ["op": "create", "id": 8, "kind": "control", "props": ["type": "button"]],
            ["op": "children", "id": 4, "ids": [8]],
            ["op": "frame", "id": 8, "x": 20, "y": 20, "w": 160, "h": 40],
        ]))
        p.menus.action("showPopover", ["choices"])
        try XCTUnwrap(p.controls.controls[8]).sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(presses, [2, 2, 1, 1, 1])
        XCTAssertTrue(p.menus.isOpen(other), "a native button's bubbled press also activates the custom invoker it passed")
        // With no press anywhere above it, its action still activates the invoker.
        p.menus.action("hidePopover", ["other"])
        try XCTUnwrap(p.views[1]).handlers = []
        p.menus.action("showPopover", ["choices"])
        try XCTUnwrap(p.controls.controls[8]).sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(presses, [2, 2, 1, 1, 1])
        XCTAssertTrue(p.menus.isOpen(other), "no press target: the invoker still activates")
    }

    func testSheetBubblingCannotOpenOutsideTheModalButPageActivationCan() throws {
        guard hasScene else { throw missing("scene", "a bubbled sheet press needs a presented sheet") }
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        p.apply(wireBatch([
            ["op": "create", "id": 8, "kind": "view", "props": ["id": "form", "semanticTag": "dialog"]],
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Form content"]],
            ["op": "children", "id": 8, "ids": [9]],
            ["op": "children", "id": 2, "ids": [3, 8]],
            ["op": "frame", "id": 8, "x": 0, "y": 100, "w": 300, "h": 180],
            ["op": "frame", "id": 9, "x": 20, "y": 20, "w": 160, "h": 40],
        ]))
        let source = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[4]), row = try XCTUnwrap(p.views[9])
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.dialogCommand("showModal", "form")
        settle { !p.menus.inTransition }
        XCTAssertTrue(window?.rootViewController?.presentedViewController is DialogSheetController)
        for hasHandler in [true, false] {
            source.handlers = hasHandler ? ["press"] : []
            let expected: UInt32 = hasHandler ? 2 : 1
            let touch = Touch(row)
            XCTAssertEqual(row.activationTarget(at: touch.location(in: nil))?.id, expected)
            row.touchesBegan([touch], with: nil)
            row.touchesEnded([touch], with: nil)
            let reply = tap(agent, 9)
            XCTAssertNil(reply["error"], "\(reply)")
            XCTAssertEqual(reply["pressed"] as? Int, Int(expected))
            delivered()
            XCTAssertFalse(p.menus.isOpen(pop), "the page's listener cannot invoke outside the modal")
        }
        XCTAssertEqual(presses, [2, 2, 1, 1])
        p.dialogCommand("close", "form")
        settle { !p.menus.inTransition && self.window?.rootViewController?.presentedViewController == nil }
        p.press(source.id)
        XCTAssertTrue(p.menus.isOpen(pop), "the same custom invoker opens once the modal boundary is gone")
        p.menus.action("hidePopover", ["choices"])
        source.props["commandfor"] = "form"; source.props["command"] = "show-modal"
        p.press(source.id)
        settle { !p.menus.inTransition }
        XCTAssertTrue(window?.rootViewController?.presentedViewController is DialogSheetController,
                      "a custom commandfor opens the ordinary dialog sheet")
        source.props["command"] = "close"
        p.press(source.id)
        settle { !p.menus.inTransition && self.window?.rootViewController?.presentedViewController == nil }
    }

    func testKeyboardAndAccessibilityActivateAHandlerlessInvoker() throws {
        let session = ExactApp.shared.makeSession(label: "custom-invoker")
        let p = page(session: session), agent = Agent(session: session)
        let source = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[4])
        var presses: [UInt32] = [], handoffs: [Bool] = []
        p.onPress = { presses.append($0); handoffs.append(p.pressHandoff?.working == true) }
        XCTAssertTrue(source.becomeFirstResponder())
        let enter = Press()
        source.pressesBegan([enter], with: nil)
        source.pressesEnded([enter], with: nil)
        XCTAssertTrue(p.menus.isOpen(pop))
        let space = Press()
        space.value.text = " "; space.value.code = .keyboardSpacebar
        source.pressesBegan([space], with: nil)
        source.pressesEnded([space], with: nil)
        XCTAssertFalse(p.menus.isOpen(pop))
        for key in ["Enter", "Space"] {
            let reply = agent.type(["id": 2, "key": key])
            XCTAssertNil(reply["error"], "\(reply)")
            XCTAssertEqual(p.menus.isOpen(pop), key == "Enter")
        }
        XCTAssertTrue(source.accessibilityActivate())
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertEqual(presses, [1, 1, 1, 1, 1])
        XCTAssertEqual(handoffs.last, true, "VoiceOver's activation hands the press's focus on, as a tap's does")
    }

    func testCustomConfirmationInvokesOnceAndItsOverlayDispatchDoesNotReenter() throws {
        guard hasScene else { throw missing("scene", "confirmation presentation and dismissal need a window scene") }
        let p = page()
        p.apply(wireBatch([
            ["op": "props", "id": 4, "set": ["accessibilityRole": "alertdialog", "accessibilityLabel": "Keep it?"]],
            ["op": "create", "id": 10, "kind": "button", "props": ["popovertarget": "choices", "popovertargetaction": "hide", "accessibilityLabel": "Keep"], "handlers": ["press"]],
            ["op": "create", "id": 11, "kind": "button", "props": ["popovertarget": "choices", "popovertargetaction": "hide", "accessibilityLabel": "Cancel"]],
            ["op": "create", "id": 12, "kind": "text", "props": ["text": "Keep this choice."]],
            ["op": "create", "id": 13, "kind": "text", "props": ["text": "Keep"]],
            ["op": "create", "id": 14, "kind": "text", "props": ["text": "Cancel"]],
            ["op": "children", "id": 10, "ids": [13]], ["op": "children", "id": 11, "ids": [14]],
            ["op": "children", "id": 4, "ids": [12, 10, 11]],
        ]))
        let source = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[4])
        let controller = try XCTUnwrap(window?.rootViewController as? Controller)
        source.handlers = ["press"]
        var presses: [UInt32] = [], events: [String] = []
        p.onPress = { id in XCTAssertFalse(p.menus.isOpen(pop)); presses.append(id) }
        p.onClipboard = { id, kind, value in if id == 4 && kind == 50 { events.append(value) } }
        p.press(source.id)
        settle { !p.menus.inTransition }
        delivered()
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(controller.presentedViewController is UIAlertController)
        XCTAssertEqual(controller.presentations, 1)
        XCTAssertEqual(presses, [2])
        XCTAssertEqual(events, ["closed,open"])

        p.menus.action("hidePopover", ["choices"])
        settle { controller.presentedViewController == nil && !p.menus.inTransition }
        delivered()
        let overlay = try XCTUnwrap(source.subviews.compactMap { $0 as? UIButton }.first)
        overlay.sendActions(for: .touchUpInside)
        settle { !p.menus.inTransition }
        delivered()
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertEqual(controller.presentations, 2, "the overlay retains sole ownership of its second presentation")
        XCTAssertEqual(presses, [2, 2], "openConfirmation dispatches the handler without recursively invoking")
        XCTAssertEqual(events, ["closed,open", "open,closed", "closed,open"])

        p.menus.action("hidePopover", ["choices"])
        settle { controller.presentedViewController == nil && !p.menus.inTransition }
        let menuAction = try XCTUnwrap(p.menus.items(of: try XCTUnwrap(p.views[1])).first as? UIAction)
        menuAction.performWithSender(nil, target: nil)
        XCTAssertFalse(p.menus.isOpen(pop), "a menu row waits until the menu has left before presenting its confirmation")
        delivered()
        settle { !p.menus.inTransition }
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertEqual(controller.presentations, 3)
        XCTAssertEqual(presses, [2, 2, 2])

        p.menus.action("hidePopover", ["choices"])
        settle { controller.presentedViewController == nil && !p.menus.inTransition }
        p.onPress = { id in presses.append(id); source.props["popovertarget"] = "other" }
        overlay.sendActions(for: .touchUpInside)
        delivered()
        XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[6])), "a native confirmation's handler must not open a painted menu when it retargets")
        XCTAssertNil(controller.presentedViewController)
        XCTAssertEqual(controller.presentations, 3)
        XCTAssertEqual(presses, [2, 2, 2, 2])

        // A handler that retargets it to hide an open popover hides it, as
        // its press did before the overlay presented anything itself.
        source.props["popovertarget"] = "choices"
        p.menus.action("showPopover", ["other"])
        let other = try XCTUnwrap(p.views[6])
        XCTAssertTrue(p.menus.isOpen(other))
        p.onPress = { id in presses.append(id); source.props["popovertarget"] = "other"; source.props["popovertargetaction"] = "hide" }
        overlay.sendActions(for: .touchUpInside)
        delivered()
        XCTAssertFalse(p.menus.isOpen(other), "the retargeted command still runs")
        XCTAssertEqual(controller.presentations, 3)
        XCTAssertEqual(presses, [2, 2, 2, 2, 2])
    }
}
#endif
