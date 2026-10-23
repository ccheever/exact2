import AppKit
import XCTest

// The surface launcher's letters beside type-to-focus (T3Composer) in a real window. The menu bar is the host's and
// Contract's now (`-exact-menu`, `-exact-menu-remove`), with nothing of the module's in it but Paste as Text (menus).
// Compiled with every file in modules/apple (T3Module included).

private var acted: [(node: UInt32, action: UInt32)] = []
private let hatchResolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let hatchAct: ExactHatches.ActFn = { _, node, action in acted.append((node, action)); return 0 }
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

private func key(_ characters: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command], window: NSWindow? = nil, at time: TimeInterval = 0) -> NSEvent {
    NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: time, windowNumber: window?.windowNumber ?? 0, context: nil,
        characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
}

final class LauncherTests: XCTestCase {
    /// The launcher's letters are its rows' `aria-keyshortcuts`
    /// (shell-panels.contract SurfaceRow), which Exact's session monitor presses (ShortcutsMac `perform`) wherever the
    /// focus is but an input context, and its `autofocus` focuses it on mount. AppKit calls a window's local key monitors
    /// in no fixed order, so the composer's type-to-focus (T3Composer.handle) must leave a launcher letter unconsumed
    /// whether it runs before Exact's monitor or after it: the letter is pressed once and never typed into the composer.
    func testTypeToFocusLeavesTheLaunchersLettersToItsShortcuts() {
        _ = NSApplication.shared
        let hooks = makeHooks()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let text = NSTextView(frame: NSRect(x: 0, y: 0, width: 200, height: 100))
        text.isEditable = true
        window.contentView!.addSubview(text)
        let launcherView = LauncherView(frame: NSRect(x: 220, y: 0, width: 260, height: 300))
        window.contentView!.addSubview(launcherView)
        let composer = T3Composer()
        let field = ExactElement(hatch: .t3Composer, id: "composer", node: 3, hatches: hooks)
        field.view = text
        field.platform = text
        composer.install(field)
        let element = ExactElement(hatch: .t3Launcher, id: "surface-chooser", node: 7, hatches: hooks)
        element.view = launcherView
        element.data = ExactData(["surface-launcher-keys": "BTFDL"])
        composer.install(element)
        // Exact's monitor: a launcher row's shortcut takes its letter, either case, by the physical key under a
        // non-Latin source (Shortcut.matches); the event goes no further.
        var pressed: [String] = []
        func exact(_ event: NSEvent) -> NSEvent? {
            let typed = event.charactersIgnoringModifiers ?? ""
            let letter = typed.unicodeScalars.allSatisfy(\.isASCII) ? typed : (T3LatinChords.physicalKeys[event.keyCode] ?? typed)
            guard event.modifierFlags.intersection([.command, .control, .option]).isEmpty, "BTFDL".contains(letter.uppercased()) else { return event }
            pressed.append(letter.uppercased())
            return nil
        }
        func press(_ event: NSEvent, composerFirst: Bool) {
            if composerFirst { _ = composer.handle(event).flatMap(exact) } else { _ = exact(event).flatMap(composer.handle) }
        }
        var time: TimeInterval = 100 // each press its own timestamp, as a keyboard's
        for composerFirst in [false, true] {
            for responder in [nil, launcherView] as [NSResponder?] {
                window.makeFirstResponder(responder)
                pressed = []
                time += 1; press(key("f", 3, [], window: window, at: time), composerFirst: composerFirst)
                time += 1; press(key("L", 37, [.shift], window: window, at: time), composerFirst: composerFirst)
                time += 1; press(key("ㅇ", 2, [], window: window, at: time), composerFirst: composerFirst) // Korean 2-Set's D key
                XCTAssertEqual(pressed, ["F", "L", "D"], "each letter pressed once, composer's monitor first: \(composerFirst)")
                XCTAssertEqual(text.string, "", "type-to-focus never takes a launcher letter")
                XCTAssertNil(NSApp.nextEvent(matching: .keyDown, until: Date(), inMode: .default, dequeue: true), "nothing is posted again")
            }
        }
        XCTAssertEqual(launcherView.keys, [], "nothing is handed to the view's keyDown")
        XCTAssertTrue(T3Composer.launcherLetter(key("f", 3, [], window: window), letters: "FLD", live: true))
        XCTAssertFalse(T3Composer.launcherLetter(key("f", 3, [.command], window: window), letters: "FLD", live: true), "a ⌘ chord is not the launcher's")
        XCTAssertFalse(T3Composer.launcherLetter(key("f", 3, [.option], window: window), letters: "FLD", live: true))
        XCTAssertFalse(T3Composer.launcherLetter(key("f", 3, [], window: window), letters: "FLD", live: false))
        // A letter the launcher does not list still goes to the composer (ChatView's type-to-focus), as does every
        // letter once the launcher is hidden or gone: the composer takes the focus and the key goes on to it, as
        // the window then delivers it.
        func typeToFocus(_ event: NSEvent) {
            window.makeFirstResponder(nil)
            let passed = composer.handle(event)
            XCTAssertTrue(passed === event, "the key goes on")
            XCTAssertTrue(window.firstResponder === text, "to the composer")
            if let passed { text.keyDown(with: passed) }
        }
        typeToFocus(key("x", 7, [], window: window, at: time + 1))
        XCTAssertEqual(text.string, "x")
        launcherView.isHidden = true
        typeToFocus(key("f", 3, [], window: window, at: time + 2))
        XCTAssertEqual(text.string, "xf")
        launcherView.isHidden = false
        composer.remove(element)
        typeToFocus(key("d", 2, [], window: window, at: time + 3))
        XCTAssertEqual(text.string, "xfd")
        composer.destroy()
    }
    /// #298 bug 3, issues/closed/20261009-macos-textarea-native-undo.md: ⌘Z, ⇧⌘Z and Edit › Undo and Redo undo typing in an Exact
    /// textarea, whose text view keeps its own history (host TextAreaMac.swift `textUndo`), as the composer and the prompt
    /// preview are. Since exact2 #327 the textarea answers `undo:` and `redo:` itself, so the clone's Edit › Undo sends it
    /// up the chain and Edit › Redo is the host's own item.
}

final class LauncherView: NSView {
    var keys: [String] = []
    override var acceptsFirstResponder: Bool { true }
    override func keyDown(with event: NSEvent) { keys.append(event.characters ?? "") }
}

let suite = XCTestSuite(forTestCaseClass: LauncherTests.self)
suite.run()
let run = suite.testRun!
print("Launcher tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.executionCount == 1 && run.totalFailureCount == 0 ? 0 : 1)
