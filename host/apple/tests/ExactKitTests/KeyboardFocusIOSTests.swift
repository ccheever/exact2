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

    func testBlockLinksOpenWithoutAHandlerAndRespectAuthoredPressAndDisabled() {
        final class Delegate: ExactSessionDelegate {
            var urls: [String] = []
            func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
                if name == "openURL", let url = args.first as? String { urls.append(url) }
            }
        }
        let delegate = Delegate()
        let session = ExactApp.shared.makeSession(delegate: delegate, label: "block-link")
        defer { session.destroy() }
        let p = session.presenter
        let link = NodeView(id: 9001, kind: "button", presenter: p)
        link.frame = CGRect(x: 0, y: 0, width: 200, height: 40)
        p.root.addSubview(link); p.views[link.id] = link
        link.applyProps(set: ["href": "https://example.test/article"], clear: [])
        XCTAssertTrue(link.activate(at: link.convert(CGPoint(x: 10, y: 10), to: nil)) === link)
        XCTAssertEqual(delegate.urls, ["https://example.test/article"])
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        link.handlers = ["press"]
        p.press(link.id)
        XCTAssertEqual(presses, [link.id])
        XCTAssertEqual(delegate.urls.count, 1)
        link.handlers = []
        link.applyProps(set: ["disabled": "true"], clear: [])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.count, 1)
        link.applyProps(set: ["inert": "true"], clear: ["disabled"])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.count, 1)
        link.applyProps(set: ["href": "//example.test/relative"], clear: ["inert"])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.last, "//example.test/relative")
        XCTAssertTrue(link.accessibilityActivate())
        XCTAssertEqual(delegate.urls.count, 3)
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

    /// HTML's `tabindex` (LLP 1088 D7.3): an explicit value makes a plain box
    /// focusable and, ≥ 0, a Tab stop, positive first; a negative one takes
    /// the focus but not Tab; absent is never `0`; a disabled button stays
    /// out, and a disabled box is a stop (Chrome's `<div disabled>`).
    func testTabindexMakesABoxFocusableAndOrdersTab() {
        let (p, first, _, field, last) = fixture()
        defer { withExtendedLifetime(p) {} }
        func box(_ id: UInt32, _ props: [String: String], kind: String = "view") -> NodeView {
            let n = NodeView(id: id, kind: kind, presenter: p)
            n.applyProps(set: props, clear: [])
            n.frame = CGRect(x: 220, y: CGFloat(id) * 20, width: 100, height: 10)
            p.root.addSubview(n); p.views[id] = n
            return n
        }
        let stop = box(10, ["tabIndex": "0"]), plain = box(11, [:]), early = box(12, ["tabIndex": "1"])
        let skipped = box(13, ["tabIndex": "-1"]), disabled = box(14, ["tabIndex": "0", "disabled": "true"])
        let off = box(15, ["tabIndex": "0", "disabled": "true"], kind: "button")
        XCTAssertFalse(plain.canBecomeFirstResponder, "absent is not tabindex=0")
        XCTAssertTrue(disabled.canBecomeFirstResponder, "disabled means nothing on a box")
        XCTAssertFalse(off.canBecomeFirstResponder, "a disabled button is out")
        XCTAssertTrue(skipped.becomeFirstResponder(), "-1 takes the focus by tap or script")
        p.moveFocus(backward: false)
        XCTAssertTrue(early.isFirstResponder, "from a node out of the order, Tab starts at the first: a positive tabindex")
        p.moveFocus(backward: false)
        XCTAssertTrue(first.isFirstResponder, "then tree order")
        p.moveFocus(backward: false)
        XCTAssertTrue(field.field?.isFirstResponder == true)
        p.moveFocus(backward: false); p.moveFocus(backward: false)
        XCTAssertTrue(stop.isFirstResponder, "tabindex=0 with no handler is a stop, in tree order")
        p.moveFocus(backward: false)
        XCTAssertTrue(disabled.isFirstResponder, "then the disabled box")
        p.moveFocus(backward: false)
        XCTAssertTrue(early.isFirstResponder, "the order wraps, skipping -1 and the disabled button")
        p.moveFocus(backward: true)
        XCTAssertTrue(disabled.isFirstResponder)
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
        XCTAssertTrue(p.viewport.focusItems(in: all).contains { $0 === editor }, "a field in the window is what the search is given")
        XCTAssertFalse(p.viewport.focusItems(in: all).contains { $0 === p.root || $0 === button }, "and none of exact2's own views")
        XCTAssertTrue(p.viewport.focusItems(in: CGRect(x: 0, y: 0, width: 400, height: 45)).isEmpty, "only where it is")
        input.isHidden = true
        XCTAssertTrue(p.viewport.focusItems(in: all).isEmpty, "nor a hidden one")
        input.isHidden = false
        input.removeFromSuperview()
        XCTAssertTrue(p.viewport.focusItems(in: all).isEmpty, "and it closes when the field leaves")
    }
}
#endif
