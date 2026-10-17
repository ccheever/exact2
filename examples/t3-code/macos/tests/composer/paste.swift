import AppKit
import XCTest

// A paste in the composer with files or an image on the pasteboard (T3ComposerEditor.swift's pasteClaim, pasteFiles,
// takePastedFiles and T3ComposerPasteHook; ChatComposer.tsx onComposerPaste with shouldHandleComposerAttachmentPaste).
// A private pasteboard stands in for the general one; `paste(_:)` is what ⌘V, Edit › Paste and the text menu send,
// `pasteAsPlainText(_:)` what ⇧⌘V and Edit › Paste as Text send.
final class ComposerPasteTests: XCTestCase {
    private var root: URL!
    private var board: NSPasteboard!

    override func setUp() {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-paste-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        board = NSPasteboard(name: NSPasteboard.Name("t3-paste-\(UUID().uuidString)"))
        board.clearContents()
    }
    override func tearDown() {
        board.releaseGlobally()
        try? FileManager.default.removeItem(at: root)
    }

    private func file(_ name: String, _ data: Data) -> URL {
        let url = root.appendingPathComponent(name)
        try? data.write(to: url)
        return url
    }
    private func png() -> Data {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 4, pixelsHigh: 4, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                   isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        return rep.representation(using: .png, properties: [:])!
    }

    func testAnImageOrAFileClaimsThePasteAndTextAloneDoesNot() {
        let notes = file("notes.md", Data("# Notes\n".utf8)), photo = file("photo.png", png())
        board.writeObjects([notes as NSURL])
        XCTAssertEqual(T3ComposerEditor.pasteClaim(board)?.urls, [notes], "a file alone is claimed")
        // Finder's copy puts the file's name beside its URL: still the file's paste.
        board.clearContents(); board.writeObjects([notes as NSURL]); board.setString("notes.md", forType: .string)
        XCTAssertEqual(T3ComposerEditor.pasteClaim(board)?.urls, [notes])
        // An app's own text beside a plain file stays a text paste ("preserves text paste when an application adds a synthetic generic file").
        board.clearContents(); board.writeObjects([notes as NSURL]); board.setString("Copied text", forType: .string)
        XCTAssertNil(T3ComposerEditor.pasteClaim(board))
        // An image claims the paste, text or not.
        board.clearContents(); board.writeObjects([photo as NSURL]); board.setString("Image caption", forType: .string)
        XCTAssertEqual(T3ComposerEditor.pasteClaim(board)?.urls, [photo])
        // A copied image with no file (a screenshot to the clipboard) is its PNG data.
        board.clearContents(); board.setData(png(), forType: .png)
        let claim = T3ComposerEditor.pasteClaim(board)
        XCTAssertEqual(claim?.urls, [])
        XCTAssertNotNil(claim?.image.flatMap { NSBitmapImageRep(data: $0) })
        // TIFF data (what AppKit copies an NSImage as) becomes PNG.
        board.clearContents(); board.setData(NSImage(data: png())!.tiffRepresentation!, forType: .tiff)
        XCTAssertEqual(T3ComposerEditor.pasteClaim(board)?.image?.starts(with: [0x89, 0x50, 0x4e, 0x47]), true)
        // Text alone, and a folder alone, are the text view's.
        board.clearContents(); board.setString("hello", forType: .string)
        XCTAssertNil(T3ComposerEditor.pasteClaim(board))
        board.clearContents(); board.writeObjects([root as NSURL])
        XCTAssertNil(T3ComposerEditor.pasteClaim(board))
    }

