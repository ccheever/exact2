#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1005 §3: a `pointerdown` action that calls `preventDefault()` keeps
/// the focus where it is, places no caret and starts no selection; the press
/// still fires, as Chrome does for a prevented pointerdown
/// (scripts/fixtures/prevent-focus.contract; host/web/tests/prevent-focus.test.mjs).
@MainActor
final class PreventFocusMacTests: XCTestCase {
    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    private func make() throws -> SheetFixture {
        clicksArriveAsInTheKeyWindow()
        let page = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../scripts/fixtures/prevent-focus.contract").standardizedFileURL
        let f = try SheetFixture(page: try String(contentsOf: page, encoding: .utf8), size: CGSize(width: 420, height: 480))
        fixture = f
        f.window.orderFrontRegardless()
        f.settle()
        return f
    }
    private func heard(_ f: SheetFixture) -> String {
        let text = f.session.agent(#"{"op":"state"}"#)
        let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
        return (json?["slots"] as? [String: Any])?["heard"] as? String ?? ""
    }
    /// A press on node `id` as a hand's comes, through the application: the
    /// session's monitor sees the down first, then the window.
    private func press(_ f: SheetFixture, _ id: String, _ modifiers: NSEvent.ModifierFlags = [], right: Bool = false) throws {
        let node = try f.node(id)
        let window = try XCTUnwrap(node.window, id)
        let at = node.convert(NSPoint(x: 12, y: node.bounds.midY), to: nil)
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: at, modifierFlags: modifiers, timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown || type == .rightMouseDown ? 1 : 0)!
        }
        let (downType, upType): (NSEvent.EventType, NSEvent.EventType) = right ? (.rightMouseDown, .rightMouseUp) : (.leftMouseDown, .leftMouseUp)
        let up = event(upType)
        NSApp.postEvent(up, atStart: true)
        NSApp.postEvent(event(downType), atStart: true)
        autoreleasepool { if let down = NSApp.nextEvent(matching: [downType == .leftMouseDown ? .leftMouseDown : .rightMouseDown], until: nil, inMode: .default, dequeue: true) { NSApp.sendEvent(down) } }
        if NSApp.nextEvent(matching: [upType == .leftMouseUp ? .leftMouseUp : .rightMouseUp], until: nil, inMode: .default, dequeue: true) != nil { NSApp.sendEvent(up) }
        f.settle()
    }

    func testAPreventedPointerdownKeepsTheFocusAndThePress() throws {
        let f = try make()
        let a = try XCTUnwrap(try f.node("a").field)
        XCTAssertTrue(f.window.makeFirstResponder(a))
        let editor = try XCTUnwrap(a.currentEditor() as? NSTextView)
        editor.setSelectedRange(NSRange(location: 2, length: 0))
        f.settle()
        for id in ["b", "box", "btn"] {
            let before = heard(f)
            try press(f, id)
            XCTAssertTrue(a.currentEditor() === editor && f.window.firstResponder === editor, "\(id): the focus stays in a, not \(String(describing: f.window.firstResponder))")
            XCTAssertEqual(editor.selectedRange(), NSRange(location: 2, length: 0), "\(id): a's caret stays")
            if id != "b" { XCTAssertTrue(heard(f).dropFirst(before.count).contains("press-\(id)"), "\(id): the press still fires: \(heard(f))") }
            XCTAssertNil(f.presenter.pointerHeld, "\(id): released")
            XCTAssertFalse(f.presenter.mouseChain.keepingFocus, "\(id): kept only while its down is handled")
        }
        // The platform's button tracks its own up in AppKit's loop; its contact ends with the loop.
        try press(f, "nbtn")
        XCTAssertTrue(heard(f).contains("press-nbtn"), heard(f))
        XCTAssertFalse(f.presenter.mouseChain.keepingFocus, "a native button: kept only while its down is handled")
        XCTAssertTrue(f.window.firstResponder === editor, "a native button: the focus stays in a")
        // A prevented secondary press keeps it too, while its down is handled.
        try press(f, "box", right: true)
        XCTAssertFalse(f.presenter.mouseChain.keepingFocus, "a right press: kept only while its down is handled")
        XCTAssertTrue(f.window.firstResponder === editor, "a right press: the focus stays in a")
        for id in ["b", "box", "btn"] { XCTAssertFalse(heard(f).split(separator: " ").contains("focus-\(id)"[...]), "\(id) never took the focus: \(heard(f))") }
        // A prevented press through the agent (its own down, never dequeued) keeps it too.
        let tap = Agent(session: f.session).tap(["id": Int(try f.node("box").id), "mouse": true])
        XCTAssertNil(tap["error"], "\(tap)")
        f.settle()
        XCTAssertTrue(f.window.firstResponder === editor, "an agent tap: the focus stays in a, not \(String(describing: f.window.firstResponder))")
        // A focusable box whose pointerdown is heard and not prevented takes the focus after it.
        XCTAssertTrue(f.window.makeFirstResponder(a))
        try press(f, "hear")
        let words = heard(f).split(separator: " ").map(String.init)
        XCTAssertTrue(words.contains("down-hear") && words.contains("focus-hear"), heard(f))
        if let down = words.lastIndex(of: "down-hear"), let focus = words.lastIndex(of: "focus-hear") { XCTAssertLessThan(down, focus, "pointerdown, then focus: \(heard(f))") }
        // A control-click that opens its menu focuses it first, as before, whatever it hears.
        XCTAssertTrue(f.window.makeFirstResponder(a))
        try press(f, "ctx", .control)
        let menu = heard(f).split(separator: " ").map(String.init)
        XCTAssertTrue(menu.contains("press-ctx") && menu.contains("focus-ctx"), "a control-click focuses, then its menu: \(heard(f))")
        // A focusable box that hears no pointerdown takes the focus, as before.
        try press(f, "free")
        XCTAssertTrue(heard(f).split(separator: " ").contains("focus-free"[...]), heard(f))
        // One that does not prevent its down takes the focus, as before.
        try press(f, "c")
        XCTAssertTrue(heard(f).contains("focus-c"), heard(f))
        XCTAssertNotNil(try f.node("c").field?.currentEditor())
    }

    /// Another button's press and release inside a prevented left contact
    /// leave its drags prevented; the next left down says again.
    func testAnotherButtonsUpLeavesThePreventedContact() throws {
        let f = try make()
        let a = try XCTUnwrap(try f.node("a").field)
        XCTAssertTrue(f.window.makeFirstResponder(a))
        let box = try f.node("box"), window = try XCTUnwrap(box.window)
        let at = box.convert(NSPoint(x: 12, y: box.bounds.midY), to: nil)
        func send(_ type: NSEvent.EventType) {
            NSApp.sendEvent(NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1,
                                               pressure: type == .leftMouseDown || type == .rightMouseDown ? 1 : 0)!)
        }
        send(.leftMouseDown)
        XCTAssertTrue(f.presenter.mouseChain.preventedContact, "the left press is prevented")
        send(.rightMouseDown)
        send(.rightMouseUp)
        XCTAssertTrue(f.presenter.mouseChain.preventedContact, "the right button's up leaves the left contact prevented")
        XCTAssertFalse(f.presenter.mouseChain.keepingFocus)
        send(.leftMouseUp)
        f.settle()
        try press(f, "free")
        XCTAssertFalse(f.presenter.mouseChain.preventedContact, "a later left down that is not prevented")
    }

    /// A down a view forwards to its superview (as a non-pressable atom
    /// forwards to its paragraph) is the same contact: it stays prevented.
    func testAForwardedDownStaysPrevented() throws {
        let f = try make()
        let box = try f.node("box"), window = try XCTUnwrap(box.window)
        let parent = try XCTUnwrap(box.superview as? NodeView)
        let at = box.convert(NSPoint(x: 12, y: box.bounds.midY), to: nil)
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let down = event(.leftMouseDown)
        NSApp.sendEvent(down)
        XCTAssertTrue(f.presenter.mouseChain.preventedContact, "the box's down is prevented")
        parent.pointerPressed(down)
        XCTAssertTrue(f.presenter.mouseChain.preventedContact, "the same down, forwarded, is still prevented")
        NSApp.sendEvent(event(.leftMouseUp))
        f.settle()
    }

    /// A text area that prevents its down takes no focus and no caret either.
    func testAPreventedPointerdownOnATextAreaKeepsTheFocus() throws {
        let f = try make()
        let a = try XCTUnwrap(try f.node("a").field)
        XCTAssertTrue(f.window.makeFirstResponder(a))
        let editor = try XCTUnwrap(a.currentEditor() as? NSTextView)
        let area = try XCTUnwrap(try f.node("t").textArea)
        area.setSelectedRange(NSRange(location: 0, length: 0))
        try press(f, "t")
        XCTAssertTrue(f.window.firstResponder === editor, "the focus stays in a: \(String(describing: f.window.firstResponder))")
        XCTAssertEqual(area.selectedRange(), NSRange(location: 0, length: 0), "no caret placed in the text area")
        XCTAssertFalse(heard(f).split(separator: " ").contains("focus-t"[...]), heard(f))
        XCTAssertNil(f.presenter.pointerHeld)
    }
}
#endif
