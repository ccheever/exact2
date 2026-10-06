import Foundation
import XCTest
@testable import ExactKit

/// Spooled image bytes outlive a killed process only until the next launch's
/// first spool: a directory whose lock no live process holds is removed, one
/// still locked, or whose lock cannot be read, is kept. Each test uses a root
/// of its own, never the real shared one.
final class RasterSpoolTests: XCTestCase {
    private let fm = FileManager.default
    private var root: URL!

    override func setUpWithError() throws {
        root = fm.temporaryDirectory.appendingPathComponent("raster-spool-test-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: root, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        // Restore what a test made unreadable, so the root can go.
        if let all = fm.enumerator(atPath: root.path) {
            for case let path as String in all { chmod(root.appendingPathComponent(path).path, 0o700) }
        }
        try? fm.removeItem(at: root)
    }

    private func spool(_ name: String, lock: Bool) throws -> URL {
        let d = root.appendingPathComponent(name, isDirectory: true)
        try fm.createDirectory(at: d, withIntermediateDirectories: true)
        try Data("bytes".utf8).write(to: d.appendingPathComponent("spooled"))
        if lock { fm.createFile(atPath: d.appendingPathComponent(".lock").path, contents: nil) }
        return d
    }

    /// Another open file description holding the lock, as a live process's would.
    private func hold(_ dir: URL) -> Int32 {
        let fd = open(dir.appendingPathComponent(".lock").path, O_RDWR | O_CLOEXEC)
        XCTAssertEqual(flock(fd, LOCK_EX | LOCK_NB), 0)
        return fd
    }

    func testASweepRemovesOnlyWhatNoLiveProcessOwns() throws {
        let unlocked = try spool("unlocked", lock: false)
        let dead = try spool("dead", lock: true)
        let live = try spool("live", lock: true)
        let owner = hold(live)
        defer { close(owner) }
        RasterSpool.sweep(root)
        XCTAssertFalse(fm.fileExists(atPath: unlocked.path), "no lock file: its creator died before taking one")
        XCTAssertFalse(fm.fileExists(atPath: dead.path), "a lock nobody holds")
        XCTAssertTrue(fm.fileExists(atPath: live.appendingPathComponent("spooled").path), "a held lock is a live owner")
    }

    /// A lock file that cannot be opened (here: unreadable) says nothing of
    /// its owner, so the directory stays (Astra's review, finding 1).
    func testALockThatCannotBeOpenedKeepsItsDirectory() throws {
        let held = try spool("held", lock: true)
        let owner = hold(held)
        defer { close(owner) }
        XCTAssertEqual(chmod(held.appendingPathComponent(".lock").path, 0), 0)
        RasterSpool.sweep(root)
        XCTAssertTrue(fm.fileExists(atPath: held.appendingPathComponent("spooled").path))
    }

    /// Establishing sweeps the dead and makes a directory whose lock is held:
    /// a second process's sweep (another descriptor) keeps it.
    func testEstablishingSweepsAndHoldsItsOwnLock() throws {
        let dead = try spool("dead", lock: true)
        let mine = try RasterSpool.establish(in: root)
        XCTAssertFalse(fm.fileExists(atPath: dead.path))
        XCTAssertEqual(mine.deletingLastPathComponent().standardizedFileURL, root.standardizedFileURL)
        RasterSpool.sweep(root)
        XCTAssertTrue(fm.fileExists(atPath: mine.path), "this process's lock is held")
        let fd = open(mine.appendingPathComponent(".lock").path, O_RDWR | O_CLOEXEC)
        defer { close(fd) }
        XCTAssertNotEqual(flock(fd, LOCK_EX | LOCK_NB), 0, "the lock is taken")
        XCTAssertNotEqual(fcntl(fd, F_GETFD) & FD_CLOEXEC, 0, "a sweep's descriptor is not inherited")
    }

    /// A symlink is never followed: an entry that links elsewhere is left
    /// alone, and a root that is a link is replaced by a real directory
    /// (Grok's review, finding 2).
    func testSymlinksAreNeverFollowed() throws {
        let elsewhere = fm.temporaryDirectory.appendingPathComponent("raster-elsewhere-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: elsewhere, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: elsewhere) }
        let victim = elsewhere.appendingPathComponent("victim", isDirectory: true)
        try fm.createDirectory(at: victim, withIntermediateDirectories: true)
        try fm.createSymbolicLink(at: root.appendingPathComponent("link"), withDestinationURL: victim)
        RasterSpool.sweep(root)
        XCTAssertTrue(fm.fileExists(atPath: victim.path), "an entry that is a link is not swept")
        let linkedRoot = root.appendingPathComponent("exact-raster")
        try fm.createSymbolicLink(at: linkedRoot, withDestinationURL: elsewhere)
        let mine = try RasterSpool.establish(in: linkedRoot)
        XCTAssertTrue(fm.fileExists(atPath: victim.path), "the link's target is not swept")
        var info = stat()
        XCTAssertEqual(lstat(linkedRoot.path, &info), 0)
        XCTAssertEqual(info.st_mode & S_IFMT, S_IFDIR, "the root is a real directory now")
        XCTAssertEqual(mine.deletingLastPathComponent().standardizedFileURL, linkedRoot.standardizedFileURL)
    }

    /// A root that cannot be made throws, and nothing is cached: the next
    /// spool tries again (finding 2).
    func testAFailedEstablishThrows() throws {
        let blocked = root.appendingPathComponent("file")
        try Data().write(to: blocked)
        XCTAssertThrowsError(try RasterSpool.establish(in: blocked.appendingPathComponent("exact-raster")))
    }

    /// The spools of builds before the directory, loose in an iOS app's own
    /// temporary directory: removed after a minute, nothing else.
    func testLooseSpoolsPastAMinuteGo() throws {
        func file(_ name: String, age: TimeInterval) throws -> URL {
            let f = root.appendingPathComponent(name)
            try Data("bytes".utf8).write(to: f)
            try fm.setAttributes([.modificationDate: Date().addingTimeInterval(-age)], ofItemAtPath: f.path)
            return f
        }
        let old = try file("exact-raster-\(UUID().uuidString)", age: 600)
        let fresh = try file("exact-raster-\(UUID().uuidString)", age: 5)
        let other = try file("someone-else", age: 600)
        RasterSpool.sweepLoose(root, now: Date())
        XCTAssertFalse(fm.fileExists(atPath: old.path))
        XCTAssertTrue(fm.fileExists(atPath: fresh.path))
        XCTAssertTrue(fm.fileExists(atPath: other.path))
    }
}
