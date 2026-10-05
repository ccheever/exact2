import AppKit
import XCTest

// Compile with the production native-module facade, its generated app keys,
// T3Composer.swift and T3WindowChrome.swift. No transport or server is needed.
let exactModule: ExactModule.Type = ExactModule.self

private let resolve: ExactHooks.ResolveFn = { _, _, _, _, _ in 0 }
private let click: ExactHooks.ActFn = { pointer, node, action in
    if node == 2, action == 0 { pointer?.assumingMemoryBound(to: Int.self).pointee += 1 }
    return 0
}
private let log: ExactHooks.LogFn = { _, _, _ in }
private let delegate: ExactHooks.DelegateFn = { _, _, _ in }

private final class ComposerFixture {
    let controller = T3Composer()
    let clicks = UnsafeMutablePointer<Int>.allocate(capacity: 1)
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    let hooks: ExactHooks
    let window: NSWindow
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 100))
    let button = NSView(frame: NSRect(x: 0, y: 100, width: 100, height: 30))
    let composer: ExactElement
    let send: ExactElement

    init() {
        _ = NSApplication.shared
        clicks.initialize(to: 0)
        table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
        table.storeBytes(of: UInt32(40), as: UInt32.self)
        table.storeBytes(of: unsafeBitCast(resolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(click, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(log, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(delegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
        hooks = ExactHooks(host: UnsafeMutableRawPointer(clicks), table: UnsafeRawPointer(table))!
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200),
            styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView?.addSubview(editor)
        window.contentView?.addSubview(button)
        editor.isEditable = true
        window.makeFirstResponder(editor)
        composer = ExactElement(hook: .t3Composer, id: "composer", node: 1, hooks: hooks)
        composer.view = editor
        composer.platform = editor
        send = ExactElement(hook: .t3Send, id: "send-message", node: 2, hooks: hooks)
        send.view = button
        controller.install(composer)
        controller.install(send)
    }

    func event(key: UInt16 = 36, modifiers: NSEvent.ModifierFlags = [], repeatKey: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers,
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: repeatKey, keyCode: key)!
    }

    deinit {
        controller.destroy()
        window.close()
        table.deallocate()
        clicks.deinitialize(count: 1)
        clicks.deallocate()
    }
}

final class ComposerTests: XCTestCase {
    func testTitlebarKeepsNativeButtonsCenteredAndContentFullSize() {
        let fixture = ComposerFixture()
        fixture.window.styleMask.formUnion([.closable, .miniaturizable, .resizable, .fullSizeContentView])
        let chrome = T3WindowChrome()
        chrome.install(fixture.composer)
        fixture.window.contentView?.superview?.layoutSubtreeIfNeeded()
        let close = fixture.window.standardWindowButton(.closeButton)!
        let bounds = close.convert(close.bounds, to: nil)
        XCTAssertEqual(fixture.window.frame.height - bounds.midY, 26, accuracy: 1)
        XCTAssertEqual(fixture.window.contentView!.frame.height, fixture.window.frame.height, accuracy: 1)
        XCTAssertTrue(fixture.window.titlebarAppearsTransparent)
        XCTAssertEqual(fixture.window.titleVisibility, .hidden)
        XCTAssertNotNil(fixture.window.toolbar)
        chrome.destroy()
        XCTAssertNil(fixture.window.toolbar)
    }

    func testApplicationDeliveryDispatchesWithoutInsertingNewline() {
        let fixture = ComposerFixture()
        fixture.editor.string = "Prompt"
        // The real application event path must execute the installed monitor,
        // not just its independently testable decision function.
        NSApp.sendEvent(fixture.event())
        XCTAssertEqual(fixture.clicks.pointee, 1)
        XCTAssertEqual(fixture.editor.string, "Prompt")
    }

    func testReturnDispatchesOnceWithoutEditingText() {
        let fixture = ComposerFixture()
        fixture.editor.string = "Prompt"
        XCTAssertNil(fixture.controller.handle(fixture.event()))
        XCTAssertEqual(fixture.clicks.pointee, 1)
        XCTAssertEqual(fixture.editor.string, "Prompt")
        XCTAssertNil(fixture.controller.handle(fixture.event(repeatKey: true)))
        XCTAssertEqual(fixture.clicks.pointee, 1)
        XCTAssertNil(fixture.controller.handle(fixture.event(key: 76)))
        XCTAssertEqual(fixture.clicks.pointee, 2)
    }

    func testModifiedReturnStaysWithTextSystem() {
        let fixture = ComposerFixture()
        for modifiers: NSEvent.ModifierFlags in [.shift, .command, .control, .option, [.command, .shift]] {
            let event = fixture.event(modifiers: modifiers)
            XCTAssertTrue(fixture.controller.handle(event) === event)
        }
        let shiftReturn = fixture.event(modifiers: .shift)
        fixture.editor.keyDown(with: shiftReturn)
        XCTAssertEqual(fixture.editor.string, "\n")
        XCTAssertEqual(fixture.clicks.pointee, 0)
    }

    func testIMECompositionNeverDispatches() {
        let fixture = ComposerFixture()
        fixture.editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(fixture.editor.hasMarkedText())
        let event = fixture.event()
        XCTAssertTrue(fixture.controller.handle(event) === event)
        XCTAssertTrue(fixture.controller.handle(fixture.event(repeatKey: true)) != nil)
        XCTAssertEqual(fixture.clicks.pointee, 0)
    }

    func testOtherKeysAndOtherFocusedFieldsAreUntouched() {
        let fixture = ComposerFixture()
        let otherKey = fixture.event(key: 0)
        XCTAssertTrue(fixture.controller.handle(otherKey) === otherKey)
        let field = NSTextView(frame: NSRect(x: 0, y: 130, width: 100, height: 50))
        fixture.window.contentView?.addSubview(field)
        fixture.window.makeFirstResponder(field)
        let enter = fixture.event()
        XCTAssertTrue(fixture.controller.handle(enter) === enter)
        XCTAssertEqual(fixture.clicks.pointee, 0)
    }

    func testOtherWindowAndReadonlyComposerAreUntouched() {
        let first = ComposerFixture(), second = ComposerFixture()
        let otherWindow = second.event()
        XCTAssertTrue(first.controller.handle(otherWindow) === otherWindow)
        first.editor.isEditable = false
        let enter = first.event()
        XCTAssertTrue(first.controller.handle(enter) === enter)
        XCTAssertEqual(first.clicks.pointee, 0)
    }

    func testEndedControlsAndDestroyDisableDispatch() {
        let fixture = ComposerFixture()
        fixture.controller.remove(fixture.send)
        XCTAssertNotNil(fixture.controller.handle(fixture.event()))
        fixture.controller.install(fixture.send)
        fixture.controller.remove(fixture.composer)
        XCTAssertNotNil(fixture.controller.handle(fixture.event()))
        fixture.controller.install(fixture.composer)
        fixture.send.ended = true
        XCTAssertNotNil(fixture.controller.handle(fixture.event()))
        fixture.send.ended = false
        fixture.controller.destroy()
        XCTAssertNotNil(fixture.controller.handle(fixture.event()))
        XCTAssertEqual(fixture.clicks.pointee, 0)
    }
}

let suite = ComposerTests.defaultTestSuite
suite.run()
guard let run = suite.testRun, run.executionCount == 8 else { exit(1) }
print("Composer: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
