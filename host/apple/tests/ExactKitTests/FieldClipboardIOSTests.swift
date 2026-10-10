#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// #125 on UIKit: a text view's or text field's paste:, copy: and cut: (the
/// edit menu, a hardware keyboard's ⌘V, ⌘C, ⌘X) are DOM's `paste`, `copy`
/// and `cut` at the nearest handler, before the editor's own, which an
/// action's `preventDefault()` cancels. A simulator runs it:
///   bun host/apple/build.mjs --test --ios
/// No test reads the pasteboard: a programmatic read asks "Allow Paste?",
/// which would hold the run. A paste carries the driver's text instead.
final class FieldClipboardIOSTests: XCTestCase {
    private var window: UIWindow!
    private var heard: [String] = []

    override func tearDown() { window = nil }

    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        // An action that prevents a paste of "BLOCK" or any cut stands in for one calling preventDefault().
        p.onClipboard = { [unowned self, unowned p] id, kind, text in
            heard.append("\(id):\(kind):\(text)")
            if text == "BLOCK" || (id == 1 && kind == 33) { p.defaultPrevented = true }
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["paste", "cut"]],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input", "copy", "cut"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 30.0],
        ]))
        return p
    }

    func testATextViewsCutIsItsEventThenItsOwn() throws {
        let p = presenter()
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(editor.becomeFirstResponder())
        editor.selectedRange = NSRange(location: 0, length: 2)
        editor.copy(nil)
        editor.cut(nil)
        XCTAssertEqual(editor.text, "llo", "an unprevented cut is the editor's")
        XCTAssertEqual(heard, ["2:32:", "2:33:"], "a cut's own copy is no second event")
    }

    func testATextFieldHearsAnAncestorAndAPreventedEditIsSkipped() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[3])
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field.becomeFirstResponder())
        field.selectedTextRange = field.textRange(from: field.beginningOfDocument, to: field.endOfDocument)
        field.cut(nil)
        XCTAssertEqual(field.text, "hello", "the ancestor's cut prevented it")
        XCTAssertTrue(node.fieldClipboard(#selector(NodeView.paste(_:)), text: "BLOCK"))
        XCTAssertFalse(node.fieldClipboard(#selector(NodeView.paste(_:)), text: "!"))
        XCTAssertEqual(heard, ["1:33:", "1:34:BLOCK", "1:34:!"])
    }

    /// A node no one hears declines the edit menu's copy, cut and paste,
    /// and one sent anyway is dropped, never forwarded to UIView (which
    /// implements none: a field's Cut, empty, raised doesNotRecognizeSelector).
    func testAnUnheardEditIsDeclinedAndNeverForwarded() throws {
        let p = presenter()
        let line = try XCTUnwrap(p.views[3]), editor = try XCTUnwrap(p.views[2]), root = try XCTUnwrap(p.views[1])
        for action in [#selector(NodeView.copy(_:)), #selector(NodeView.cut(_:)), #selector(NodeView.paste(_:))] {
            XCTAssertEqual(root.canPerformAction(action, withSender: nil), action != #selector(NodeView.copy(_:)), "the root hears paste and cut, not copy")
            // A field's or text area's edits are its editor's, not the node's.
            XCTAssertFalse(line.canPerformAction(action, withSender: nil))
            XCTAssertFalse(editor.canPerformAction(action, withSender: nil))
        }
        line.copy(nil)
        line.cut(nil)
        root.copy(nil)
        XCTAssertEqual(heard, [], "an unheard copy is no event and no crash")
    }
}
#endif
