#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// Platform timing's first `clock` (LLP 1080.000 §12): the host ran on the
/// wall's time until then, motion included, so the clock is taken over at the
/// wall and never set behind it; a hold begun after it is not refused.
final class AgentClockIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func booted() throws -> ExactSession {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "agent-clock")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        return session
    }

    func testARecognizedPanAlsoDeliversThePointerObserverInOrder() throws {
        let session = try booted(), p = session.presenter
        let node = NodeView(id: UInt32.max, kind: "view", presenter: p)
        p.views[node.id] = node
        node.frame = CGRect(x: 0, y: 0, width: 100, height: 100)
        p.viewport.addSubview(node)
        node.handlers = ["pan", "panrelease", "pointerdown", "pointermove", "pointerup"]
        let agent = Agent(session: session)
        var events: [String] = []
        p.onPointer = { _, kind, sample in
            events.append(kind == .down ? "down" : kind == .up ? "up" : "move")
            XCTAssertEqual(sample.buttons, kind == .up ? 0 : 1)
        }
        p.onPan = { _, _, _ in events.append("pan") }
        p.onPanRelease = { _, _, _ in events.append("release") }
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(node.id)])?["delivery"] as? String, "recognized")
        _ = agent.recognizedPan("move", ["dx": 10.0, "ms": 16.0])
        _ = agent.recognizedPan("up", [:])
        XCTAssertEqual(events, ["down", "move", "pan", "up", "release"])
        events = []
        _ = agent.recognizedPan("down", ["id": Int(node.id)])
        _ = agent.recognizedPan("cancel", [:])
        XCTAssertEqual(events, ["down", "up"])
    }

    /// A swipe row inside a panning surface (the smoke's precedence fixture),
    /// offered the touch as UIKit offers it: on the row both the swipe and
    /// the pan are, so only UIKit's arbitration could give the contact to
    /// either, and the contact is unsupported, saying why and sending
    /// nothing. The row's button keeps the touch from the swipe (rule 3) but
    /// not from the pan, so a contact there is the surface's pan, and its
    /// lift with no drag presses the button. Beside the row it is the pan's;
    /// a swipe whose `touch-action` leaves the horizontal to the platform
    /// never begins, and a button in a swipe row with no pan is a finger.
    func testAContactIsUnsupportedOnlyWhereTwoDragsAreOfferedIt() throws {
        let session = try booted(), p = session.presenter
        let make = { (id: UInt32, frame: CGRect, parent: UIView, handlers: Set<String>) -> NodeView in
            let n = NodeView(id: id, kind: "view", presenter: p)
            p.views[id] = n
            n.frame = frame
            n.handlers = handlers
            parent.addSubview(n)
            return n
        }
        let surface = make(UInt32.max - 2, CGRect(x: 0, y: 0, width: 400, height: 500), p.viewport, ["pan", "pointerdown"])
        let row = make(UInt32.max - 1, CGRect(x: 0, y: 0, width: 400, height: 100), surface, ["swiperight", "pointerdown"])
        row.style["touch_action"] = .string("pan-y")
        let button = make(UInt32.max, CGRect(x: 0, y: 0, width: 80, height: 40), row, ["press", "pointerdown"])
        let agent = Agent(session: session)
        var events: [UInt32] = [], pressed: [UInt32] = []
        p.onPointer = { id, _, _ in events.append(id) }
        p.onPress = { pressed.append($0) }
        let onRow = try XCTUnwrap(agent.recognizedPan("down", ["id": Int(row.id)]))
        XCTAssertEqual(onRow["delivery"] as? String, "unsupported", "the swipe and the pan are both offered it")
        XCTAssertTrue((onRow["reason"] as? String)?.contains("UIKit's arbitration") == true, "\(onRow)")
        XCTAssertEqual(events, [], "an unsupported contact sends nothing")
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(button.id)])?["contact"] as? Int, Int(surface.id), "the button keeps it from the swipe, not from the pan")
        let up = try XCTUnwrap(agent.recognizedPan("up", [:]))
        XCTAssertEqual(pressed, [button.id], "a pan that never began presses the button it landed on: \(up)")
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(surface.id), "y": 300.0])?["contact"] as? Int, Int(surface.id), "beside the row it is the pan's")
        _ = agent.recognizedPan("cancel", [:])
        row.style["touch_action"] = .string("auto")
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(row.id)])?["contact"] as? Int, Int(surface.id), "a swipe that never begins is no candidate")
        _ = agent.recognizedPan("cancel", [:])
        row.style["touch_action"] = .string("pan-y")
        // Walked from the view hit at the point: the row's point over its button is the button's.
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(row.id), "x": 40.0, "y": 20.0])?["contact"] as? Int, Int(surface.id), "the row at its button: the button keeps it from the swipe")
        _ = agent.recognizedPan("cancel", [:])
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(row.id), "x": 5.0, "y": 60.0])?["contact"] as? Int, Int(surface.id), "a swipe begins only past the screen edge")
        _ = agent.recognizedPan("cancel", [:])
        // An open row whose `touch-action` leaves the rightward drag to the platform still closes leftward.
        row.style["touch_action"] = .string("pan-left")
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(row.id), "x": 200.0, "y": 60.0])?["contact"] as? Int, Int(surface.id), "closed: the swipe never begins")
        _ = agent.recognizedPan("cancel", [:])
        row.translatePx = CGPoint(x: 60, y: 0)
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(row.id), "x": 200.0, "y": 60.0])?["delivery"] as? String, "unsupported", "open: it may close leftward")
        row.translatePx = .zero
        row.style["touch_action"] = .string("pan-y")
        surface.handlers = ["pointerdown"]
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(button.id)])?["delivery"] as? String, "recognized", "no pan: the button's finger, the swipe kept from it")
        _ = agent.recognizedPan("cancel", [:])
        // A height binding is offered no touch through a scroll view below it.
        let sheet = make(UInt32.max - 5, CGRect(x: 0, y: 520, width: 400, height: 300), p.viewport, [])
        sheet.heightRecognizer = UIPanGestureRecognizer()
        let scroller = UIScrollView(frame: sheet.bounds)
        sheet.addSubview(scroller)
        let item = make(UInt32.max - 4, CGRect(x: 0, y: 0, width: 400, height: 60), scroller, ["pointerdown"])
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(item.id)])?["delivery"] as? String, "recognized", "through a scroll view: a finger")
        _ = agent.recognizedPan("cancel", [:])
        let handle = make(UInt32.max - 3, CGRect(x: 0, y: 0, width: 400, height: 20), sheet, ["pointerdown"])
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(handle.id)])?["delivery"] as? String, "unsupported", "beside the scroll view the binding is offered it")
    }

    /// The agent's tap is what a finger there delivers (LLP 1012 §1): its
    /// `pointerdown` and `pointerup`, bubbled, then the press; a finger held
    /// off any `pan` node is the same with time between, and a lift beside
    /// its target presses nothing, as a touch up outside a control.
    func testATapAndAHeldFingerDeliverThePointerEventsBeforeThePress() throws {
        let session = try booted(), p = session.presenter
        let outer = NodeView(id: UInt32.max - 1, kind: "view", presenter: p), node = NodeView(id: UInt32.max, kind: "view", presenter: p)
        for v in [outer, node] { p.views[v.id] = v }
        outer.frame = CGRect(x: 0, y: 200, width: 300, height: 300)
        node.frame = CGRect(x: 0, y: 0, width: 100, height: 100)
        p.viewport.addSubview(outer)
        outer.addSubview(node)
        outer.handlers = ["pointerdown"]
        node.handlers = ["press", "pointerdown", "pointerup"]
        let agent = Agent(session: session)
        var events: [String] = []
        p.onPointer = { id, kind, sample in
            events.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id == outer.id ? "outer" : "node")")
            XCTAssertEqual(sample.type, "touch")
        }
        p.onPress = { _ in events.append("press") }
        XCTAssertNil(agent.tap(["id": Int(node.id)])["error"])
        XCTAssertEqual(events, ["down node", "down outer", "up node", "press"])
        events = []
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(node.id)])?["delivery"] as? String, "recognized")
        XCTAssertEqual(events, ["down node", "down outer"], "down as it lands")
        XCTAssertTrue(node.pressed, "held, the target shows its press")
        XCTAssertFalse(outer.pressed)
        _ = agent.recognizedPan("hold", ["ms": 800.0])
        let up = try XCTUnwrap(agent.recognizedPan("up", [:]))
        XCTAssertEqual(events, ["down node", "down outer", "up node", "press"], "\(up)")
        XCTAssertFalse(node.pressed, "released at the lift")
        events = []
        _ = agent.recognizedPan("down", ["id": Int(node.id)])
        _ = agent.recognizedPan("move", ["dx": 200.0, "ms": 16.0])
        _ = agent.recognizedPan("up", [:])
        XCTAssertEqual(events, ["down node", "down outer", "up node"], "lifted beside it: no press")
        XCTAssertFalse(node.pressed)
        _ = agent.recognizedPan("down", ["id": Int(node.id)])
        _ = agent.recognizedPan("cancel", [:])
        XCTAssertFalse(node.pressed, "released at a cancel")
        XCTAssertNil(agent.recognizedPan("down", ["id": Int(node.id), "pan": true]), "a drag's contact is a pan's or none")
    }

    /// What the pointer events do never sends the press elsewhere: a node
    /// whose `pointerup` puts another in its place presses neither. A node
    /// its own down disables still hears its release; a view the pool
    /// reuses for another node is not the held one. A press while a contact
    /// is held is refused; the samples carry the modifiers held.
    func testAHeldFingersPressIsJudgedAgainstTheUnpressedBox() throws {
        let session = try booted(), p = session.presenter
        let node = NodeView(id: UInt32.max, kind: "view", presenter: p)
        p.views[node.id] = node
        node.frame = CGRect(x: 0, y: 200, width: 100, height: 40)
        node.handlers = ["press"]
        node.applyStyle(["press_scale": 0.5])
        p.viewport.addSubview(node)
        var pressed = 0
        p.onPress = { _ in pressed += 1 }
        let agent = Agent(session: session)
        // Four points inside its left edge: outside the box drawn at half size, inside the box it presses as.
        XCTAssertEqual(agent.recognizedPan("down", ["id": Int(node.id), "x": 4.0, "y": 220.0])?["delivery"] as? String, "recognized")
        XCTAssertTrue(node.pressed)
        _ = agent.recognizedPan("move", ["dx": 0.0, "dy": 0.0])
        XCTAssertTrue(node.pressed, "a still finger stays pressed")
        _ = agent.recognizedPan("up", [:])
        XCTAssertEqual(pressed, 1, "the lift presses it")
    }

    func testAHeldFingerKeepsItsNodesAndATapItsTarget() throws {
        let session = try booted(), p = session.presenter
        let make = { (id: UInt32) -> NodeView in
            let n = NodeView(id: id, kind: "view", presenter: p)
            p.views[id] = n
            n.frame = CGRect(x: 0, y: 200, width: 100, height: 100)
            n.handlers = ["press", "pointerdown", "pointerup"]
            p.viewport.addSubview(n)
            return n
        }
        let node = make(UInt32.max)
        let agent = Agent(session: session)
        var events: [String] = [], held: [String] = []
        var onUp: (() -> Void)?
        p.onPointer = { id, kind, sample in
            events.append("\(kind == .down ? "down" : "up") \(id)")
            held.append(sample.held)
            if kind == .up { onUp?() }
        }
        p.onPress = { events.append("press \($0)") }
        let other = UInt32.max - 1
        onUp = {
            node.removeFromSuperview(); p.views[node.id] = nil
            _ = make(other)
        }
        XCTAssertNil(agent.tap(["id": Int(node.id), "modifiers": "Shift"])["error"])
        XCTAssertEqual(events, ["down \(node.id)", "up \(node.id)"], "the node under the finger changed: nothing pressed")
        XCTAssertEqual(held, ["Shift+", "Shift+"])

        onUp = nil
        events = []
        let target = try XCTUnwrap(p.views[other])
        p.onPointer = { id, kind, _ in
            events.append("\(kind == .down ? "down" : "up") \(id)")
            if kind == .down { target.props["disabled"] = "true" }
        }
        _ = agent.recognizedPan("down", ["id": Int(other)])
        XCTAssertEqual(agent.tap(["id": Int(other)])["error"] as? String, "a contact is already down; use `tap up` or `tap cancel` first")
        _ = agent.recognizedPan("up", [:])
        XCTAssertEqual(events, ["down \(other)", "up \(other)"], "its own down disabled it: it hears its release, and presses nothing")

        target.props["disabled"] = nil
        events = []
        p.onPointer = { id, kind, _ in events.append("\(kind == .down ? "down" : "up") \(id)") }
        _ = agent.recognizedPan("down", ["id": Int(other)])
        target.incarnation = NodePool.issue() // the pool gave the view to another node
        _ = agent.recognizedPan("up", [:])
        XCTAssertEqual(events, ["down \(other)"], "a reused view hears no release and takes no press")
    }

    /// The press is checked against what the finger reached where it landed,
    /// after every callback: a node its own `pointerdown` disabled, or whose
    /// view the pool gave another node, presses nothing; a held finger's
    /// lift presses what it went down on, never what took its place; and a
    /// handler further in that hands an ancestor's view to another node keeps
    /// that ancestor from hearing the event as the other one.
    func testAFingersPressIsTheNodeItLandedOnAsItIsAtTheLift() throws {
        let session = try booted(), p = session.presenter
        let add = { (id: UInt32, frame: CGRect, parent: UIView, handlers: Set<String>) -> NodeView in
            let n = NodeView(id: id, kind: "view", presenter: p)
            p.views[id] = n
            n.frame = frame
            n.handlers = handlers
            parent.addSubview(n)
            return n
        }
        let agent = Agent(session: session)
        var events: [String] = []
        p.onPress = { events.append("press \($0)") }
        let box = CGRect(x: 0, y: 200, width: 100, height: 100)

        // Disabled by its own pointerdown.
        let a = add(UInt32.max, box, p.viewport, ["press", "pointerdown"])
        p.onPointer = { id, _, _ in events.append("down \(id)"); a.props["disabled"] = "true" }
        _ = agent.tap(["id": Int(a.id)])
        XCTAssertEqual(events, ["down \(a.id)"], "disabled at the release: no press")
        a.removeFromSuperview(); p.views[a.id] = nil

        // Its view given to the next row (a virtualized row reused, as
        // `NodePool.rebind` does) by its own pointerup, in the plain tap.
        events = []
        let b = add(UInt32.max - 1, box, p.viewport, ["press", "pointerup"])
        let first = b.id, next = UInt32.max - 7
        p.onPointer = { id, _, _ in
            events.append("up \(id)")
            p.views[first] = nil
            b.rebind(next)
            p.views[next] = b
            b.handlers = ["press"]
        }
        _ = agent.tap(["id": Int(first)])
        XCTAssertEqual(events, ["up \(first)"], "the view is the next row's: neither row pressed")
        b.removeFromSuperview(); p.views[next] = nil

        // A container held over child A; A's pointerdown puts B in its place.
        events = []
        let container = add(UInt32.max - 2, box, p.viewport, [])
        let childA = add(UInt32.max - 3, CGRect(x: 0, y: 0, width: 100, height: 100), container, ["press", "pointerdown"])
        var childB: NodeView?
        p.onPointer = { id, kind, _ in
            events.append("down \(id)")
            childA.removeFromSuperview(); p.views[childA.id] = nil
            childB = add(UInt32.max - 4, CGRect(x: 0, y: 0, width: 100, height: 100), container, ["press"])
        }
        _ = agent.recognizedPan("down", ["id": Int(container.id), "x": 50.0, "y": 250.0])
        _ = agent.recognizedPan("up", [:])
        XCTAssertNotNil(childB)
        XCTAssertEqual(events, ["down \(childA.id)"], "the lift presses what it went down on, gone: never B")
        container.removeFromSuperview(); p.views[container.id] = nil

        // An inner handler hands the outer node's view to another node before its turn.
        events = []
        let outer = add(UInt32.max - 5, box, p.viewport, ["pointerdown"])
        let inner = add(UInt32.max - 6, CGRect(x: 0, y: 0, width: 100, height: 100), outer, ["pointerdown"])
        p.onPointer = { id, _, _ in events.append("down \(id)"); if id == inner.id { outer.incarnation = NodePool.issue() } }
        _ = agent.tap(["id": Int(inner.id)])
        XCTAssertEqual(events, ["down \(inner.id)"], "the outer view is another node's by its turn")
    }

    /// The motion engine at the wall, as a running animation leaves it, then a hold.
    private func holdAfter(_ session: ExactSession, _ op: [String: Any]) throws -> (wall: Double, reply: [String: Any], held: Bool) {
        session.apply(session.runtime.tick(now: session.now()))
        let wall = session.now()
        let reply = Agent(session: session).clock(op)
        let node = try XCTUnwrap(session.presenter.views.keys.min())
        let (hold, batch) = session.runtime.holdBegin(node, property: 0, now: session.now())
        session.apply(batch)
        return (wall, reply, hold != nil && batch.error == nil)
    }

    func testTakingTheClockOverStartsAtTheWallAndHoldsStillBegin() throws {
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing starts the clock at 0") }
        let session = try booted()
        XCTAssertNil(session.clock, "no clock op yet: the host reads the wall")
        let r = try holdAfter(session, ["take": true])
        XCTAssertNil(r.reply["error"], "\(r.reply)")
        let landed = try XCTUnwrap(r.reply["clock"] as? Double)
        XCTAssertGreaterThanOrEqual(landed, r.wall, "taken over at the wall, not the runner's 0")
        XCTAssertGreaterThanOrEqual(session.now(), landed)
        XCTAssertTrue(r.held, "a hold after the takeover is not refused")
        // Taken: a later take is a no-op seek, and a target behind the clock is refused.
        XCTAssertEqual(Agent(session: session).clock(["take": true])["clock"] as? Double, session.clock)
        XCTAssertNotNil(Agent(session: session).clock(["to": 100.0])["error"], "the clock cannot go backwards")
        // While the runner catches up its stops never set the host's clock behind the floor.
        let floor = try XCTUnwrap(session.clock) + 1000
        _ = Agent(session: session).advanceStepped(to: floor - 1000, deadline: Date().addingTimeInterval(1), floor: floor)
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(session.clock), floor)
    }

    func testARunnerAheadOfTheWallSetsTheFloor() throws {
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing starts the clock at 0") }
        let session = try booted()
        let agent = Agent(session: session)
        let wall = try XCTUnwrap(agent.clock(["take": true])["clock"] as? Double)
        let ahead = try XCTUnwrap(agent.clock(["to": wall + 60000])["clock"] as? Double)
        session.clock = nil // by hand: no known path leaves the runner ahead with the host's clock unset
        let taken = try XCTUnwrap(agent.clock(["take": true])["clock"] as? Double)
        XCTAssertGreaterThanOrEqual(taken, ahead, "the runner's clock, ahead of the wall, is the floor")
    }

    func testAFirstTargetBehindTheWallNeverSetsTheClockBack() throws {
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing starts the clock at 0") }
        let session = try booted()
        let r = try holdAfter(session, ["to": 100.0])
        XCTAssertGreaterThanOrEqual(session.clock ?? 0, r.wall, "the clock stays at the wall")
        XCTAssertTrue(r.held, "a hold after it is not refused (before §12: ClockWentBackwards)")
    }

    /// The app's own native motion on the wall clock (LLP 1012, amended
    /// 2026-10-16): `clock settle` waits for it once, up to 1.5 s, and says
    /// `settled: false`, reason `motion`, when it runs past that; a seek's
    /// steps wait for none, its last for up to the whole jump.
    func testTheClockWaitsForTheAppsOwnMotionOnceAndSaysWhenItRunsOn() throws {
        let session = try booted()
        let window = try XCTUnwrap(self.window)
        let agent = Agent(session: session)
        _ = agent.clock(["settle": true])
        let fade = UIView(frame: CGRect(x: 0, y: 0, width: 40, height: 40))
        window.addSubview(fade)
        defer { fade.layer.removeAllAnimations(); fade.removeFromSuperview() }
        UIView.animate(withDuration: 4) { fade.alpha = 0 }
        var t = Date()
        let settled = agent.clock(["settle": true])
        XCTAssertEqual(settled["settled"] as? Bool, false, "\(settled)")
        XCTAssertEqual(settled["reason"] as? String, "motion")
        XCTAssertLessThan(Date().timeIntervalSince(t), 3, "one 1.5 s wait, not one per round")
        let at = settled["clock"] as? Double ?? 0
        t = Date()
        _ = agent.clock(["to": at + 1000, "step": true])
        XCTAssertLessThan(Date().timeIntervalSince(t), 0.5, "a step waits for nothing")
        t = Date()
        let jumped = agent.clock(["to": at + 1100, "jump": 300.0])
        XCTAssertGreaterThan(Date().timeIntervalSince(t), 0.25, "the last step waits for up to the whole jump")
        XCTAssertLessThan(Date().timeIntervalSince(t), 1, "and no longer")
        XCTAssertEqual(jumped["settled"] as? Bool, false, "and says it still runs: \(jumped)")
        XCTAssertEqual(jumped["reason"] as? String, "motion")
    }
}
#endif
