#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// #140: AppKit's keyUp and a flags change that releases a modifier reach the
/// `keyup` handlers at the focus and above, as the monitor routes a keyboard's
/// keys; each key event carries DOM's `code` and a keydown its `repeat`.
final class KeyUpMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// A field (2) in a column (1); both hear `key` and `keyup`.
    private func fixture() throws -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "input", "props": ["value": ""]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 200, "h": 30],
        ]))
        for id: UInt32 in [1, 2] { p.views[id]!.handlers.formUnion(["key", "keyup"]) }
        window.makeFirstResponder(try XCTUnwrap(p.views[2]?.field))
        return p
    }
    private func event(_ type: NSEvent.EventType, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [], _ character: String = "", repeats: Bool = false) -> NSEvent {
        if type == .flagsChanged {
            return NSEvent.keyEvent(with: .flagsChanged, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber,
                                    context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: code)!
        }
        return NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber,
                                context: nil, characters: character, charactersIgnoringModifiers: character, isARepeat: repeats, keyCode: code)!
    }

    func testAKeyUpBubblesWithItsCodeAndAKeyDownSaysWhenItRepeats() throws {
        let p = try fixture()
        var heard: [String] = []
        p.onKey = { id, press in heard.append("\(id) \(press.up ? "up" : "down") \(press.chord) \(press.code) \(press.repeats)") }
        // kVK_ANSI_B is 11.
        XCTAssertFalse(p.routeKey(event(.keyDown, 11, .command, "b"), focused: true))
        XCTAssertFalse(p.routeKey(event(.keyDown, 11, .command, "b", repeats: true), focused: true))
        XCTAssertFalse(p.routeKey(event(.keyUp, 11, .command, "b"), focused: true))
        XCTAssertEqual(heard, ["2 down Meta+b KeyB false", "1 down Meta+b KeyB false",
                               "2 down Meta+b KeyB true", "1 down Meta+b KeyB true",
                               "2 up Meta+b KeyB false", "1 up Meta+b KeyB false"])
        XCTAssertEqual(KeyPress("Meta+b", code: "KeyB", repeats: true).payload, "Meta+b\nKeyB\ntrue")
        XCTAssertEqual(KeyPress("a").payload, "a", "a chord alone when the host knows no more")
    }

    /// A modifier is a flags change to AppKit: pressed, a keydown that holds
    /// it; released, a keyup that no longer does. The device bits tell the
    /// sides apart, so the left ⌘'s release while the right is held is one.
    func testAModifiersReleaseIsAKeyUpThatNoLongerHoldsIt() throws {
        let p = try fixture()
        var heard: [String] = []
        p.onKey = { id, press in if id == 2 { heard.append("\(press.up ? "up" : "down") \(press.chord) \(press.code)") } }
        let left = NSEvent.ModifierFlags(rawValue: NSEvent.ModifierFlags.command.rawValue | 0x8)
        let both = NSEvent.ModifierFlags(rawValue: left.rawValue | 0x10)
        let right = NSEvent.ModifierFlags(rawValue: NSEvent.ModifierFlags.command.rawValue | 0x10)
        // kVK_Command is 55, kVK_RightCommand 54.
        for (code, flags) in [(UInt16(55), left), (54, both), (55, right), (54, [])] as [(UInt16, NSEvent.ModifierFlags)] {
            let e = event(.flagsChanged, code, flags)
            _ = p.keyDown(e)
            p.keyUp(e)
        }
        XCTAssertEqual(heard, ["down Meta+Meta MetaLeft", "down Meta+Meta MetaRight", "up Meta+Meta MetaLeft", "up Meta MetaRight"])
    }

    func testAKeyUpDuringCompositionIsTheInputMethods() throws {
        let p = try fixture()
        var heard = 0
        p.onKey = { _, _ in heard += 1 }
        let editor = try XCTUnwrap(window.firstResponder as? NSTextView)
        editor.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        p.keyUp(event(.keyUp, 4, [], "h"))
        XCTAssertEqual(heard, 0)
    }
}
#endif
