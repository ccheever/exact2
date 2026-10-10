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

    /// One vocabulary with the web driver: a letter and its code, a named
    /// key's function character (so a field does not insert the name).
    func testDriverKeysMatchTheWebVocabulary() {
        XCTAssertEqual(KeyCodes.device("p")?.code, "KeyP")
        XCTAssertEqual(KeyCodes.device("p")?.key, "p")
        XCTAssertEqual(KeyCodes.device("P")?.code, "KeyP")
        XCTAssertEqual(KeyCodes.device("KeyP")?.code, "KeyP")
        XCTAssertEqual(KeyCodes.device("7")?.code, "Digit7")
        XCTAssertEqual(KeyCodes.device("7")?.key, "7")
        XCTAssertEqual(KeyCodes.device("End")?.code, "End")
        XCTAssertEqual(KeyCodes.device("Home")?.key, "Home")
        XCTAssertEqual(KeyCodes.device("Delete")?.code, "Delete")
        XCTAssertEqual(KeyCodes.device("PageUp")?.code, "PageUp")
        XCTAssertEqual(KeyCodes.device("-")?.code, "Minus")
        XCTAssertEqual(KeyCodes.device(" ")?.code, "Space")
        XCTAssertEqual(KeyCodes.device(" ")?.key, " ")
        XCTAssertEqual(KeyCodes.device("+")?.code, "Equal")
        XCTAssertEqual(KeyCodes.device("!")?.code, "Digit1")
        XCTAssertEqual(KeyCodes.device("?")?.code, "Slash")
        XCTAssertEqual(KeyCodes.eventText(code: "Digit1", raw: "!", lone: false), "!")
        XCTAssertNil(KeyCodes.device("Nope"))
        XCTAssertEqual(KeyCodes.eventText(code: "End", raw: "End", lone: false), "\u{F72B}")
        XCTAssertEqual(KeyCodes.eventText(code: "Home", raw: "Home", lone: false), "\u{F729}")
        XCTAssertEqual(KeyCodes.eventText(code: "Delete", raw: "Delete", lone: false), "\u{F728}")
        XCTAssertEqual(KeyCodes.eventText(code: "PageUp", raw: "PageUp", lone: false), "\u{F72C}")
        XCTAssertEqual(KeyCodes.eventText(code: "PageDown", raw: "PageDown", lone: false), "\u{F72D}")
        XCTAssertEqual(KeyCodes.eventText(code: "F1", raw: "F1", lone: false), "\u{F704}")
        XCTAssertEqual(KeyCodes.eventText(code: "KeyP", raw: "P", lone: false), "P")
        XCTAssertEqual(KeyCodes.eventText(code: "KeyP", raw: "p", lone: false), "p")
        XCTAssertEqual(KeyCodes.eventText(code: "KeyP", raw: "KeyP", lone: false), "p")
        XCTAssertEqual(KeyCodes.eventText(code: "ShiftLeft", raw: "Shift", lone: true), "")
        XCTAssertEqual(KeyCodes.eventText(code: "Space", raw: "Space", lone: false), " ")
        XCTAssertEqual(KeyCodes.eventText(code: "Enter", raw: "Enter", lone: false), "\r")
    }

    /// F13–F24 as the web takes them: a driver's key, a hardware keyboard's
    /// HID usage (0x68–0x73), and AppKit's function characters.
    func testF13ThroughF24AreKeys() {
        XCTAssertEqual(KeyCodes.device("F13")?.code, "F13")
        XCTAssertEqual(KeyCodes.device("F24")?.key, "F24")
        XCTAssertNil(KeyCodes.device("F25"))
        XCTAssertEqual(KeyCodes.hid(104), "F13")
        XCTAssertEqual(KeyCodes.hid(115), "F24")
        XCTAssertEqual(KeyCodes.eventText(code: "F13", raw: "F13", lone: false), "\u{F710}")
        XCTAssertEqual(KeyCodes.eventText(code: "F24", raw: "F24", lone: false), "\u{F71B}")
        XCTAssertTrue(KeyCodes.named("F24"))
    }

    func testContextMenuIsANamedKeyOnTheKeyboardAndDriver() {
        XCTAssertEqual(KeyCodes.hid(101), "ContextMenu")
        XCTAssertEqual(KeyCodes.device("ContextMenu")?.code, "ContextMenu")
        XCTAssertEqual(KeyCodes.device("ContextMenu")?.key, "ContextMenu")
        XCTAssertTrue(KeyCodes.named("ContextMenu"))
        XCTAssertEqual(KeyCodes.eventText(code: "ContextMenu", raw: "ContextMenu", lone: false), "\u{F735}")
        #if canImport(AppKit)
        XCTAssertEqual(KeyCodes.mac[0x6E], "ContextMenu")
        #endif
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

    /// A function key is its name by its character, with or without a
    /// virtual key: F21–F24 have none on a Mac.
    func testAnAppKitFunctionKeyIsItsName() throws {
        func key(_ chars: String, code: UInt16) throws -> String {
            NodeView.keyName(try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                windowNumber: 0, context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)))
        }
        XCTAssertEqual(try key("\u{F710}", code: 105), "F13", "kVK_F13")
        XCTAssertEqual(try key("\u{F717}", code: 90), "F20", "kVK_F20")
        XCTAssertEqual(try key("\u{F718}", code: .max), "F21")
        XCTAssertEqual(try key("\u{F71B}", code: .max), "F24")
    }
    #endif
}
