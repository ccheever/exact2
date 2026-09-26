#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A hardware keyboard reaches and operates controls, as on macOS: Tab and
/// Shift-Tab walk the sequential order — inputs and pressables in tree
/// order, never plain text — through a field and around; a pressable Tab
/// reached shows its ring, one a touch focused does not. UIKit, so a
/// simulator runs it (the command in `FieldCompositionIOSTests`).
final class KeyboardFocusIOSTests: XCTestCase {
    private var window: UIWindow!
    private func fixture() -> (Presenter, NodeView, NodeView, NodeView, NodeView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let first = NodeView(id: 1, kind: "button", presenter: p)
        let text = NodeView(id: 2, kind: "text", presenter: p)
        let field = NodeView(id: 3, kind: "input", presenter: p)
        let last = NodeView(id: 4, kind: "button", presenter: p)
        text.applyProps(set: ["text": "Northbound"], clear: [])
        first.handlers = ["press"]; field.handlers = ["change"]; last.handlers = ["press"]
        for (i, node) in [first, text, field, last].enumerated() {
            node.frame = CGRect(x: 0, y: CGFloat(i) * 50, width: 200, height: 40)
            p.root.addSubview(node); p.views[node.id] = node
        }
        window.makeKeyAndVisible()
        return (p, first, text, field, last)
    }
    /// What UIKit sends for Tab (or Shift-Tab) from this node's chain.
    private func tab(_ node: UIResponder, shift: Bool = false, file: StaticString = #filePath, line: UInt = #line) {
        var responder: UIResponder? = node, command: UIKeyCommand?
        while let r = responder, command == nil {
            command = r.keyCommands?.first { $0.input == "\t" && $0.modifierFlags == (shift ? .shift : []) }
            responder = r.next
        }
        guard let command, let action = command.action else { return XCTFail("no Tab command on the chain", file: file, line: line) }
        XCTAssertTrue(command.wantsPriorityOverSystemBehavior, "a text input's own Tab must not win", file: file, line: line)
        let target = node.target(forAction: action, withSender: command) as? NSObject
        XCTAssertNotNil(target, file: file, line: line)
        _ = target?.perform(action, with: command)
    }

    func testTabWalksControlsInTreeOrderAndSkipsText() throws {
        let (p, first, text, field, last) = fixture()
        defer { withExtendedLifetime(p) {} }
        let editor = try XCTUnwrap(field.field)
        XCTAssertTrue(first.becomeFirstResponder(), "a pressable takes the focus")
        tab(first)
        XCTAssertTrue(editor.isFirstResponder, "Tab passes over the paragraph to the field")
        XCTAssertFalse(text.isFirstResponder)
        tab(editor)
        XCTAssertTrue(last.isFirstResponder, "Tab leaves the field for the next control")
        tab(last)
        XCTAssertTrue(first.isFirstResponder, "the order wraps")
        tab(first, shift: true)
        XCTAssertTrue(last.isFirstResponder, "Shift-Tab walks back")
    }

    func testFirstTabTakesTheFirstControlAndShowsItsRing() {
        let (p, first, _, _, last) = fixture()
        p.moveFocus(backward: false)
        XCTAssertTrue(first.isFirstResponder)
        XCTAssertNotNil(first.focusRing, "a control Tab reached shows its ring")
        p.moveFocus(backward: true)
        XCTAssertTrue(last.isFirstResponder)
        XCTAssertNil(first.focusRing, "the ring leaves with the focus")
        XCTAssertTrue(first.becomeFirstResponder())
        XCTAssertNil(first.focusRing, "a touch's focus shows no ring")
    }

    /// UIKit's focus search (`FocusSearch`) finds nothing in a tree of
    /// exact2's own views, and finds an input's field as UIKit always did.
    func testFocusSearchSeesOnlyWhatUIKitCanFocus() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let button = NodeView(id: 1, kind: "button", presenter: p)
        button.frame = CGRect(x: 0, y: 0, width: 200, height: 40)
        p.root.addSubview(button); p.views[button.id] = button
        window.makeKeyAndVisible()
        let all = window.bounds
        XCTAssertTrue(p.viewport.focusItems(in: all).isEmpty, "a tree of exact2's views offers UIKit nothing to search")
        let input = NodeView(id: 2, kind: "input", presenter: p)
        input.frame = CGRect(x: 0, y: 50, width: 200, height: 40)
        p.root.addSubview(input); p.views[input.id] = input
        let editor = try XCTUnwrap(input.field)
        XCTAssertTrue(p.viewport.focusItems(in: all).contains { $0 === p.root }, "a field in the window opens the search")
        XCTAssertTrue(input.focusItems(in: all).contains { $0 === editor })
        input.removeFromSuperview()
        XCTAssertTrue(p.viewport.focusItems(in: all).isEmpty, "and it closes when the field leaves")
    }
}
#endif