    func testAPasteHoldsTheFilesForTypeScriptAndPastesNoText() {
        let fixture = EditorFixture()
        fixture.composer.editor.pasteboard = { [board] in board! }
        fixture.composer.editor.pasteDirectory = root.appendingPathComponent("paste", isDirectory: true)
        fixture.type("look ")
        let notes = file("notes.md", Data("# Notes\n".utf8))
        board.writeObjects([notes as NSURL])
        fixture.topics.removeAll()
        fixture.editor.paste(nil)
        XCTAssertEqual(fixture.editor.string, "look ", "no path or name is pasted as text")
        XCTAssertEqual(fixture.composer.editor.pastedFiles.map(\.url), [notes])
        XCTAssertEqual(fixture.topics, ["t3.editor"], "TypeScript is told to sync")
        // A copied image is written as image.png, staged by the take and then removed.
        board.clearContents(); board.setData(png(), forType: .png)
        fixture.editor.paste(nil)
        let written = fixture.composer.editor.pastedFiles[1].url
        XCTAssertEqual(written.lastPathComponent, "image.png")
        XCTAssertTrue(FileManager.default.fileExists(atPath: written.path))
        let attach = T3ComposerAttach(dataRoot: root.appendingPathComponent("data", isDirectory: true), agent: false)
        let files = fixture.composer.editor.takePastedFiles { attach.stage($0, fileLimit: 1024) }
        XCTAssertEqual(files.map { $0["kind"] as? String }, ["file", "image"])
        XCTAssertEqual(files.map { $0["name"] as? String }, ["notes.md", "image.png"])
        XCTAssertNotNil(files[0]["id"] as? String, "the plain file is staged within the limit")
        XCTAssertFalse(FileManager.default.fileExists(atPath: written.deletingLastPathComponent().path), "the written image's folder goes")
        XCTAssertTrue(FileManager.default.fileExists(atPath: notes.path), "the pasted file itself stays")
        XCTAssertTrue(fixture.composer.editor.pastedFiles.isEmpty)
        XCTAssertEqual(fixture.composer.editor.takePastedFiles { attach.stage($0, fileLimit: 1024) }.count, 0, "taken once")
        // Text on the pasteboard is the text view's own paste (which reads the general pasteboard, left untouched here).
        board.clearContents(); board.setString("hello", forType: .string)
        fixture.editor.paste(nil)
        XCTAssertTrue(fixture.editor.string.hasPrefix("look "))
        XCTAssertTrue(fixture.composer.editor.pastedFiles.isEmpty)
        // The composer reads images, so AppKit enables Paste for an image alone (a plain-text view reads none).
        XCTAssertTrue(fixture.editor.readablePasteboardTypes.contains(.png))
        XCTAssertTrue(fixture.editor.readablePasteboardTypes.contains(.tiff))
        // Another text view of the same class keeps its own paste with files on the pasteboard, and its own types.
        let other = NSTextView(frame: NSRect(x: 0, y: 0, width: 100, height: 20))
        other.isEditable = true
        fixture.window.contentView?.addSubview(other)
        board.clearContents(); board.writeObjects([notes as NSURL])
        other.paste(nil)
        XCTAssertTrue(fixture.composer.editor.pastedFiles.isEmpty, "only the composer's paste attaches")
        XCTAssertFalse(other.readablePasteboardTypes.contains(.png))
    }

    /// The reference's Paste as Text still attaches the files on the clipboard, an image with no inline chip
    /// (onComposerPaste with `skipImageInlineChip`); before, ⇧⌘V pasted Finder's file names as text.
    func testPasteAsTextAttachesTheFilesAndMarksThemForNoImageChip() {
        let fixture = EditorFixture()
        fixture.composer.editor.pasteboard = { [board] in board! }
        fixture.composer.editor.pasteDirectory = root.appendingPathComponent("paste", isDirectory: true)
        fixture.type("look ")
        let photo = file("photo.png", png()), notes = file("notes.md", Data("# Notes\n".utf8))
        board.writeObjects([notes as NSURL, photo as NSURL]); board.setString("notes.md\nphoto.png", forType: .string)
        fixture.topics.removeAll()
        fixture.editor.pasteAsPlainText(nil)
        XCTAssertEqual(fixture.editor.string, "look ", "no name is pasted as text")
        XCTAssertEqual(fixture.composer.editor.pastedFiles.map(\.url), [notes, photo])
        XCTAssertEqual(fixture.composer.editor.pastedFiles.map(\.asText), [true, true])
        XCTAssertEqual(fixture.topics, ["t3.editor"])
        XCTAssertFalse(fixture.composer.editor.pastingAsText, "set only while the text view's own paste runs")
        // ⌘V of a copied image after it is a plain paste; the take says which paste each file came from.
        board.clearContents(); board.setData(png(), forType: .png)
        fixture.editor.paste(nil)
        XCTAssertEqual(fixture.composer.editor.pastedFiles.map(\.asText), [true, true, false])
        let attach = T3ComposerAttach(dataRoot: root.appendingPathComponent("data", isDirectory: true), agent: false)
        let files = fixture.composer.editor.takePastedFiles { attach.stage($0, fileLimit: 1024) }
        XCTAssertEqual(files.map { $0["asText"] as? Bool }, [true, true, nil])
        XCTAssertEqual(files.map { $0["kind"] as? String }, ["file", "image", "image"])
        // Another text view's Paste as Text is its own.
        let other = NSTextView(frame: NSRect(x: 0, y: 0, width: 100, height: 20))
        other.isEditable = true
        fixture.window.contentView?.addSubview(other)
        board.clearContents(); board.writeObjects([photo as NSURL])
        other.pasteAsPlainText(nil)
        XCTAssertTrue(fixture.composer.editor.pastedFiles.isEmpty)
    }
}
