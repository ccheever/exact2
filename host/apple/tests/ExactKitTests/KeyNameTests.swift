import XCTest
#if canImport(AppKit)
import AppKit
#endif
@testable import ExactKit

/// A character key's `KeyboardEvent.key` as Chrome reports it on a Mac:
/// Option's character with Option down (Option+A is "å"), Control's
/// unmodified key, a dead key's plain one (Grok's batch 2 review).
final class KeyNameTests: XCTestCase {
    func testOptionTypesItsCharacterAndControlKeepsThePlainKey() {
        XCTAssertEqual(KeyCodes.typed(option: true, characters: "å", ignoringModifiers: "a"), "å")
        XCTAssertEqual(KeyCodes.typed(option: true, characters: "Å", ignoringModifiers: "A"), "Å")
        XCTAssertEqual(KeyCodes.typed(option: false, characters: "\u{1}", ignoringModifiers: "a"), "a", "Control+A")
        XCTAssertEqual(KeyCodes.typed(option: true, characters: "\u{1}", ignoringModifiers: "a"), "a", "Control+Option+A")
        XCTAssertEqual(KeyCodes.typed(option: true, characters: "", ignoringModifiers: "e"), "e", "a dead key")
        XCTAssertEqual(KeyCodes.typed(option: false, characters: "S", ignoringModifiers: "S"), "S")
    }

    #if canImport(AppKit)
    func testAnAppKitOptionKeyIsItsCharacter() throws {
        func key(_ chars: String, _ plain: String, _ flags: NSEvent.ModifierFlags) throws -> String {
            NodeView.keyName(try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0,
                windowNumber: 0, context: nil, characters: chars, charactersIgnoringModifiers: plain, isARepeat: false, keyCode: 0)))
        }
        XCTAssertEqual(try key("å", "a", .option), "å")
        XCTAssertEqual(try key("\u{1}", "a", .control), "a")
        XCTAssertEqual(try key("a", "a", .command), "a")
    }
    #endif
}
