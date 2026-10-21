#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// HTML's `autocomplete` as UIKit's text content type (LLP 1102 §3.6): the
/// last field token names it over the one `type` implies, `off` clears even
/// that, and `on` or a token UIKit has no type for leave `type`'s.
final class AutofillIOSTests: XCTestCase {
    func testAutocompleteOverridesTheTypesContentType() throws {
        let p = Presenter()
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
        node.applyProps(set: ["value": "", "type": "email"], clear: [])
        let field = try XCTUnwrap(node.field)
        XCTAssertEqual(field.textContentType, .emailAddress)
        for (value, expected) in [
            ("username", UITextContentType.username), ("section-a billing given-name", .givenName),
            ("tel", .telephoneNumber), ("ONE-TIME-CODE", .oneTimeCode), ("cc-exp", .creditCardExpiration),
            ("work email webauthn", .emailAddress), ("address-line1", .streetAddressLine1),
            ("shipping\r\nmobile tel", .telephoneNumber), ("section-x\u{c}shipping postal-code", .postalCode),
        ] {
            node.applyProps(set: ["autocomplete": value], clear: [])
            XCTAssertEqual(field.textContentType, expected, value)
        }
        node.applyProps(set: ["autocomplete": "off"], clear: [])
        XCTAssertNil(field.textContentType)
        // HTML's grammar, ASCII only: `off` must stand alone, a contact kind
        // comes before a contact field, and a Kelvin sign is no `k`.
        for value in ["on", "impp", "off webauthn", "home name", "garbage username", "nic\u{212A}name"] {
            node.applyProps(set: ["autocomplete": value], clear: [])
            XCTAssertEqual(field.textContentType, .emailAddress, value)
        }
        node.applyProps(set: ["type": "password", "autocomplete": "new-password"], clear: [])
        XCTAssertEqual(field.textContentType, .newPassword)
        node.applyProps(set: [:], clear: ["autocomplete"])
        XCTAssertEqual(field.textContentType, .password)
        XCTAssertTrue(field.isSecureTextEntry)

        let area = NodeView(id: 2, kind: "textarea", presenter: p)
        area.frame = CGRect(x: 0, y: 0, width: 300, height: 88)
        area.applyProps(set: ["value": "", "autocomplete": "street-address"], clear: [])
        XCTAssertEqual(try XCTUnwrap(area.textArea).textContentType, .fullStreetAddress)
        area.applyProps(set: ["autocomplete": "off"], clear: [])
        XCTAssertNil(area.textArea?.textContentType)
    }
}
#endif
