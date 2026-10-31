#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1005 §3 on UIKit: a `pointerdown` action that calls `preventDefault()`
/// keeps the focus where it is: the view the touch went down on takes no
/// focus from it (a field's own tap, a node's), and the press still fires, as
/// Chrome does for a prevented pointerdown by touch
/// (scripts/fixtures/prevent-focus.contract; host/web/tests/prevent-focus.test.mjs).
/// The touch goes to the node's pointer recognizer and to the node as UIKit
/// delivers it; the focus a field's own tap would ask for is asked directly.
final class PreventFocusIOSTests: XCTestCase {
    private final class Touch: UITouch {
        let target: UIView
        init(_ target: UIView) { self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(CGPoint(x: 8, y: target.bounds.midY), to: view) }
    }
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func make(_ scene: UIWindowScene) throws -> ExactSession {
        let session = ExactApp.shared.makeSession(label: "prevent-focus")
        self.session = session
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 600)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        session.presenter.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(session.presenter.viewport)
        XCTAssertNil(session.boot(plan: try fixturePlan("PREVENT_FOCUS"), size: window.bounds.size).error)
        settle()
        return session
    }
    private func settle() { for _ in 0..<5 { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) } }
    private func node(_ session: ExactSession, _ id: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == id }, id)
    }
    private func heard(_ session: ExactSession) -> [String] {
        let text = session.agent(#"{"op":"state"}"#)
        let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
        return ((json?["slots"] as? [String: Any])?["heard"] as? String ?? "").split(separator: " ").map(String.init)
    }
    private func recognizer(_ node: NodeView) throws -> PointerRecognizer {
        try XCTUnwrap(node.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first, "\(node.props["testId"] ?? "")'s pointer recognizer")
    }

    func testAPreventedPointerdownKeepsTheFocusAndThePress() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "a field takes the focus only in an app's window scene") }
        let session = try make(scene)
        let a = try XCTUnwrap(try node(session, "a").field)
        XCTAssertTrue(a.becomeFirstResponder())
        settle()
        // A field (b), a text area (t), a focusable box: none takes the focus from the touch.
        for id in ["b", "t", "box"] {
            let target = try node(session, id)
            let view: UIView = target.field ?? target.textArea ?? target
            let touch = Touch(view), event = UIEvent(), observer = try recognizer(target)
            observer.touchesBegan([touch], with: event)
            if id == "box" { target.touchesBegan([touch], with: event) }
            XCTAssertFalse(view.becomeFirstResponder(), "\(id): its own tap's focus is kept out")
            observer.touchesEnded([touch], with: event)
            if id == "box" { target.touchesEnded([touch], with: event) }
            settle()
            XCTAssertTrue(a.isFirstResponder, "\(id): the focus stays in a")
            XCTAssertEqual(session.presenter.preventedTouches.count, 0, "\(id): over a turn after the lift")
        }
        XCTAssertTrue(heard(session).contains("press-box"), "the box's press still fires: \(heard(session))")
        for id in ["b", "t", "box"] { XCTAssertFalse(heard(session).contains("focus-\(id)"), "\(id) never took the focus: \(heard(session))") }
        // After the touch, the same field takes the focus as ever (an action's focus(), a later tap).
        // A prevented tap on a box that takes no focus ends no editing: a keeps it (the press's focus decision),
        // and what was typed in it is committed, as every press commits it on the Mac.
        // Text an input method is still composing is not committed by the press; once composed, the next press commits it once.
        a.insertText("x")
        a.setMarkedText("k", selectedRange: NSRange(location: 1, length: 0))
        settle()
        let plain = try node(session, "plain"), event = UIEvent()
        func tapPlain() throws {
            let touch = Touch(plain)
            try recognizer(plain).touchesBegan([touch], with: event)
            plain.touchesBegan([touch], with: event)
            try recognizer(plain).touchesEnded([touch], with: event)
            plain.touchesEnded([touch], with: event)
            settle()
        }
        try tapPlain()
        XCTAssertTrue(a.isFirstResponder, "a prevented tap elsewhere ends no editing")
        XCTAssertTrue(heard(session).contains("press-plain"), "\(heard(session))")
        XCTAssertFalse(heard(session).contains("change-a"), "nothing committed mid-composition: \(heard(session))")
        a.unmarkText()
        settle()
        try tapPlain()
        XCTAssertEqual(heard(session).filter { $0 == "change-a" }.count, 1, "the typed text committed, once: \(heard(session))")
        let b = try XCTUnwrap(try node(session, "b").field)
        XCTAssertTrue(b.becomeFirstResponder())
        settle()
        XCTAssertTrue(heard(session).contains("focus-b"), "\(heard(session))")
    }

    /// Two fingers whose downs both prevent: neither view takes the focus while
    /// its touch is down; and a prevented touch on selectable text starts no
    /// selection.
    func testEachPreventedTouchKeepsItsViewOutAndStartsNoSelection() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "a field takes the focus only in an app's window scene") }
        let session = try make(scene)
        let a = try XCTUnwrap(try node(session, "a").field)
        XCTAssertTrue(a.becomeFirstResponder())
        let b = try node(session, "b"), t = try node(session, "t")
        let first = Touch(try XCTUnwrap(b.field)), second = Touch(try XCTUnwrap(t.textArea)), event = UIEvent()
        try recognizer(b).touchesBegan([first], with: event)
        try recognizer(t).touchesBegan([second], with: event)
        XCTAssertFalse(try XCTUnwrap(b.field).becomeFirstResponder(), "the first finger's field")
        XCTAssertFalse(try XCTUnwrap(t.textArea).becomeFirstResponder(), "the second finger's text area")
        try recognizer(b).touchesEnded([first], with: event)
        try recognizer(t).touchesEnded([second], with: event)
        settle()
        XCTAssertTrue(a.isFirstResponder)
        let para = try node(session, "para")
        let held = try XCTUnwrap(session.presenter.textSelection.install(para))
        let interaction = UITextInteraction(for: .nonEditable)
        XCTAssertTrue(held.interactionShouldBegin(interaction, at: CGPoint(x: 4, y: 4)), "selectable text, before any touch")
        let touch = Touch(para)
        try recognizer(para).touchesBegan([touch], with: event)
        XCTAssertFalse(held.interactionShouldBegin(interaction, at: CGPoint(x: 4, y: 4)), "no selection under a prevented touch")
        try recognizer(para).touchesEnded([touch], with: event)
        settle()
        XCTAssertTrue(held.interactionShouldBegin(interaction, at: CGPoint(x: 4, y: 4)), "selection again after the lift")
    }

    /// An action's own `focus()` moves the focus even to the view a prevented
    /// touch is down on, as it does on the web and the Mac.
    func testAnActionsFocusStillMovesItDuringAPreventedTouch() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "a field takes the focus only in an app's window scene") }
        let session = try make(scene)
        XCTAssertTrue(try XCTUnwrap(try node(session, "a").field).becomeFirstResponder())
        let b = try node(session, "b"), field = try XCTUnwrap(b.field), touch = Touch(field), event = UIEvent()
        try recognizer(b).touchesBegan([touch], with: event)
        XCTAssertFalse(field.becomeFirstResponder(), "its own tap's focus is kept out")
        session.presenter.focusNode(b)
        settle()
        XCTAssertTrue(field.isFirstResponder, "the action's focus() is delivered")
        try recognizer(b).touchesEnded([touch], with: event)
        settle()
        XCTAssertTrue(heard(session).contains("focus-b"), "\(heard(session))")
    }

    /// A touch whose `pointerdown` was not prevented leaves the focus to UIKit.
    func testAnUnpreventedTouchLeavesTheFocusToUIKit() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "a field takes the focus only in an app's window scene") }
        let session = try make(scene)
        XCTAssertTrue(try XCTUnwrap(try node(session, "a").field).becomeFirstResponder())
        let c = try XCTUnwrap(try node(session, "c").field)
        XCTAssertEqual(session.presenter.preventedTouches.count, 0)
        XCTAssertTrue(c.becomeFirstResponder())
        settle()
        XCTAssertTrue(heard(session).contains("focus-c"), "\(heard(session))")
    }
}
#endif
