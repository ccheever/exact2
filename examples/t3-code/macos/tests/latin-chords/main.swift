import AppKit
import XCTest

// T3LatinChords.swift: ⌘/⌃ chords under a non-Latin source match by physical key,
// and the wake answers only its topics; the pull request dialog's select on open is Contract's now
// (dialog-trap.contract FieldOpenSelect). (A still pointer's hover after the
// list re-renders is the host's own since exact2 #139.) Real AppKit views in a real window; compiled
// with every file in modules/apple.

private func tick(_ seconds: TimeInterval = 0.02) { let end = Date(timeIntervalSinceNow: seconds); while Date() < end { RunLoop.current.run(mode: .default, before: end) } }
class Flipped: NSView { override var isFlipped: Bool { true } }

/// Records the menu equivalents it is sent, as the app menu's declared chords do.
final class Target: NSObject { var fired: [String] = []; @objc func sidebar(_ sender: Any?) { fired.append("sidebar") }; @objc func palette(_ sender: Any?) { fired.append("palette") } }

final class LatinChordsTests: XCTestCase {
    let window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 420, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
    func key(_ characters: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command]) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
    }
    override func setUp() { _ = NSApplication.shared; window.isReleasedWhenClosed = false; window.contentView = Flipped(frame: NSRect(x: 0, y: 0, width: 420, height: 300)) }

    func testKoreanChordsTakeTheirPhysicalKey() {
        // Korean 2-Set: B is ㅠ, K is ㅏ, I is ㅑ (keybindings.ts adds KeyboardEvent.code's letter).
        XCTAssertEqual(T3LatinChords.latinChord(key("ㅠ", 11))?.charactersIgnoringModifiers, "b")
        XCTAssertEqual(T3LatinChords.latinChord(key("ㅏ", 40))?.charactersIgnoringModifiers, "k")
        XCTAssertEqual(T3LatinChords.latinChord(key("ㅑ", 34))?.keyCode, 34)
        XCTAssertEqual(T3LatinChords.latinChord(key("ㅠ", 11, [.command, .shift]))?.charactersIgnoringModifiers, "B")
        XCTAssertEqual(T3LatinChords.latinChord(key("ㅠ", 11, [.control]))?.charactersIgnoringModifiers, "b")
        XCTAssertEqual(T3LatinChords.latinChord(key("ㅠ", 11))?.modifierFlags.contains(.command), true)
        // A Cyrillic layout's chord too; the digit row keeps its digits.
        XCTAssertEqual(T3LatinChords.latinChord(key("и", 11))?.charactersIgnoringModifiers, "b")
        XCTAssertNil(T3LatinChords.latinChord(key("b", 11)), "a Latin key is left alone (no second letter on remapped layouts)")
        XCTAssertNil(T3LatinChords.latinChord(key("ü", 99)), "an unmapped key keeps its characters")
        XCTAssertNil(T3LatinChords.latinChord(key("ㅠ", 11, [])), "typing Hangul is not a chord")
        XCTAssertNil(T3LatinChords.latinChord(key("ㅠ", 11, [.option])), "Option alone is text input")
        XCTAssertNil(T3LatinChords.latinChord(key("ㅠ", 11), composing: true), "composition keeps its keys")
    }

    func testTheRemappedChordReachesAMenuEquivalent() {
        let target = Target(), menu = NSMenu(title: "Main"), top = NSMenuItem(title: "View", action: nil, keyEquivalent: ""), sub = NSMenu(title: "View")
        let sidebar = NSMenuItem(title: "Toggle Sidebar", action: #selector(Target.sidebar(_:)), keyEquivalent: "b"); sidebar.target = target
        let palette = NSMenuItem(title: "Command Palette", action: #selector(Target.palette(_:)), keyEquivalent: "k"); palette.target = target
        sub.addItem(sidebar); sub.addItem(palette); top.submenu = sub; menu.addItem(top)
        let chords = T3LatinChords(agent: true)
        XCTAssertFalse(menu.performKeyEquivalent(with: key("ㅠ", 11)), "AppKit alone does not match ⌘ㅠ to ⌘B")
        XCTAssertTrue(menu.performKeyEquivalent(with: chords.route(key("ㅠ", 11))))
        XCTAssertTrue(menu.performKeyEquivalent(with: chords.route(key("ㅏ", 40))))
        XCTAssertEqual(target.fired, ["sidebar", "palette"])
        // Inside a field that is composing, the chord is the input method's.
        let field = NSTextView(frame: NSRect(x: 0, y: 0, width: 200, height: 40)); window.contentView?.addSubview(field)
        window.makeFirstResponder(field); tick()
        field.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertEqual(chords.route(key("ㅠ", 11)).charactersIgnoringModifiers, "ㅠ")
        field.unmarkText()
        XCTAssertEqual(chords.route(key("ㅠ", 11)).charactersIgnoringModifiers, "b", "outside the composer and with nothing composing")
        chords.destroy()
    }

    func testWakeEmitsOnlyKnownTopics() {
        var topics: [String] = [], replies: [[String: Any]] = []
        T3LatinChords.wake(["op": "wake", "topic": "t3.notify", "generation": 3], changed: { topics.append($0) }) { replies.append($0) }
        T3LatinChords.wake(["op": "wake", "topic": "t3.events"], changed: { topics.append($0) }) { replies.append($0) }
        tick()
        XCTAssertEqual(topics, ["t3.notify"])
        XCTAssertEqual(replies.map { $0["ok"] as? Bool }, [false, true], "the refusal answers at once; the wake after it emits")
        XCTAssertEqual(replies.last?["generation"] as? Int, 3)
        // The pull request panel wakes its own resource.
        T3LatinChords.wake(["op": "wake", "topic": "t3.pr", "generation": 4], changed: { topics.append($0) }) { replies.append($0) }
        tick()
        XCTAssertEqual(topics, ["t3.notify", "t3.pr"])
        XCTAssertEqual(replies.last?["ok"] as? Bool, true)
    }
}

let suite = XCTestSuite(forTestCaseClass: LatinChordsTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.failureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
