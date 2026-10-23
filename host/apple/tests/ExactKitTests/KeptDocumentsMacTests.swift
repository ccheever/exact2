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
    /// again without it, in the order it had. AppKit applies a clear and a
    /// note a turn or more later (`noteWhenCleared`), so each step is waited
    /// for, up to 3 s.
    func testAForgottenDocumentLeavesOpenRecentAndTheRestKeepTheirOrder() throws {
        _ = NSApplication.shared
        let controller = NSDocumentController.shared
        func listed(_ want: [String]) -> [String] {
            let end = Date().addingTimeInterval(3)
            while controller.recentDocumentURLs.map(\.lastPathComponent) != want, Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
            return controller.recentDocumentURLs.map(\.lastPathComponent)
        }
        func clear() { if !controller.recentDocumentURLs.isEmpty { controller.clearRecentDocuments(nil); _ = listed([]) } }
        let saved = controller.recentDocumentURLs
        defer {
            clear()
            saved.reversed().forEach(controller.noteNewRecentDocumentURL)
        }
        let urls = try ["a.md", "b.md", "c.md"].map { try write($0, $0) }
        clear()
        urls.forEach(controller.noteNewRecentDocumentURL)
        let noted = listed(["c.md", "b.md", "a.md"])
        try XCTSkipUnless(noted == ["c.md", "b.md", "a.md"], "this test host keeps no recent documents: \(noted)")
        let b = try XCTUnwrap(KeptDocuments.identity(urls[1].path))
        KeptDocuments.forgotten(device: b.device, inode: b.inode)
        XCTAssertEqual(listed(["c.md", "a.md"]), ["c.md", "a.md"])
        // And it stays so: a clear that lands late takes nothing noted after it.
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(controller.recentDocumentURLs.map(\.lastPathComponent), ["c.md", "a.md"])
        // While a clear is on its way: a document the app notes is held, then noted newest, and the list the
        // library reconciles kept records against (`recent`) is the one to come.
        let c = try XCTUnwrap(KeptDocuments.identity(urls[2].path)), a = try XCTUnwrap(KeptDocuments.identity(urls[0].path))
        KeptDocuments.forgotten(device: c.device, inode: c.inode)
        XCTAssertNotNil(KeptDocuments.toNote, "the clear is on its way")
        KeptDocuments.note(urls[1])
        XCTAssertEqual(KeptDocuments.recent.map { KeptDocuments.file(URL(fileURLWithPath: $0)) }, [urls[1], urls[0]].map(KeptDocuments.file))
        XCTAssertEqual(listed(["b.md", "a.md"]), ["b.md", "a.md"])
        // A kept document reopened meanwhile moves to the top, past a newer one; one noted and forgotten meanwhile
        // goes. (A clear that never lands, or lands after the 2 s fallback, leaves c.md or loses b.md here.)
        KeptDocuments.note(urls[2])
        _ = listed(["c.md", "b.md", "a.md"])
        KeptDocuments.forgotten(device: c.device, inode: c.inode)
        KeptDocuments.note(urls[0])
        let d = try write("d.md", "d.md"), dIdentity = try XCTUnwrap(KeptDocuments.identity(d.path))
        KeptDocuments.note(d)
        KeptDocuments.forgotten(device: dIdentity.device, inode: dIdentity.inode)
        XCTAssertEqual(KeptDocuments.recent.map { KeptDocuments.file(URL(fileURLWithPath: $0)) }, [urls[0], urls[1]].map(KeptDocuments.file))
        XCTAssertEqual(listed(["a.md", "b.md"]), ["a.md", "b.md"])
        // Clear Menu while one is on its way: nothing comes back.
        KeptDocuments.forgotten(device: a.device, inode: a.inode)
        KeptDocuments.clear()
        XCTAssertEqual(listed([]), [])
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(controller.recentDocumentURLs, [])
    }

    /// A clear that lands after the 2 s fallback took what it noted: noted
    /// again once it lands (`watchLate`), and never past a real Clear Menu.
    /// The late clear is made by hand: the fallback runs on a list it
    /// assumes cleared, then the test clears it.
    func testWhatTheFallbackNotedComesBackAfterALateClearButNotAfterClearMenu() throws {
        _ = NSApplication.shared
        let controller = NSDocumentController.shared
        func listed(_ want: [String], within: TimeInterval = 3) -> [String] {
            let end = Date().addingTimeInterval(within)
            while controller.recentDocumentURLs.map(\.lastPathComponent) != want, Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
            return controller.recentDocumentURLs.map(\.lastPathComponent)
        }
        func clear() { if !controller.recentDocumentURLs.isEmpty { controller.clearRecentDocuments(nil); _ = listed([]) } }
        let saved = controller.recentDocumentURLs
        defer {
            KeptDocuments.late = nil
            clear()
            saved.reversed().forEach(controller.noteNewRecentDocumentURL)
        }
        let urls = try ["a.md", "b.md", "c.md"].map { try write($0, $0) }
        for clearMenu in [false, true] {
            clear()
            urls.forEach(controller.noteNewRecentDocumentURL)
            let noted = listed(["c.md", "b.md", "a.md"])
            try XCTSkipUnless(noted == ["c.md", "b.md", "a.md"], "this test host keeps no recent documents: \(noted)")
            // b.md forgotten, its clear not landed in 2 s: the fallback notes a.md and c.md over the list as it is.
            KeptDocuments.toNote = (Set(urls.map(KeptDocuments.file)), [urls[0], urls[2]])
            KeptDocuments.noteWhenCleared(tries: 0)
            XCTAssertEqual(listed(["c.md", "a.md", "b.md"]), ["c.md", "a.md", "b.md"])
            XCTAssertNotNil(KeptDocuments.late, "watched for a later clear")
            if clearMenu {
                KeptDocuments.clear()
                XCTAssertEqual(listed([]), [])
                RunLoop.main.run(until: Date().addingTimeInterval(1))
                XCTAssertEqual(controller.recentDocumentURLs, [], "a Clear Menu is not undone")
            } else {
                // The clear lands late: what the fallback noted comes back, in its order.
                controller.clearRecentDocuments(nil)
                XCTAssertEqual(listed(["c.md", "a.md"], within: 5), ["c.md", "a.md"])
                KeptDocuments.late = nil
            }
        }
    }

    /// While a late clear is watched for, a document forgotten stays gone,
    /// and one reopened meanwhile comes back with the rest. Driven by hand:
    /// the watch looks when the test asks first.
    func testALateWatchKeepsAForgottenDocumentGoneAndBringsBackOneReopened() throws {
        _ = NSApplication.shared
        let controller = NSDocumentController.shared
        func listed(_ want: [String]) -> [String] {
            let end = Date().addingTimeInterval(3)
            while controller.recentDocumentURLs.map(\.lastPathComponent) != want, Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
            return controller.recentDocumentURLs.map(\.lastPathComponent)
        }
        func clear() { if !controller.recentDocumentURLs.isEmpty { controller.clearRecentDocuments(nil); _ = listed([]) } }
        let saved = controller.recentDocumentURLs
        defer {
            KeptDocuments.late = nil
            clear()
            saved.reversed().forEach(controller.noteNewRecentDocumentURL)
        }
        let urls = try ["a.md", "b.md", "c.md"].map { try write($0, $0) }
        // Forgotten meanwhile: a.md, which the fallback had noted.
        clear()
        KeptDocuments.late = ([urls[0], urls[2]], KeptDocuments.generation)
        let a = try XCTUnwrap(KeptDocuments.identity(urls[0].path))
        KeptDocuments.forgotten(device: a.device, inode: a.inode)
        KeptDocuments.watchLate()
        let came = listed(["c.md"])
        try XCTSkipUnless(!came.isEmpty, "this test host keeps no recent documents: \(came)")
        XCTAssertEqual(came, ["c.md"])
        // Reopened meanwhile: c.md, one the old list had too.
        clear()
        KeptDocuments.late = ([urls[0], urls[2]], KeptDocuments.generation)
        KeptDocuments.note(urls[2])
        _ = listed(["c.md"])
        KeptDocuments.watchLate()
        XCTAssertEqual(listed(["c.md", "a.md"]), ["c.md", "a.md"])
        // The only document forgotten, reopened before its late clear lands: it comes back after it.
        clear()
        KeptDocuments.late = ([], KeptDocuments.generation)
        KeptDocuments.note(urls[1])
        _ = listed(["b.md"])
        KeptDocuments.watchLate()
        controller.clearRecentDocuments(nil)
        XCTAssertEqual(listed(["b.md"]), ["b.md"])
    }
}
#endif
