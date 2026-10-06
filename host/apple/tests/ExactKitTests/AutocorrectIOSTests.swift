#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// HTML's `autocorrect="off"` on UIKit (#111): the text is kept as typed,
/// so a textarea and an input ask for no smart quotes or dashes as well as
/// no correction; without it each is the keyboard's default.
final class AutocorrectIOSTests: XCTestCase {
    func testAutocorrectOffAsksForNoSmartQuotesOrDashes() throws {
        let p = Presenter()
        for kind in ["textarea", "input"] {
            let node = NodeView(id: 1, kind: kind, presenter: p)
            node.frame = CGRect(x: 0, y: 0, width: 300, height: 88)
            node.applyProps(set: ["value": ""], clear: [])
            let read = { () -> [Int] in
                if let t = node.textArea { return [t.autocorrectionType.rawValue, t.smartQuotesType.rawValue, t.smartDashesType.rawValue] }
                guard let f = node.field else { return [] }
                return [f.autocorrectionType.rawValue, f.smartQuotesType.rawValue, f.smartDashesType.rawValue]
            }
            XCTAssertEqual(read(), [UITextAutocorrectionType.default.rawValue, UITextSmartQuotesType.default.rawValue, UITextSmartDashesType.default.rawValue], kind)
            node.applyProps(set: ["autocorrect": "off"], clear: [])
            XCTAssertEqual(read(), [UITextAutocorrectionType.no.rawValue, UITextSmartQuotesType.no.rawValue, UITextSmartDashesType.no.rawValue], kind)
            node.applyProps(set: [:], clear: ["autocorrect"])
            XCTAssertEqual(read(), [UITextAutocorrectionType.default.rawValue, UITextSmartQuotesType.default.rawValue, UITextSmartDashesType.default.rawValue], kind)
        }
    }
}
#endif
