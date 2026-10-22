import AppKit
import XCTest

// Paste as Text in the composer: its key (ChatComposer.tsx isPasteAsTextShortcut) arms the Contract's `paste`
// (composer.contract `richKey`), and the text view's `pasteAsPlainText:` runs inside that key's event, where a large
// paste stays inline (T3ComposerEditor.isPasteAsTextKey). The paste's files are Exact's `paste` event.
final class ComposerPasteTests: XCTestCase {
    private func key(_ ignoring: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0, context: nil,
                         characters: ignoring, charactersIgnoringModifiers: ignoring, isARepeat: false, keyCode: code)!
    }

    func testPasteAsTextIsTheKeyThePasteRunsIn() {
        XCTAssertTrue(T3ComposerEditor.isPasteAsTextKey(key("v", 9, [.command, .shift])))
        XCTAssertFalse(T3ComposerEditor.isPasteAsTextKey(key("V", 9, [.command, .shift, .option])), "⌥ leaves it out, as the reference's rule does")
        XCTAssertFalse(T3ComposerEditor.isPasteAsTextKey(key("ㅍ", 9, [.command, .shift])), "the key's character, as the reference's event.key")
        XCTAssertFalse(T3ComposerEditor.isPasteAsTextKey(key("v", 9, [.command])), "Paste")
        XCTAssertFalse(T3ComposerEditor.isPasteAsTextKey(key("v", 9, [.command, .shift, .control])))
        XCTAssertFalse(T3ComposerEditor.isPasteAsTextKey(nil), "chosen with the pointer, no key comes first")
    }
}
