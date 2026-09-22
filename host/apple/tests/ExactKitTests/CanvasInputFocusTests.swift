// A real UIKit responder handoff, because the iOS agent activates a node
// directly and never exercises touch-release focus. UIKit cannot construct a
// UITouch, so the test seeds CanvasInput's actual raw-contact reducer and then
// lets UIWindow perform each responder transition.
//
// @ref LLP 1008 §9; LLP 1012 §1; LLP 1041.002 S1
#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

@MainActor
final class CanvasInputFocusTests: XCTestCase {
    private var window: UIWindow!
    private var presenter: Presenter!
    private var canvas: NodeView!
    private var input: CanvasInput!

    override func setUp() {
        super.setUp()
        presenter = Presenter()
        let controller = UIViewController()
        controller.view.addSubview(presenter.viewport)
        presenter.viewport.frame = controller.view.bounds
        presenter.viewport.autoresizingMask = [.flexibleWidth, .flexibleHeight]

        canvas = NodeView(id: 1, kind: "canvas", presenter: presenter)
        canvas.handlers = ["focus"]
        canvas.frame = CGRect(x: 0, y: 0, width: 300, height: 300)
        presenter.root.addSubview(canvas)
        input = CanvasInput(view: canvas)
        canvas.canvasInput = input

        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.rootViewController = controller
        window.makeKeyAndVisible()
    }

    override func tearDown() {
        window.isHidden = true
        input = nil
        canvas = nil
        presenter = nil
        window = nil
        super.tearDown()
    }

    private func button(_ id: UInt32, in parent: UIView) -> NodeView {
        let button = NodeView(id: id, kind: "button", presenter: presenter)
        button.frame = CGRect(x: 0, y: 0, width: 80, height: 40)
        parent.addSubview(button)
        return button
    }

    private func seedContact() -> NSObject {
        let contact = NSObject()
        input.beginContact(ObjectIdentifier(contact))
        XCTAssertEqual(input.contactCount, 1)
        return contact
    }

    func testResponderMovesInsideCanvasRetainRawContactUntilFocusLeaves() {
        let jump = button(2, in: canvas.overlay!)
        let light = button(3, in: canvas.overlay!)
        let outside = button(4, in: presenter.root)

        XCTAssertTrue(canvas.becomeFirstResponder())
        let contact = seedContact()
        let token = ObjectIdentifier(contact)
        let movementID = input.contact(token)
        XCTAssertTrue(jump.becomeFirstResponder())
        XCTAssertEqual(input.contact(token), movementID)
        XCTAssertTrue(light.becomeFirstResponder())
        XCTAssertEqual(input.contact(token), movementID)

        XCTAssertTrue(outside.becomeFirstResponder())
        XCTAssertNil(input.contact(token))
        withExtendedLifetime(contact) {}
    }

    func testDirectResignationAndEndEditingClearRawContacts() {
        XCTAssertTrue(canvas.becomeFirstResponder())
        let direct = seedContact()
        XCTAssertTrue(canvas.resignFirstResponder())
        XCTAssertEqual(input.contactCount, 0)

        XCTAssertTrue(canvas.becomeFirstResponder())
        let ended = seedContact()
        XCTAssertTrue(window.endEditing(true))
        XCTAssertEqual(input.contactCount, 0)
        withExtendedLifetime((direct, ended)) {}
    }

    func testApplicationResignationClearsRawContacts() {
        let contact = seedContact()
        NotificationCenter.default.post(name: UIApplication.willResignActiveNotification, object: nil)
        XCTAssertEqual(input.contactCount, 0)
        withExtendedLifetime(contact) {}
    }
}
#endif
