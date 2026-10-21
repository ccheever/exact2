import AppKit
import XCTest

// What the module adds to a paste in the composer: Paste as Text (T3ComposerEditor.pasteAsText; ChatComposer.tsx
// isPasteAsTextShortcut and consumePasteAsTextShortcut) and ⌘V with image data alone, which AppKit's Paste refuses
// for a plain text view (T3ComposerEditor.pastesOnlyFiles). The paste's files are Exact's `paste` event and the
// Contract's (composer.contract `pasted`); its staging is TypeScript's (composer-attach-stage.ts).
final class ComposerPasteTests: XCTestCase {
    /// A text view that records its pastes, and whether a large paste would stay inline then.
    final class Recorder: NSTextView {
        var pastes: [Bool] = []
        weak var editor: T3ComposerEditor?
        override func paste(_ sender: Any?) { pastes.append(editor?.pastingAsText ?? false) }
    }

    private func png() -> Data {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 4, pixelsHigh: 4, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                   isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        return rep.representation(using: .png, properties: [:])!
    }

    func testPasteAsTextPressesItsKeyThenPastesOnTheNextTurnWithTheTextInline() {
        let fixture = EditorFixture()
        let recorder = Recorder(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
        recorder.isEditable = true
        recorder.editor = fixture.composer.editor
        fixture.window.contentView?.addSubview(recorder)
        let element = ExactElement(hatch: .t3Composer, id: "composer", node: 2, hatches: fixture.hooks)
        element.view = recorder; element.platform = recorder
        element.data = ExactData(["snapshot-owner": "owner-a", "owner": "field-a"])
        fixture.composer.install(element)
        fixture.key("paste-as-text", node: 41)
        var turns: [() -> Void] = []
        fixture.composer.editor.nextTurn = { turns.append($0) }
        let drain = { while !turns.isEmpty { turns.removeFirst()() } }
        fixture.composer.editor.pasteAsText(recorder)
        XCTAssertEqual(pressedNodes, [41], "the Contract hears Paste as Text first")
        XCTAssertEqual(recorder.pastes, [], "the key's press runs on the next turn, and the paste after it")
        drain()
        XCTAssertEqual(recorder.pastes, [true])
        XCTAssertFalse(fixture.composer.editor.pastingAsText, "set only while the text view's own paste runs")
        // Another text view's Paste as Text is its own.
        fixture.composer.editor.pasteAsText(fixture.editor)
        drain()
        XCTAssertEqual(recorder.pastes, [true])
    }

    func testOnlyImageDataWithNoTextIsAPasteAppKitWouldRefuse() {
        let board = NSPasteboard(name: NSPasteboard.Name("t3-paste-\(UUID().uuidString)"))
        defer { board.releaseGlobally() }
        board.clearContents(); board.setData(png(), forType: .png)
        XCTAssertTrue(T3ComposerEditor.pastesOnlyFiles(board))
        board.clearContents(); board.setData(NSImage(data: png())!.tiffRepresentation!, forType: .tiff)
        XCTAssertTrue(T3ComposerEditor.pastesOnlyFiles(board))
        // Text beside an image is AppKit's to enable (a Finder copy carries the file's name).
        board.clearContents(); board.setData(png(), forType: .png); board.setString("photo.png", forType: .string)
        XCTAssertFalse(T3ComposerEditor.pastesOnlyFiles(board))
        board.clearContents(); board.setString("hello", forType: .string)
        XCTAssertFalse(T3ComposerEditor.pastesOnlyFiles(board))
        board.clearContents()
        XCTAssertFalse(T3ComposerEditor.pastesOnlyFiles(board))
    }
}
