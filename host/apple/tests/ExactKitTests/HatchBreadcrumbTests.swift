import XCTest
@testable import ExactKit

/// The crash breadcrumb (LLP 1075.003.000.001 §4.4): what a process that
/// died inside a hatch call left is read by the next launch; a call that
/// returned leaves nothing; nesting past 8 is counted; each session's slot
/// is its own; a record torn by the death is passed over for the last whole
/// one; a live process's file is left alone.
final class HatchBreadcrumbTests: XCTestCase {
    private var directory = ""

    override func setUp() {
        directory = (NSTemporaryDirectory() as NSString).appendingPathComponent("exact-breadcrumb-tests-\(getpid())-\(UUID().uuidString)")
    }

    override func tearDown() { try? FileManager.default.removeItem(atPath: directory) }

    func testARunThatEndedInsideAHatchIsReadByTheNext() throws {
        let crumbs = try XCTUnwrap(HatchBreadcrumb(directory: directory, pid: 4001))
        let slot = try XCTUnwrap(crumbs.take(label: "main"))
        crumbs.push(slot, name: "route 4", moment: "built", incarnation: 1)
        crumbs.pop(slot)                                                   // returned: nothing is left open
        crumbs.push(slot, name: "navigation", moment: "built", incarnation: 1)
        crumbs.push(slot, name: "element avatar", moment: "changed", incarnation: 1)
        // Its process still holds the file: a reader leaves it alone.
        XCTAssertEqual(HatchBreadcrumb.read(directory: directory), [])
        XCTAssertTrue(FileManager.default.fileExists(atPath: crumbs.path))
        crumbs.close(delete: false)                                        // the process died here
        XCTAssertEqual(HatchBreadcrumb.read(directory: directory),
                       ["the last run ended while inside hatch element avatar (changed, call 3), session main; if it repeats, launch with EXACT_HATCHES=off"])
        XCTAssertFalse(FileManager.default.fileExists(atPath: crumbs.path), "read once, then deleted")
        XCTAssertEqual(HatchBreadcrumb.read(directory: directory), [])
    }

    func testACleanRunLeavesNothingAndNestingPastEightIsCounted() throws {
        let clean = try XCTUnwrap(HatchBreadcrumb(directory: directory, pid: 4002))
        let slot = try XCTUnwrap(clean.take(label: ""))
        clean.push(slot, name: "tabs", moment: "built", incarnation: 1)
        clean.pop(slot)
        clean.close(delete: false)
        XCTAssertEqual(HatchBreadcrumb.read(directory: directory), [], "no call was open")

        let deep = try XCTUnwrap(HatchBreadcrumb(directory: directory, pid: 4003))
        let s = try XCTUnwrap(deep.take(label: ""))
        for i in 1...11 { deep.push(s, name: "element e\(i)", moment: "built", incarnation: 2) }
        deep.pop(s)                                                        // the 11th returned
        deep.close(delete: false)
        XCTAssertEqual(HatchBreadcrumb.read(directory: directory),
                       ["the last run ended while inside hatch element e8 (built, call 8) (+2 nested); if it repeats, launch with EXACT_HATCHES=off"])
    }

    func testEachSessionsSlotIsItsOwnAndANinthRunsWithoutOne() throws {
        let crumbs = try XCTUnwrap(HatchBreadcrumb(directory: directory, pid: 4004))
        let a = try XCTUnwrap(crumbs.take(label: "a")), b = try XCTUnwrap(crumbs.take(label: "b"))
        crumbs.push(a, name: "element dot", moment: "built", incarnation: 1)
        crumbs.push(b, name: "toolbar", moment: "built", incarnation: 1)
        crumbs.free(a)                                                     // a ended cleanly; b's record stays
        let rest = (0..<7).compactMap { _ in crumbs.take(label: "") }
        XCTAssertEqual(rest.count, 7, "a's slot is free again")
        XCTAssertNil(crumbs.take(label: "ninth"))
        crumbs.close(delete: false)
        XCTAssertEqual(HatchBreadcrumb.read(directory: directory),
                       ["the last run ended while inside hatch toolbar (built, call 1), session b; if it repeats, launch with EXACT_HATCHES=off"])
    }

    func testATornRecordIsPassedOverForTheLastWholeOne() throws {
        let crumbs = try XCTUnwrap(HatchBreadcrumb(directory: directory, pid: 4005))
        let slot = try XCTUnwrap(crumbs.take(label: ""))
        crumbs.push(slot, name: "route home", moment: "built", incarnation: 1)
        crumbs.push(slot, name: "element badge", moment: "built", incarnation: 1)
        let path = crumbs.path
        crumbs.close(delete: false)
        // The second record's name, damaged after its checksum was written.
        let file = try XCTUnwrap(FileHandle(forUpdatingAtPath: path))
        try file.seek(toOffset: UInt64(HatchBreadcrumb.headerBytes + HatchBreadcrumb.recordBytes + 20))
        try file.write(contentsOf: Data([0xFF, 0xFF]))
        try file.close()
        XCTAssertEqual(HatchBreadcrumb.read(directory: directory),
                       ["the last run ended while inside hatch route home (built, call 1); if it repeats, launch with EXACT_HATCHES=off"])
    }
}
