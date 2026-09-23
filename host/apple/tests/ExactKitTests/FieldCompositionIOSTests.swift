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
    func testCommittedCompositionIsWhatTheAppHears() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
        node.handlers = ["change"]
        window.addSubview(node); p.views[node.id] = node
        window.makeKeyAndVisible()
        // A controlled input, as Caltrain's search is: the app echoes what it hears.
        var heard: [String] = []
        p.onChange = { [unowned node] _, value in
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
