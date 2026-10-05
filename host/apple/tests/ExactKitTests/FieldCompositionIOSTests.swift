#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A single-line field composing through an input method — pinyin, then
/// 你好 chosen: the app hears, and the field keeps, the committed text,
/// never the composing value the app echoed back while it was marked (the
/// textarea's order, `textViewDidChange`). UIKit, so a simulator runs it:
///   EXACT_TESTS=1 EXACT_LIB_DIR=<target>/aarch64-apple-ios-sim/release EXACT_LIB=caltrain_apple \
///   xcodebuild test -scheme Exact -destination 'platform=iOS Simulator,name=<iPhone>' \
///     -only-testing:ExactKitTests/FieldCompositionIOSTests
final class FieldCompositionIOSTests: XCTestCase {
    func testInputsApplyPhysicalAndLogicalAlignmentAfterRestyling() throws {
        let session = ExactApp.shared.makeSession(label: "input-alignment")
        defer { session.destroy() }
        let p = session.presenter
        for kind in ["input", "textarea"] {
            let node = NodeView(id: 1, kind: kind, presenter: p)
            node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
            node.applyProps(set: ["value": "hello"], clear: [])
            for (alignment, direction, expected) in [
                ("center", "ltr", NSTextAlignment.center), ("right", "ltr", .right),
                ("start", "rtl", .right), ("end", "rtl", .left), ("left", "rtl", .left),
                ("start", "ltr", .left),
            ] {
                node.applyStyle(["text_align": .string(alignment), "direction": .string(direction), "line_height": "24px"])
                XCTAssertEqual(node.field?.textAlignment ?? node.textArea?.textAlignment, expected, "\(kind) \(alignment) \(direction)")
                if let editor = node.textArea {
                    let paragraph = try XCTUnwrap(editor.textStorage.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)
                    XCTAssertEqual(paragraph.alignment, expected)
                    XCTAssertEqual(paragraph.minimumLineHeight, 24)
                    XCTAssertEqual((editor.typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.alignment, expected)
                }
            }
            node.applyStyle([:])
            XCTAssertEqual(node.field?.textAlignment ?? node.textArea?.textAlignment, .left)
        }
    }

    func testCommittedCompositionIsWhatTheAppHears() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
        node.handlers = ["input"]
        window.addSubview(node); p.views[node.id] = node
        window.makeKeyAndVisible()
        // A controlled input, as Caltrain's search is: the app echoes what it
        // hears per keystroke (`input`; `change` is the commit, LLP 1069.001).
        var heard: [String] = []
        p.onInput = { [unowned node] _, value in
            heard.append(value)
            node.applyProps(set: ["value": value], clear: [])
        }
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field.becomeFirstResponder())
        field.setMarkedText("nihao", selectedRange: NSRange(location: 5, length: 0))
        node.fieldChanged()
        XCTAssertNotNil(field.markedTextRange)
        XCTAssertEqual(node.pendingValue, "nihao", "the echo waits for the composition")
        field.setMarkedText("你好", selectedRange: NSRange(location: 2, length: 0))
        field.unmarkText()
        XCTAssertNil(field.markedTextRange)
        node.fieldChanged()
        XCTAssertEqual(heard.last, "你好", "the app hears the committed text")
        XCTAssertEqual(field.text, "你好")
        XCTAssertNil(node.pendingValue)
    }
}
#endif
