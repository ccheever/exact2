import Foundation
import XCTest
@testable import ExactKit

/// Spooled image bytes outlive a killed process only until the next launch's
/// first spool: a directory whose lock no live process holds is removed, one
/// still locked or younger than the grace period is kept.
final class RasterSpoolTests: XCTestCase {
    func testASweepRemovesOnlyWhatNoLiveProcessOwns() throws {
        let fm = FileManager.default
        let root = fm.temporaryDirectory.appendingPathComponent("raster-spool-test-\(UUID().uuidString)", isDirectory: true)
        defer { try? fm.removeItem(at: root) }
        func dir(_ name: String, lock: Bool, age: TimeInterval) throws -> URL {
            let d = root.appendingPathComponent(name, isDirectory: true)
            try fm.createDirectory(at: d, withIntermediateDirectories: true)
            try Data("bytes".utf8).write(to: d.appendingPathComponent("spooled"))
            if lock { fm.createFile(atPath: d.appendingPathComponent(".lock").path, contents: nil) }
            try fm.setAttributes([.modificationDate: Date().addingTimeInterval(-age)], ofItemAtPath: d.path)
            return d
        }
        let unlocked = try dir("unlocked", lock: false, age: 600)
        let dead = try dir("dead", lock: true, age: 600)
        let live = try dir("live", lock: true, age: 600)
        let fresh = try dir("fresh", lock: false, age: 5)
        // Another open file description holding the lock, as a live process's would.
        let owner = open(live.appendingPathComponent(".lock").path, O_RDWR)
        XCTAssertEqual(flock(owner, LOCK_EX | LOCK_NB), 0)
        defer { close(owner) }
        RasterSpool.sweep(root, now: Date())
        XCTAssertFalse(fm.fileExists(atPath: unlocked.path), "no lock file and past the grace period")
        XCTAssertFalse(fm.fileExists(atPath: dead.path), "a lock nobody holds")
        XCTAssertTrue(fm.fileExists(atPath: live.appendingPathComponent("spooled").path), "a held lock is a live owner")
        XCTAssertTrue(fm.fileExists(atPath: fresh.path), "inside the grace period")
    }

    /// The spools of builds before the directory, loose in an iOS app's own
    /// temporary directory: removed past the grace period, nothing else.
    func testLooseSpoolsPastTheGracePeriodGo() throws {
        let fm = FileManager.default
        let tmp = fm.temporaryDirectory.appendingPathComponent("raster-loose-test-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: tmp) }
        func file(_ name: String, age: TimeInterval) throws -> URL {
            let f = tmp.appendingPathComponent(name)
            try Data("bytes".utf8).write(to: f)
            try fm.setAttributes([.modificationDate: Date().addingTimeInterval(-age)], ofItemAtPath: f.path)
            return f
        }
        let old = try file("exact-raster-\(UUID().uuidString)", age: 600)
        let fresh = try file("exact-raster-\(UUID().uuidString)", age: 5)
        let other = try file("someone-else", age: 600)
        RasterSpool.sweepLoose(tmp, now: Date())
        XCTAssertFalse(fm.fileExists(atPath: old.path))
        XCTAssertTrue(fm.fileExists(atPath: fresh.path))
        XCTAssertTrue(fm.fileExists(atPath: other.path))
    }

    func testThisProcessSpoolsInItsOwnLockedDirectory() throws {
        let file = RasterSpool.file()
        XCTAssertEqual(file.deletingLastPathComponent(), RasterSpool.directory)
        // Our own lock is held, so a sweep of the root (past the grace period
        // too) keeps this process's directory.
        let root = RasterSpool.directory.deletingLastPathComponent()
        RasterSpool.sweep(root, now: Date().addingTimeInterval(3600))
        XCTAssertTrue(FileManager.default.fileExists(atPath: RasterSpool.directory.path))
    }
}
