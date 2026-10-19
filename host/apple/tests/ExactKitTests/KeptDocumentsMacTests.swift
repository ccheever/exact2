#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1069.010.000 D4 on a Mac: a kept document's security-scoped bookmark
/// leads to the object chosen, not to whatever is at its path. A move is
/// followed; a file another program renamed to a backup before writing its
/// own at the name is followed to the original; one deleted and replaced is
/// gone. The library checks the same identity again (`kept::restore`).
final class KeptDocumentsMacTests: XCTestCase {
    private var folder: URL!

    override func setUpWithError() throws {
        folder = try FileManager.default.url(for: .itemReplacementDirectory, in: .userDomainMask,
                                             appropriateFor: FileManager.default.temporaryDirectory, create: true)
            .resolvingSymlinksInPath()
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: folder)
    }

    private func write(_ name: String, _ text: String) throws -> URL {
        let url = folder.appendingPathComponent(name)
        try Data(text.utf8).write(to: url)
        return url
    }

    private func move(_ from: String, _ to: String) throws {
        try FileManager.default.moveItem(at: folder.appendingPathComponent(from), to: folder.appendingPathComponent(to))
    }

    private func kept(_ url: URL) throws -> (Data, UInt64, UInt64) {
        let bookmark = try XCTUnwrap(KeptDocuments.bookmark(url))
        let identity = try XCTUnwrap(KeptDocuments.identity(url.path))
        return (bookmark, identity.device, identity.inode)
    }

    func testABookmarkLeadsToTheObjectChosenAndNotToWhatIsAtItsPath() throws {
        let (chosen, device, inode) = try kept(try write("notes.md", "chosen"))
        let same = try XCTUnwrap(KeptDocuments.resolve(chosen, device: device, inode: inode))
        XCTAssertEqual(same.url.lastPathComponent, "notes.md")
        XCTAssertNil(same.bookmark, "a bookmark that still leads there is kept as it is")

        try move("notes.md", "renamed.md")
        let moved = try XCTUnwrap(KeptDocuments.resolve(chosen, device: device, inode: inode))
        XCTAssertEqual(moved.url.lastPathComponent, "renamed.md", "a move is followed")
        let refreshed = try XCTUnwrap(moved.bookmark, "a stale bookmark is made again")

        // Another program's save: the original renamed to a backup, its own
        // file written at the name. The name's new file does not inherit.
        try move("renamed.md", "renamed.md~")
        _ = try write("renamed.md", "another program's")
        let original = try XCTUnwrap(KeptDocuments.resolve(refreshed, device: device, inode: inode))
        XCTAssertEqual(original.url.lastPathComponent, "renamed.md~")
        XCTAssertEqual(try String(contentsOf: original.url, encoding: .utf8), "chosen")

        // The original deleted: what is at the name is still not it.
        try FileManager.default.removeItem(at: folder.appendingPathComponent("renamed.md~"))
        XCTAssertNil(KeptDocuments.resolve(refreshed, device: device, inode: inode))
    }

    func testAFolderIsKeptAsAFolder() throws {
        let notes = folder.appendingPathComponent("notes", isDirectory: true)
        try FileManager.default.createDirectory(at: notes, withIntermediateDirectories: true)
        let (chosen, device, inode) = try kept(notes)
        try move("notes", "moved")
        let moved = try XCTUnwrap(KeptDocuments.resolve(chosen, device: device, inode: inode))
        XCTAssertEqual(moved.url.lastPathComponent, "moved")
        var isFolder: ObjCBool = false
        XCTAssertTrue(FileManager.default.fileExists(atPath: moved.url.path, isDirectory: &isFolder) && isFolder.boolValue)
    }

    /// `forgetDocument` takes a document off Open Recent: the list is noted
    /// again without it, in the order it had.
    func testAForgottenDocumentLeavesOpenRecentAndTheRestKeepTheirOrder() throws {
        _ = NSApplication.shared
        let controller = NSDocumentController.shared
        let saved = controller.recentDocumentURLs
        defer {
            controller.clearRecentDocuments(nil)
            saved.reversed().forEach(controller.noteNewRecentDocumentURL)
        }
        let urls = try ["a.md", "b.md", "c.md"].map { try write($0, $0) }
        controller.clearRecentDocuments(nil)
        urls.forEach(controller.noteNewRecentDocumentURL)
        let listed = controller.recentDocumentURLs.map(\.lastPathComponent)
        try XCTSkipUnless(listed == ["c.md", "b.md", "a.md"], "this test host keeps no recent documents: \(listed)")
        let b = try XCTUnwrap(KeptDocuments.identity(urls[1].path))
        KeptDocuments.forgotten(device: b.device, inode: b.inode)
        XCTAssertEqual(controller.recentDocumentURLs.map(\.lastPathComponent), ["c.md", "a.md"])
    }
}
#endif
