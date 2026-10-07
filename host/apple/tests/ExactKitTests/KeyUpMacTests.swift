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
    private func event(_ type: NSEvent.EventType, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [], _ character: String = "", repeats: Bool = false, at time: TimeInterval = 0) -> NSEvent {
        if type == .flagsChanged {
            return NSEvent.keyEvent(with: .flagsChanged, location: .zero, modifierFlags: flags, timestamp: time, windowNumber: window.windowNumber,
                                    context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: code)!
        }
        return NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: time, windowNumber: window.windowNumber,
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
        XCTAssertFalse(p.routeKey(event(.keyDown, 4, [], "h"), focused: true))
        XCTAssertEqual(heard, 0, "a plain key is the composition's")
    }

    /// What the field (2) hears, keys and inputs, in order; a flags change
    /// goes through both halves, as the session's monitor sends it.
    private final class Heard { var lines: [String] = [] }
    private func record(_ p: Presenter) -> Heard {
        let heard = Heard()
        p.views[2]!.handlers.insert("input")
        p.onInput = { _, value in heard.lines.append("input \(value)") }
        p.onKey = { id, press in if id == 2 { heard.lines.append("\(press.up ? "keyup" : "keydown") \(press.chord) \(press.code)") } }
        return heard
    }
    private func flags(_ p: Presenter, _ e: NSEvent) { _ = p.keyDown(e); p.keyUp(e) }
    private func compose(_ text: String) throws -> NSTextView {
        let editor = try XCTUnwrap(window.firstResponder as? NSTextView)
        editor.setMarkedText(text, selectedRange: NSRange(location: (text as NSString).length, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(editor.hasMarkedText())
        return editor
    }
    private let leftCommand = NSEvent.ModifierFlags(rawValue: NSEvent.ModifierFlags.command.rawValue | 0x8)

    /// A ⌘ chord during a composition (#140). Chrome hands the page the
    /// chord's keydown before the input method's result, with the composed
    /// text already in the field and reported to `input` (CDP's
    /// `Input.imeSetComposition`, then ⌘B: `input 한`, then the keys).
    /// AppKit reports composed text only once committed, so the chord
    /// commits it first: its handlers hear the same value, ⌘'s own keys
    /// pass through the composition, and the keyups follow.
    func testACommandChordCommitsTheCompositionThenReachesTheHandlers() throws {
        let p = try fixture()
        let heard = record(p)
        let editor = try compose("한")
        flags(p, event(.flagsChanged, 55, leftCommand))
        let chord = event(.keyDown, 11, leftCommand, "b")
        XCTAssertFalse(p.routeKey(chord, focused: true), "unprevented, AppKit gets it")
        XCTAssertFalse(editor.hasMarkedText())
        XCTAssertTrue(p.endedComposition(chord), "its menu item, too, is refused")
        XCTAssertFalse(p.routeKey(event(.keyUp, 11, leftCommand, "b"), focused: true))
        flags(p, event(.flagsChanged, 55, []))
        XCTAssertEqual(heard.lines, ["keydown Meta+Meta MetaLeft", "input 한", "keydown Meta+b KeyB", "keyup Meta+b KeyB", "keyup Meta MetaLeft"])
        XCTAssertEqual(editor.string, "한")
        XCTAssertFalse(p.endedComposition(event(.keyDown, 11, leftCommand, "b", at: 1)), "a later ⌘B is a chord like any")
    }

    /// A composer's ⌘Enter during a composition sends what was typed: its
    /// handler reads the committed text, clears the field and prevents the
    /// default, and the cleared value lands. The Send button that declares
    /// ⌘Enter is not pressed: the web's shortcut listener skips a keydown
    /// that arrived composing.
    func testACommandEnterDuringACompositionSendsTheComposedText() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "button", "props": ["accessibilityKeyShortcuts": "Meta+Enter", "accessibilityLabel": "Send"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "frame", "id": 3, "x": 10, "y": 50, "w": 80, "h": 30],
        ]))
        p.views[3]!.handlers.insert("press")
        var presses = 0
        p.onPress = { _ in presses += 1 }
        let heard = record(p)
        let editor = try compose("한")
        let keyed = p.onKey
        p.onKey = { id, press in
            keyed?(id, press)
            guard id == 2, !press.up, press.chord == "Meta+Enter" else { return }
            heard.lines.append("sent \(editor.string)")
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": ""]]]))
            p.defaultPrevented = true
        }
        // kVK_Return is 36.
        XCTAssertTrue(p.routeKey(event(.keyDown, 36, leftCommand, "\r"), focused: true))
        XCTAssertEqual(heard.lines, ["input 한", "keydown Meta+Enter Enter", "sent 한"])
        XCTAssertFalse(editor.hasMarkedText())
        XCTAssertEqual(editor.string, "")
        XCTAssertEqual(presses, 0)
        XCTAssertTrue(p.routeKey(event(.keyDown, 36, leftCommand, "\r", at: 1), focused: true), "without a composition, the button's chord")
        XCTAssertEqual(presses, 1)
    }

    /// Shift held as a composition begins (Korean's doubled consonants) and
    /// released inside it: its keyup still comes, as Chrome forwards every
    /// flags change, so no app's held-Shift state sticks.
    func testAModifierReleasedDuringACompositionIsAKeyUp() throws {
        let p = try fixture()
        let heard = record(p)
        // kVK_Shift is 56; 0x2 is the left Shift's device bit.
        flags(p, event(.flagsChanged, 56, NSEvent.ModifierFlags(rawValue: NSEvent.ModifierFlags.shift.rawValue | 0x2)))
        _ = try compose("ㅆ")
        flags(p, event(.flagsChanged, 56, []))
        XCTAssertEqual(heard.lines.filter { $0.hasPrefix("key") }, ["keydown Shift+Shift ShiftLeft", "keyup Shift ShiftLeft"])
    }
}
#endif
