import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

// The suffix includes this shared test in the simulator lane.
final class DocumentLanguageIOSTests: XCTestCase {
    func testArabicSwitchesNativeDirectionAndAccessibilityLanguage() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let presenter = Presenter()
        presenter.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "123"]],
            ["op": "create", "id": 2, "kind": "text", "style": ["direction": "ltr"]],
            ["op": "roots", "ids": [1, 2]],
            ["op": "language", "lang": "ar", "dir": "rtl"],
        ]))
        let text = try XCTUnwrap(presenter.views[1])
        let explicit = try XCTUnwrap(presenter.views[2])
        #if os(macOS)
        XCTAssertEqual(presenter.root.userInterfaceLayoutDirection, .rightToLeft)
        XCTAssertEqual(text.userInterfaceLayoutDirection, .rightToLeft)
        XCTAssertEqual(explicit.userInterfaceLayoutDirection, .leftToRight)
        XCTAssertEqual(text.accessibilityAttributeValue(NSAccessibility.Attribute(rawValue: "AXLanguage")) as? String, "ar")
        #else
        XCTAssertEqual(presenter.root.semanticContentAttribute, .forceRightToLeft)
        XCTAssertEqual(text.semanticContentAttribute, .forceRightToLeft)
        XCTAssertEqual(explicit.semanticContentAttribute, .forceLeftToRight)
        XCTAssertEqual(text.accessibilityLanguage, "ar")
        #endif
        presenter.apply(wireBatch([["op": "language", "lang": "en", "dir": "ltr"]]))
        #if os(macOS)
        XCTAssertEqual(text.userInterfaceLayoutDirection, .leftToRight)
        XCTAssertEqual(text.accessibilityAttributeValue(NSAccessibility.Attribute(rawValue: "AXLanguage")) as? String, "en")
        #else
        XCTAssertEqual(text.semanticContentAttribute, .forceLeftToRight)
        XCTAssertEqual(text.accessibilityLanguage, "en")
        #endif
    }
}
