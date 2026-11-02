import AppKit
import XCTest

// Compile with the production native-module facade, its generated app keys,
// T3Composer.swift and T3WindowChrome.swift. No transport or server is needed.
let exactModule: ExactModule.Type = ExactModule.self

private let resolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let click: ExactHatches.ActFn = { pointer, node, action in
    if node == 2, action == 0 { pointer?.assumingMemoryBound(to: Int.self).pointee += 1 }
    return 0
}
private let log: ExactHatches.LogFn = { _, _, _ in }
private let delegate: ExactHatches.DelegateFn = { _, _, _ in }

private final class ComposerFixture {
    let controller = T3Composer()
    let clicks = UnsafeMutablePointer<Int>.allocate(capacity: 1)
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    let hooks: ExactHatches
    let window: NSWindow
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 100))
    let button = NSView(frame: NSRect(x: 0, y: 100, width: 100, height: 30))
    let composer: ExactElement

    init() {
        _ = NSApplication.shared
        clicks.initialize(to: 0)
        table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
        table.storeBytes(of: UInt32(40), as: UInt32.self)
        table.storeBytes(of: unsafeBitCast(resolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(click, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(log, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(delegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
        hooks = ExactHatches(host: UnsafeMutableRawPointer(clicks), table: UnsafeRawPointer(table))!
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200),
            styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView?.addSubview(editor)
        window.contentView?.addSubview(button)
        editor.isEditable = true
        window.makeFirstResponder(editor)
        composer = ExactElement(hatch: .t3Composer, id: "composer", node: 1, hatches: hooks)
        composer.view = editor
        composer.platform = editor
        controller.install(composer)
    }

    func activate() -> Bool {
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        let deadline = Date(timeIntervalSinceNow: 5)
        while !(NSApp.isActive && NSApp.keyWindow === window), Date() < deadline {
            if let event = NSApp.nextEvent(matching: .any, until: Date(timeIntervalSinceNow: 0.01), inMode: .default, dequeue: true) { NSApp.sendEvent(event) }
        }
        return NSApp.isActive && NSApp.keyWindow === window && window.firstResponder === editor
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
    func testAppearanceChangesWindowAndRestoresSystemInheritance() {
        let fixture = ComposerFixture()
        let chrome = T3WindowChrome()
        chrome.install(fixture.composer)
        chrome.setAppearance("dark")
        XCTAssertEqual(fixture.window.appearance?.name, .darkAqua)
        chrome.setAppearance("light")
        XCTAssertEqual(fixture.window.appearance?.name, .aqua)
        chrome.setAppearance("system")
        XCTAssertNil(fixture.window.appearance)
        chrome.destroy()
    }

    func testReturnModifiersReachTheSharedField() {
        let fixture = ComposerFixture()
        for flags: NSEvent.ModifierFlags in [[], .command, .shift, .control, .option, [.command, .shift]] {
            let event = fixture.event(modifiers: flags)
            XCTAssertTrue(fixture.controller.handle(event) === event)
        }
        XCTAssertEqual(fixture.clicks.pointee, 0)
    }


    func testModelPickerKeysBelongToContractNotTheReturnMonitor() {
        let fixture = ComposerFixture()
        let field = NSTextField(frame: NSRect(x: 0, y: 130, width: 200, height: 24))
        fixture.window.contentView?.addSubview(field)
        fixture.window.makeFirstResponder(field)
        // Escape, the arrows and Return in the picker's search field are the
        // field's own `key`/`submit` (Contract) and the dismiss layer's command.
        for key: UInt16 in [53, 125, 126, 36] { XCTAssertNotNil(fixture.controller.handle(fixture.event(key: key))) }
        XCTAssertEqual(fixture.clicks.pointee, 0)
    }

    func testAnchorsReportRowOffsetsOnlyWhenTheyMove() {
        let fixture = ComposerFixture()
        let row = NSView(frame: NSRect(x: 40, y: 0, width: 600, height: 32))
        let traits = NSView(frame: NSRect(x: 147, y: 2, width: 90.9, height: 28))
        let runtime = NSView(frame: NSRect(x: 250.9, y: 2, width: 133, height: 28))
        row.addSubview(traits); row.addSubview(runtime)
        fixture.window.contentView?.addSubview(row)
        var changes = 0
        let controller = T3Composer(changed: { name in XCTAssertEqual(name, "t3.status"); changes += 1 })
        // The host retains its elements; the controller holds them weakly.
        let elements = [(5, "traits", traits), (6, "runtime", runtime)].map { (node: UInt32, name: String, view: NSView) -> ExactElement in
            let element = ExactElement(hatch: .t3Anchor, id: name, node: node, hatches: fixture.hooks)
            element.view = view; element.platform = view
            element.data = ExactData(["anchor": name])
            return element
        }
        elements.forEach(controller.install)
        XCTAssertEqual(controller.anchorFrames["traits"], [147, 91])
        XCTAssertEqual(controller.anchorFrames["runtime"], [251, 133])
        XCTAssertEqual((controller.status["anchors"] as? [String: [Double]])?.count, 2)
        let before = changes
        row.frame.origin.x = 80
        controller.measure()
        XCTAssertEqual(changes, before, "moving the whole row changes no offset")
        traits.frame.size.width = 70
        runtime.frame.origin.x = 230
        XCTAssertEqual(controller.anchorFrames["runtime"], [230, 133], "a frame change re-measures on its own")
        XCTAssertGreaterThan(changes, before)
        controller.remove(elements[1])
        XCTAssertNil(controller.anchorFrames["runtime"], "a removed anchor stops reporting")
        controller.destroy()
    }

    func testNativeMonitorLeavesReturnToTheTextSystem() {
        let fixture = ComposerFixture()
        fixture.editor.string = "Prompt"
        fixture.editor.setSelectedRange(NSRange(location: 6, length: 0))
        // Application delivery needs an active key window; a hidden fixture drops
        // the event before the text system, even without the adapter installed.
        XCTAssertTrue(fixture.activate(), "Return is routed through the active key window to its editor")
        // This fixture has no Contract key handler, so ordinary AppKit inserts the newline.
        NSApp.sendEvent(fixture.event())
        XCTAssertEqual(fixture.clicks.pointee, 0)
        XCTAssertEqual(fixture.editor.string, "Prompt\n")
    }

    func testReturnAndRepeatsAreNotConsumedByTheOSAdapter() {
        let fixture = ComposerFixture()
        fixture.editor.string = "Prompt"
        XCTAssertNotNil(fixture.controller.handle(fixture.event()))
        XCTAssertEqual(fixture.editor.string, "Prompt")
        XCTAssertNotNil(fixture.controller.handle(fixture.event(repeatKey: true)))
        XCTAssertNotNil(fixture.controller.handle(fixture.event(key: 76)))
        XCTAssertEqual(fixture.clicks.pointee, 0)
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
        fixture.controller.remove(fixture.composer)
        XCTAssertNotNil(fixture.controller.handle(fixture.event()))
        fixture.controller.install(fixture.composer)
        fixture.controller.destroy()
        XCTAssertNotNil(fixture.controller.handle(fixture.event()))
        XCTAssertEqual(fixture.clicks.pointee, 0)
    }
}

private func runComposerTests() {
    var executed = 0, failures = 0, succeeded = true
    for suite in [ComposerTests.defaultTestSuite, PromptPreviewEditorTests.defaultTestSuite] {
        suite.run()
        guard let run = suite.testRun, run.executionCount == suite.testCaseCount else { exit(1) }
        executed += run.executionCount; failures += run.totalFailureCount; succeeded = succeeded && run.hasSucceeded
    }
    guard executed >= 13 else { exit(1) }
    print("Composer: \(executed) tests, \(failures) failures")
    exit(succeeded ? 0 : 1)
}

final class ComposerTestApplicationDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        DispatchQueue.main.async { runComposerTests() }
    }
}
let composerTestApplication = NSApplication.shared
composerTestApplication.setActivationPolicy(.regular)
let composerTestDelegate = ComposerTestApplicationDelegate()
composerTestApplication.delegate = composerTestDelegate
composerTestApplication.run()
