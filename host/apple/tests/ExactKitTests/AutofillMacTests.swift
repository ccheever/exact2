#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// HTML's `autocomplete` as AppKit's content type (LLP 1102 §3.6): the last
/// field token names it, `off` and an unknown token leave none, and a
/// textarea takes it too.
final class AutofillMacTests: XCTestCase {
    func testAutocompleteSetsTheFieldsContentType() throws {
        _ = NSApplication.shared
        let p = Presenter()
        for kind in ["input", "textarea"] {
            let node = NodeView(id: 1, kind: kind, presenter: p)
            node.frame = NSRect(x: 0, y: 0, width: 300, height: 44)
            let content = { node.field?.contentType ?? node.textArea?.contentType }
            node.applyProps(set: ["value": ""], clear: [])
            XCTAssertNil(content(), kind)
            for (value, expected) in [
                ("username", NSTextContentType.username), ("section-login current-password", .password),
                ("shipping postal-code", .postalCode), ("ONE-TIME-CODE", .oneTimeCode),
                ("work email webauthn", .emailAddress), ("new-password", .newPassword),
                ("billing\r\nwork tel", .telephoneNumber),
            ] {
                node.applyProps(set: ["autocomplete": value], clear: [])
                XCTAssertEqual(content(), expected, "\(kind) \(value)")
            }
            for value in ["off", "on", "impp", "", "garbage username", "nic\u{212A}name"] {
                node.applyProps(set: ["autocomplete": value], clear: [])
                XCTAssertNil(content(), "\(kind) \(value)")
            }
        }
    }
}
#endif
