import AppKit
import XCTest

// Compile with T3ComposerIntent.swift alone; real NSEvents through the actual
// NSApp local monitors, since the Exact agent's synthetic keys bypass them.
final class ComposerIntentTests: XCTestCase {
    private func key(_ flags: NSEvent.ModifierFlags, code: UInt16 = 36, repeating: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0, context: nil,
            characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: repeating, keyCode: code)!
    }
    private func flags(_ flags: NSEvent.ModifierFlags) -> NSEvent {
        NSEvent.keyEvent(with: .flagsChanged, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0, context: nil,
            characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 55)!
    }
    private func click(_ flags: NSEvent.ModifierFlags) -> NSEvent {
        NSEvent.mouseEvent(with: .leftMouseDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0, context: nil,
            eventNumber: 0, clickCount: 1, pressure: 1)!
    }

    func testReturnChordsAreRecordedOnceForTheNextSend() {
        let intent = T3ComposerIntent()
        intent.record(key(.command), at: 10)
        XCTAssertEqual(intent.take(at: 10.05)["modifiers"] as? String, "meta")
        XCTAssertTrue(intent.take().isEmpty, "a send never reuses an earlier press")
        intent.record(key([.command, .option]), at: 20)
        let background = intent.take(at: 20.2)
        XCTAssertEqual(background["modifiers"] as? String, "meta+alt")
        XCTAssertEqual(background["source"] as? String, "key")
        XCTAssertEqual((background["ageMs"] as? Double).map { ($0 * 10).rounded() / 10 }, 200)
        intent.record(key([], code: 0), at: 30)
        XCTAssertTrue(intent.take().isEmpty, "only Return records a key gesture")
        intent.record(key(.command, repeating: true), at: 31)
        XCTAssertTrue(intent.take().isEmpty, "a held Return does not record again")
        intent.record(click(.command), at: 40)
        XCTAssertEqual(intent.take(at: 40)["source"] as? String, "pointer")
        intent.destroy()
    }

    // r4-composer: ⌘↩ (a draft starts in the background) and ⌥⌘↩ (send and
    // start a new thread) are not the editor's plain-Return send. They pass
    // through to the send button's aria-keyshortcuts (Meta+Enter, Meta+Alt+Enter),
    // and the gesture recorded on the way resolves the intent through the
    // server's keybindings (composer-editor-intent.ts sendIntent).
    func testCommandReturnChordsPassToTheSendShortcutWithTheirGesture() {
        XCTAssertEqual(T3Composer.returnAction(key([]), hasMarkedText: false), .send)
        XCTAssertEqual(T3Composer.returnAction(key(.command), hasMarkedText: false), .passThrough)
        XCTAssertEqual(T3Composer.returnAction(key([.command, .option]), hasMarkedText: false), .passThrough)
        XCTAssertEqual(T3Composer.returnAction(key(.shift), hasMarkedText: false), .passThrough, "⇧↩ inserts a newline")
        XCTAssertEqual(T3Composer.returnAction(key([.command, .option]), hasMarkedText: true), .passThrough, "IME text commits first")
        XCTAssertEqual(T3Composer.returnAction(key(.command), hasMarkedText: false, shortcut: "mod-enter"), .send, "the ⌘↩ send setting sends from the editor")
        XCTAssertEqual(T3Composer.returnAction(key([.command, .option]), hasMarkedText: false, shortcut: "mod-enter"), .passThrough)
        XCTAssertEqual(T3Composer.returnAction(key([], code: 76), hasMarkedText: false), .send, "keypad Enter")

        _ = NSApplication.shared
        let intent = T3ComposerIntent()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        for (flags, expected) in [(NSEvent.ModifierFlags.command, "meta"), ([.command, .option], "meta+alt")] {
            let down = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36)!
            NSApp.sendEvent(down)
            let gesture = intent.take()
            XCTAssertEqual(gesture["modifiers"] as? String, expected)
            XCTAssertEqual(gesture["source"] as? String, "key")
            XCTAssertLessThan(gesture["ageMs"] as? Double ?? 9999, 2000, "fresh enough for sendIntent")
        }
        intent.destroy()
        window.close()
    }

    func testHeldCommandFlipsTheReportedStateAndNotifies() {
        var notified: [String] = []
        let intent = T3ComposerIntent { notified.append($0) }
        intent.track(flags(.command).modifierFlags)
        XCTAssertEqual(intent.status["modifiers"] as? String, "meta")
        intent.track(flags([.command, .shift]).modifierFlags)
        XCTAssertEqual(notified, ["t3.status"], "Shift alone does not change the label")
        intent.track([])
        XCTAssertEqual(intent.status["modifiers"] as? String, "")
        XCTAssertEqual(notified, ["t3.status", "t3.status"])
        intent.destroy()
    }

    func testTheLocalMonitorsSeeRealApplicationEvents() {
        _ = NSApplication.shared
        var notified = 0
        let intent = T3ComposerIntent { _ in notified += 1 }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let down = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36)!
        NSApp.sendEvent(down)
        XCTAssertEqual(intent.take()["modifiers"] as? String, "meta")
        XCTAssertEqual(intent.status["modifiers"] as? String, "meta")
        XCTAssertGreaterThan(notified, 0)
        intent.destroy()
        NSApp.sendEvent(down)
        XCTAssertTrue(intent.take().isEmpty, "a destroyed module records nothing")
        window.close()
    }
}

let suite = XCTestSuite(forTestCaseClass: ComposerIntentTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
