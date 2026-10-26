import AppKit
import XCTest

// The composer's keys around the shared field (T3Composer.handle): formatting keys pass to it, under Korean
// too, and leave its composition alone. Its focus on a click and the commit of its composition on a press
// elsewhere are Exact's (the host's KeyUpMacTests). Real AppKit views in a real window; compiled with every
// file in modules/apple.

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

/// Stands in for Exact's node, the text view's real delegate.
final class Node: NSObject, NSTextViewDelegate {}
private func tick(_ seconds: TimeInterval = 0.02) { let end = Date(timeIntervalSinceNow: seconds); while Date() < end { RunLoop.current.run(mode: .default, before: end) } }

final class ComposerFixture {
    let hooks = makeHooks()
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 420, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
    let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let button = NSButton(frame: NSRect(x: 0, y: 160, width: 80, height: 30))
    let node = Node()
    let composer = T3Composer()
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
    }
    deinit { composer.destroy(); window.close() }
    func key(_ characters: String, _ ignoring: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command]) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: characters, charactersIgnoringModifiers: ignoring, isARepeat: false, keyCode: code)!
    }
}

final class ComposerInputTests: XCTestCase {
    func testFormattingKeysPassThroughAfterAClickAndUnderKorean() {
        let f = ComposerFixture()
        f.editor.string = "draft"
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.setSelectedRange(NSRange(location: 0, length: 5))
        XCTAssertNotNil(f.composer.handle(f.key("b", "b", 11)))
        XCTAssertEqual(f.editor.string, "draft")
        f.editor.string = "word"; f.editor.setSelectedRange(NSRange(location: 0, length: 4))
        XCTAssertNotNil(f.composer.handle(f.key("ㅠ", "ㅠ", 11)))
        XCTAssertEqual(f.editor.string, "word")
    }

    func testFormattingAdapterLeavesCompositionForTheSharedHost() {
        let f = ComposerFixture()
        f.window.makeFirstResponder(f.editor); tick()
        f.editor.string = ""
        f.editor.insertText("안", replacementRange: f.editor.selectedRange())
        f.editor.setMarkedText("녕", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(f.editor.hasMarkedText())
        XCTAssertNotNil(f.composer.handle(f.key("ㅠ", "ㅠ", 11)))
        XCTAssertTrue(f.editor.hasMarkedText())
        XCTAssertTrue(f.editor.string.hasPrefix("안녕"), "the composed syllable stays: \(f.editor.string)")
        XCTAssertFalse(f.editor.string.contains("**"))
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
