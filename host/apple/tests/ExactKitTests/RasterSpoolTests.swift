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
    /// a second process's sweep (another descriptor) keeps it. The held
    /// descriptor itself is close-on-exec, so no exec'd child keeps it.
    func testEstablishingSweepsAndHoldsItsOwnLock() throws {
        let base = root.appendingPathComponent("exact-raster")
        let dead = try spool("exact-raster/dead", lock: true)
        let outside = try spool("outside", lock: true)
        let spool = RasterSpool(tmp: root)
        let mine = try spool.directory()
        XCTAssertEqual(mine.deletingLastPathComponent().standardizedFileURL, base.standardizedFileURL)
        XCTAssertFalse(fm.fileExists(atPath: dead.path), "the first spool sweeps an abandoned directory (Astra r3 finding 2)")
        XCTAssertTrue(fm.fileExists(atPath: outside.path), "outside the spool root: not swept")
        RasterSpool.sweep(base)
        XCTAssertTrue(fm.fileExists(atPath: mine.path), "this process's lock is held")
        let fd = open(mine.appendingPathComponent(".lock").path, O_RDWR | O_CLOEXEC)
        defer { close(fd) }
        XCTAssertNotEqual(flock(fd, LOCK_EX | LOCK_NB), 0, "the lock is taken")
        let held = try XCTUnwrap(spool.heldLock)
        XCTAssertNotEqual(fcntl(held, F_GETFD) & FD_CLOEXEC, 0, "the held descriptor is not inherited across exec")
        XCTAssertEqual(try spool.directory(), mine, "made once")
    }

    /// A spool root that cannot be made throws, nothing is kept, and once it
    /// can be made the next spool makes it (Astra's r2 finding 4).
    func testAFailedSpoolIsTriedAgain() throws {
        XCTAssertEqual(chmod(root.path, 0o500), 0)
        let spool = RasterSpool(tmp: root)
        XCTAssertThrowsError(try spool.directory())
        XCTAssertNil(spool.heldLock)
        XCTAssertEqual(chmod(root.path, 0o700), 0)
        let mine = try spool.directory()
        XCTAssertTrue(fm.fileExists(atPath: mine.path))
        XCTAssertNotNil(spool.heldLock)
    }

    /// Another process holding the namespace lock (a creator between its
    /// directory and its lock) makes a spool give up, without sweeping, after
    /// its bounded attempts, never wait; the next spool succeeds once it is
    /// released (Astra's r2 finding 2).
    func testAHeldNamespaceIsNotWaitedFor() throws {
        let base = root.appendingPathComponent("exact-raster")
        try fm.createDirectory(at: base, withIntermediateDirectories: true)
        let other = open(base.appendingPathComponent(".namespace.lock").path, O_RDWR | O_CREAT | O_CLOEXEC, 0o600)
        XCTAssertEqual(flock(other, LOCK_EX | LOCK_NB), 0)
        // The holder is mid-creation: its directory has no lock file yet.
        let creating = base.appendingPathComponent("creating", isDirectory: true)
        try fm.createDirectory(at: creating, withIntermediateDirectories: true)
        let spool = RasterSpool(tmp: root)
        let started = Date()
        XCTAssertThrowsError(try spool.directory())
        XCTAssertTrue(fm.fileExists(atPath: creating.path), "no sweep without the namespace lock (Grok r2)")
        XCTAssertLessThan(Date().timeIntervalSince(started), 5, "bounded, not a blocking wait")
        close(other)
        XCTAssertNoThrow(try spool.directory())
    }

    /// A symlink is never followed: an entry that links elsewhere is left
    /// alone; a root that is a link is unlinked and made real, while a real
    /// root (another launch's, just repaired) keeps its live spools (Grok r1
    /// finding 2, Astra r2 finding 1).
    func testSymlinksAreNeverFollowed() throws {
        let elsewhere = fm.temporaryDirectory.appendingPathComponent("raster-elsewhere-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: elsewhere, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: elsewhere) }
        let victim = elsewhere.appendingPathComponent("victim", isDirectory: true)
        try fm.createDirectory(at: victim, withIntermediateDirectories: true)
        try fm.createSymbolicLink(at: root.appendingPathComponent("link"), withDestinationURL: victim)
        RasterSpool.sweep(root)
        XCTAssertTrue(fm.fileExists(atPath: victim.path), "an entry that is a link is not swept")
        let linked = root.appendingPathComponent("exact-raster")
        try fm.createSymbolicLink(at: linked, withDestinationURL: elsewhere)
        let first = try RasterSpool.establish(in: linked)
        defer { close(first.lock) }
        XCTAssertTrue(fm.fileExists(atPath: victim.path), "the link's target is not swept")
        var info = stat()
        XCTAssertEqual(lstat(linked.path, &info), 0)
        XCTAssertEqual(info.st_mode & S_IFMT, S_IFDIR, "the root is a real directory now")
    }

    /// A link in the root's place that cannot be unlinked (its directory is
    /// read-only) is not followed: nothing is created in its target (Grok r3
    /// finding 1).
    func testALinkThatCannotBeUnlinkedIsNotFollowed() throws {
        let elsewhere = fm.temporaryDirectory.appendingPathComponent("raster-elsewhere-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: elsewhere, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: elsewhere) }
        let linked = root.appendingPathComponent("exact-raster")
        try fm.createSymbolicLink(at: linked, withDestinationURL: elsewhere)
        XCTAssertEqual(chmod(root.path, 0o555), 0)
        XCTAssertThrowsError(try RasterSpool.establish(in: linked))
        XCTAssertEqual(chmod(root.path, 0o700), 0)
        XCTAssertEqual(try fm.contentsOfDirectory(atPath: elsewhere.path), [], "no lock file or spool in the link's target")
    }

    /// Two launches both see the symlinked root; the second is paused there
    /// while the first repairs it and spools, then resumes its own repair:
    /// the first's live spool survives (Astra r2 finding 1, r3 finding 1).
    func testAConcurrentRepairKeepsTheOtherLaunchsSpool() throws {
        let elsewhere = fm.temporaryDirectory.appendingPathComponent("raster-elsewhere-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: elsewhere, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: elsewhere) }
        let linked = root.appendingPathComponent("exact-raster")
        try fm.createSymbolicLink(at: linked, withDestinationURL: elsewhere)
        var first: (url: URL, lock: Int32)?
        RasterSpool.sawSymlink = { [fm] in
            RasterSpool.sawSymlink = nil
            // The other launch, all the way through, while this one waits.
            first = try? RasterSpool.establish(in: linked)
            if let first { try? Data("bytes".utf8).write(to: first.url.appendingPathComponent("spooled")) }
            _ = fm
        }
        defer { RasterSpool.sawSymlink = nil }
        let second = try RasterSpool.establish(in: linked)
        defer { close(second.lock) }
        let made = try XCTUnwrap(first)
        defer { close(made.lock) }
        XCTAssertTrue(fm.fileExists(atPath: made.url.appendingPathComponent("spooled").path), "the first launch's live spool survives")
        XCTAssertNotEqual(made.url, second.url)
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
