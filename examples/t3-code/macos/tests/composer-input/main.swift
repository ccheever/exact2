import AppKit
import XCTest

// T3ComposerInput.swift: the composer's focus reaches its node on a click, not only on
// typing; ⌘B bolds under a non-Latin source and while a syllable is composing; a press elsewhere
// ends composition first. The transcript's remembered position moved to Contract (ChatColumn in
// app-main.contract); its cases are in timeline-anchor.test.ts.
// Real AppKit views in a real window; compiled with every file in modules/apple.

private let hatchResolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let hatchAct: ExactHatches.ActFn = { _, _, _ in 0 }
private let hatchLog: ExactHatches.LogFn = { _, _, _ in }
private let hatchDelegate: ExactHatches.DelegateFn = { _, _, _ in }
private func makeHooks() -> ExactHatches {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
    table.storeBytes(of: UInt32(40), as: UInt32.self)
    table.storeBytes(of: unsafeBitCast(hatchResolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(hatchAct, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(hatchLog, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(hatchDelegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    return ExactHatches(host: nil, table: UnsafeRawPointer(table))!
}

/// Stands in for Exact's node, the text view's real delegate: focus on begin-editing, blur on end.
final class Node: NSObject, NSTextViewDelegate {
    var events: [String] = []
    func textDidBeginEditing(_ notification: Notification) { events.append("focus") }
    func textDidEndEditing(_ notification: Notification) { events.append("blur") }
}
private func tick(_ seconds: TimeInterval = 0.02) { let end = Date(timeIntervalSinceNow: seconds); while Date() < end { RunLoop.current.run(mode: .default, before: end) } }

final class ComposerFixture {
    let hooks = makeHooks()
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 420, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
    let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let button = NSButton(frame: NSRect(x: 0, y: 160, width: 80, height: 30))
    let node = Node()
    let composer = T3Composer()
    let composerInput = T3ComposerInput()
    let element: ExactElement
    init() {
        _ = NSApplication.shared
        window.isReleasedWhenClosed = false
        editor.isEditable = true; editor.isRichText = false; editor.allowsUndo = true
        editor.delegate = node
        scroller.documentView = editor
        window.contentView?.addSubview(scroller); window.contentView?.addSubview(button)
        element = ExactElement(hatch: .t3Composer, id: "composer", node: 1, hatches: hooks)
        element.view = scroller; element.platform = editor
        element.data = ExactData(["snapshot-owner": "owner-a"])
        composer.install(element) // the real delegate proxy goes in front of the node
        composer.editor.styler.richText = true
        composerInput.install(element)
    }
    func key(_ characters: String, _ ignoring: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command]) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: characters, charactersIgnoringModifiers: ignoring, isARepeat: false, keyCode: code)!
    }
    func press(at point: NSPoint) -> NSEvent {
        NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }
}

final class ComposerInputTests: XCTestCase {
    func testClickFocusReachesTheNodeOnce() {
        let f = ComposerFixture()
        f.window.makeFirstResponder(f.button); tick()
        XCTAssertEqual(f.node.events.filter { $0 == "focus" }.count, 0)
        // A click into an existing draft: first responder only, no edit.
        f.editor.string = "existing draft"
        f.window.makeFirstResponder(f.editor); tick()
        XCTAssertEqual(f.node.events.filter { $0 == "focus" }.count, 1, "focus reaches Exact's node through the proxy without typing")
        XCTAssertTrue(f.composerInput.composerFocused)
        f.window.makeFirstResponder(f.button); tick()
        XCTAssertEqual(f.node.events.last, "blur")
        XCTAssertFalse(f.composerInput.composerFocused)
        f.window.makeFirstResponder(f.editor); tick()
        XCTAssertEqual(f.node.events.filter { $0 == "focus" }.count, 2, "each return of the focus is a new focus")
        XCTAssertEqual(f.composerInput.bridged, 2)
    }

    func testChordKeyUnderAKoreanSource() {
        let f = ComposerFixture()
        XCTAssertEqual(T3ComposerInput.chordKey(f.key("b", "b", 11)), "b")
        XCTAssertEqual(T3ComposerInput.chordKey(f.key("ㅠ", "ㅠ", 11)), "b", "Korean 2-Set reports ㅠ for the B key")
        XCTAssertEqual(T3ComposerInput.chordKey(f.key("ㅑ", "ㅑ", 34)), "i")
        XCTAssertEqual(T3ComposerInput.chordKey(f.key("ㅠ", "ㅠ", 99)), "ㅠ", "an unmapped key keeps its characters")
    }

    func testCommandBBoldsAfterAClickAndUnderKorean() {
        let f = ComposerFixture()
        f.editor.string = "draft"
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.setSelectedRange(NSRange(location: 0, length: 5))
        XCTAssertNil(f.composer.handle(f.key("b", "b", 11)), "⌘B is consumed")
        XCTAssertEqual(f.editor.string, "**draft**")
        f.editor.string = "word"; f.editor.setSelectedRange(NSRange(location: 0, length: 4))
        XCTAssertNil(f.composer.handle(f.key("ㅠ", "ㅠ", 11)), "⌘B under Korean 2-Set is consumed")
        XCTAssertEqual(f.editor.string, "**word**")
    }

    func testCommandBEndsCompositionThenBolds() {
        let f = ComposerFixture()
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.string = ""
        f.editor.insertText("안", replacementRange: f.editor.selectedRange())
        f.editor.setMarkedText("녕", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(f.editor.hasMarkedText())
        XCTAssertNil(f.composer.handle(f.key("ㅠ", "ㅠ", 11)))
        XCTAssertFalse(f.editor.hasMarkedText(), "composition ended")
        XCTAssertTrue(f.editor.string.hasPrefix("안녕"), "the composed syllable stays: \(f.editor.string)")
        XCTAssertTrue(f.editor.string.contains("**"), "and the chord toggled bold: \(f.editor.string)")
    }

    func testPressElsewhereEndsComposition() {
        let f = ComposerFixture()
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.string = ""
        f.editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        f.composerInput.handle(f.press(at: NSPoint(x: 20, y: 60))) // inside the composer: composition goes on
        XCTAssertTrue(f.editor.hasMarkedText())
        f.composerInput.handle(f.press(at: NSPoint(x: 20, y: 175))) // on the button
        XCTAssertFalse(f.editor.hasMarkedText())
        XCTAssertEqual(f.editor.string, "한")
    }

    func testReopenedFieldKeepsExplicitSelectAll() {
        let f = ComposerFixture()
        let old = NSTextField(frame: NSRect(x: 100, y: 200, width: 200, height: 22)); old.stringValue = "set"
        f.window.contentView?.addSubview(old)
        f.window.makeFirstResponder(old); tick()
        (old.currentEditor() as? NSTextView)?.setSelectedRange(NSRange(location: 3, length: 0)); tick()
        // Reopen checkout/rename with the same value. FieldOpenSelect focuses and selects its text;
        // the removed scheme-remount workaround must not restore the previous field's caret.
        old.removeFromSuperview()
        let new = NSTextField(frame: NSRect(x: 100, y: 200, width: 200, height: 22)); new.stringValue = "set"
        f.window.contentView?.addSubview(new)
        f.window.makeFirstResponder(new)
        (new.currentEditor() as? NSTextView)?.setSelectedRange(NSRange(location: 0, length: 3)); tick(); tick()
        XCTAssertEqual((new.currentEditor() as? NSTextView)?.selectedRange(), NSRange(location: 0, length: 3), "select-on-open stays selected")
        // A field focused afresh (another text, or the old one still in the window) keeps AppKit's select-all.
        let other = NSTextField(frame: NSRect(x: 100, y: 170, width: 200, height: 22)); other.stringValue = "else"
        f.window.contentView?.addSubview(other)
        f.window.makeFirstResponder(other); tick(); tick()
        XCTAssertEqual((other.currentEditor() as? NSTextView)?.selectedRange(), NSRange(location: 0, length: 4))
    }
}

let suite = XCTestSuite(forTestCaseClass: ComposerInputTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.failureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
