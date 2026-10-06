import AppKit
import XCTest

// thread-commands-and-keys: ⌥↑ (thread.editQueuedMessage) reaches the queued
// edit only with the caret collapsed at the very start (T3ComposerQueueKey.swift).

final class ComposerQueueKeyTests: XCTestCase {
    private func arrowUp(_ fixture: EditorFixture, _ flags: NSEvent.ModifierFlags = [.option], repeatKey: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags.union([.numericPad, .function]), timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "\u{F700}", charactersIgnoringModifiers: "\u{F700}", isARepeat: repeatKey, keyCode: 126)!
    }

    func testChordNamesFollowTheWebGrammar() {
        let fixture = EditorFixture()
        XCTAssertEqual(T3Composer.ariaChord(arrowUp(fixture)), "Alt+ArrowUp")
        XCTAssertEqual(T3Composer.ariaChord(arrowUp(fixture, [.option, .shift])), "Alt+Shift+ArrowUp")
        let letter = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command, .shift], timestamp: 0, windowNumber: fixture.window.windowNumber,
            context: nil, characters: "K", charactersIgnoringModifiers: "K", isARepeat: false, keyCode: 40)!
        XCTAssertEqual(T3Composer.ariaChord(letter), "Meta+Shift+k")
    }

    func testOnlyACaretAtTheStartEditsTheQueue() {
        let fixture = EditorFixture()
        fixture.key("queued-edit|Alt+ArrowUp", node: 7)
        fixture.type("first line\nsecond")
        // Mid-text: the key stays with the text view (it moves the caret).
        let midText = arrowUp(fixture)
        XCTAssertTrue(fixture.composer.handle(midText) === midText)
        XCTAssertEqual(pressedNodes, [])
        // A selection from the start is not a collapsed caret.
        fixture.editor.setSelectedRange(NSRange(location: 0, length: 3))
        XCTAssertNotNil(fixture.composer.handle(arrowUp(fixture)))
        fixture.editor.setSelectedRange(NSRange(location: 0, length: 0))
        XCTAssertNil(fixture.composer.handle(arrowUp(fixture)))
        XCTAssertEqual(pressedNodes, [7])
        // A held key is consumed and ignored.
        XCTAssertNil(fixture.composer.handle(arrowUp(fixture, repeatKey: true)))
        XCTAssertEqual(pressedNodes, [7])
        // Another chord, or another focus, passes.
        XCTAssertNotNil(fixture.composer.handle(arrowUp(fixture, [.option, .shift])))
        fixture.window.makeFirstResponder(nil)
        XCTAssertNotNil(fixture.composer.handle(arrowUp(fixture)))
        XCTAssertEqual(pressedNodes, [7])
    }

    func testAnEmptyComposerEditsAndNoButtonMeansNoEdit() {
        let fixture = EditorFixture()
        // Without the client's button (a draft, no queue, an edit already open) the key is the text view's.
        let bare = arrowUp(fixture)
        XCTAssertTrue(fixture.composer.handle(bare) === bare)
        fixture.key("queued-edit|Alt+ArrowUp", node: 9)
        XCTAssertNil(fixture.composer.handle(arrowUp(fixture)))
        XCTAssertEqual(pressedNodes, [9])
        // A rebinding names its own chord.
        let rebound = EditorFixture()
        rebound.key("queued-edit|Control+ArrowUp", node: 11)
        XCTAssertNotNil(rebound.composer.handle(arrowUp(rebound)))
        XCTAssertNil(rebound.composer.handle(arrowUp(rebound, [.control])))
        XCTAssertEqual(pressedNodes, [11])
    }

    func testCompositionKeepsTheKey() {
        let fixture = EditorFixture()
        fixture.key("queued-edit|Alt+ArrowUp", node: 7)
        fixture.editor.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 0, length: 0))
        fixture.editor.setSelectedRange(NSRange(location: 0, length: 0))
        XCTAssertTrue(fixture.editor.hasMarkedText())
        XCTAssertNotNil(fixture.composer.handle(arrowUp(fixture)))
        XCTAssertEqual(pressedNodes, [])
    }
}
