#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class CollectionFillIOSTests: XCTestCase {
    private func snapshot(_ revision: Int, measured: Bool) -> [String: Any] {
        let rows: [[String: Any]] = (0..<4).map {
            ["view": 2 + $0, "root": 2 + $0, "epoch": 1, "index": $0, "size": 72, "measured": measured]
        }
        return ["op": "collections", "items": [["view": 1, "revision": revision, "scrollSequence": 0,
            "count": 100, "totalExtent": 7200, "pending": false, "rows": rows, "correction": NSNull()]]]
    }

    /// Four 72-point rows from index `0` (or the indices given), each at its
    /// place in a 100-row list under 20 points of padding.
    private func fixture(_ indices: [Int] = Array(0..<4)) throws -> (Presenter, UIWindow, UIScrollView) {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 200))
        window.addSubview(p.viewport)
        p.viewport.frame = window.bounds
        window.makeKeyAndVisible()
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll", "padding_top": 20]],
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0,
                "count": 100, "totalExtent": 7200, "pending": false,
                "rows": indices.enumerated().map { ["view": 2 + $0.offset, "root": 2 + $0.offset, "epoch": 1, "index": $0.element] },
                "correction": NSNull()]]],
        ]
        for (i, index) in indices.enumerated() {
            ops.append(["op": "create", "id": 2 + i, "kind": "view"])
            ops.append(["op": "frame", "id": 2 + i, "x": 10, "y": 20 + 72 * index, "w": 280, "h": 72])
        }
        ops += [["op": "children", "id": 1, "ids": indices.indices.map { 2 + $0 }], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
                ["op": "content", "id": 1, "w": 300, "h": 7220]]
        p.apply(wireBatch(ops))
        window.layoutIfNeeded()
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        return (p, window, scroll)
    }

    // Baseline coverage: callback rationing predates this lane.
    func testCoveredScrollDefersItsReportAndAGapRescuesOnlyWhatShows() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        let start = CACurrentMediaTime()
        for i in 1...1000 { scroll.contentOffset.y = CGFloat(1 + i % 90) }
        let milliseconds = (CACurrentMediaTime() - start) * 1000
        XCTAssertTrue(p.collections.covers(1))
        XCTAssertTrue(reports.isEmpty, "covered UIKit callbacks return before a row report")
        XCTAssertTrue(p.collections.fillPending.contains(1))
        p.collections.fillSlice(1, limit: 5)
        func limit(_ data: Data) -> UInt32 { data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 88, as: UInt32.self) } }
        XCTAssertEqual(reports.count, 1)
        XCTAssertEqual(limit(try XCTUnwrap(reports.last)), 5)
        scroll.contentOffset.y = 200
        XCTAssertFalse(p.collections.covers(1))
        XCTAssertEqual(reports.count, 2, "a visible gap is rescued in its UIKit callback")
        XCTAssertEqual(limit(try XCTUnwrap(reports.last)), 2, "visible rows plus two, rather than a whole window")
        print("D3 scroll callbacks=1000 covered_reports=0 ms=\(milliseconds) slice_limit=5 rescue_limit=2")
    }
    func testCoveredScrollPublishesItsDeferredReportThroughTheScheduledPump() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        p.collections.onFeedback = { _ in }
        p.collections.flush()
        let ready = expectation(description: "initial collection reports drained")
        DispatchQueue.main.async { ready.fulfill() }
        wait(for: [ready], timeout: 5)
        let reported = expectation(description: "covered scroll's pump report")
        var inScrollCallback = true
        var reports: [Data] = []
        p.collections.onFeedback = {
            XCTAssertFalse(inScrollCallback, "row reports run after UIKit's offset callback")
            reports.append($0)
            if reports.count == 1 { reported.fulfill() }
        }
        let started = CACurrentMediaTime()
        scroll.contentOffset.y = 40
        XCTAssertTrue(p.collections.covers(1))
        XCTAssertTrue(p.collections.fillPending.contains(1))
        XCTAssertTrue(reports.isEmpty)
        inScrollCallback = false
        wait(for: [reported], timeout: 5)
        XCTAssertEqual(reports.count, 1)
        XCTAssertFalse(p.collections.fillPending.contains(1), "the scheduled pump services the queued fill")
        let limit = try XCTUnwrap(reports.first).withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 88, as: UInt32.self) }
        XCTAssertGreaterThan(limit, 0)
        XCTAssertLessThan(limit, UInt32.max - 1, "the pump rations its slice")
        print("D3 scheduled scroll reports=\(reports.count) limit=\(limit) elapsed_ms=\((CACurrentMediaTime() - started) * 1000)")
    }

    /// Steps of a port or more each frame (a scroll indicator dragged)
    /// show rows no frame before showed: past the first, a jump's, each
    /// rescue is a glimpse, its rows bounded.
    func testCrossingAPortEachFrameRescuesAGlimpse() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        func word(_ data: Data, _ at: Int) -> UInt32 { data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: at, as: UInt32.self) } }
        scroll.contentOffset.y = 10
        reports.removeAll()
        scroll.contentOffset.y = 900
        XCTAssertEqual(word(try XCTUnwrap(reports.last), 92) & 16, 0, "a lone jump builds what shows")
        scroll.contentOffset.y = 1800
        let glimpse = try XCTUnwrap(reports.last)
        XCTAssertEqual(word(glimpse, 92) & 16, 16, "the next step across a port is a glimpse")
        XCTAssertEqual(word(glimpse, 88), 2, "two rows before a glimpse's cost is measured")
        XCTAssertTrue(p.collections.glimpses.contains(1))
    }

    func testScrollReportsOnlyNewRowMeasurements() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        func count(_ data: Data) -> UInt32 {
            data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 96, as: UInt32.self) }
        }
        p.collections.flush()
        XCTAssertEqual(count(try XCTUnwrap(reports.last)), 4)
        p.apply(wireBatch([snapshot(2, measured: true)]))
        p.collections.settle()
        reports.removeAll()
        let started = CACurrentMediaTime()
        for y in [30, 40, 50] {
            scroll.contentOffset.y = CGFloat(y)
            p.collections.fillSlice(1, limit: 0)
        }
        let sizes = reports.map(count)
        print("QUEUE 675 scroll reports=\(sizes.count) measurements=\(sizes) bytes=\(reports.reduce(0) { $0 + $1.count }) ms=\((CACurrentMediaTime() - started) * 1000)")
        XCTAssertEqual(sizes, [0, 0, 0])
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 10, "y": 20, "w": 280, "h": 73]]))
        p.collections.settle()
        XCTAssertEqual(count(try XCTUnwrap(reports.last)), 1, "a changed height is sent")
        p.apply(wireBatch([snapshot(3, measured: false)]))
        p.collections.settle()
        // Invalidation withdraws the acknowledgement, even if a row's
        // frame and epoch have not changed.
        p.collections.fillSlice(1, limit: 1)
        XCTAssertEqual(count(try XCTUnwrap(reports.last)), 4)
    }

    func testARejectedPinDoesNotAcknowledgeItsRowMeasurements() throws {
        let (p, window, _) = try fixture()
        defer { p.reset(); window.isHidden = true }
        p.apply(wireBatch([snapshot(1, measured: true)]))
        var storedHeight = 72.0, rejected = 0
        p.collections.onFeedback = { data in
            data.withUnsafeBytes { bytes in
                let pin = UInt32(littleEndian: bytes.loadUnaligned(fromByteOffset: 76, as: UInt32.self))
                // The runner rejects the whole report for an unpublished pin.
                if pin == 99 { rejected += 1; return }
                let count = UInt32(littleEndian: bytes.loadUnaligned(fromByteOffset: 96, as: UInt32.self))
                for i in 0..<Int(count) {
                    let at = 100 + i * 20
                    let id = UInt32(littleEndian: bytes.loadUnaligned(fromByteOffset: at, as: UInt32.self))
                    if id == 2 {
                        storedHeight = Double(bitPattern: UInt64(littleEndian: bytes.loadUnaligned(fromByteOffset: at + 12, as: UInt64.self)))
                    }
                }
            }
        }
        p.collections.flush()
        // Model a host view the runner does not own, inside a mounted row.
        p.apply(wireBatch([
            ["op": "create", "id": 99, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 2, "ids": [99]],
            ["op": "frame", "id": 2, "x": 10, "y": 20, "w": 280, "h": 73],
        ]))
        p.collections.pointer(99)
        p.collections.settle()
        XCTAssertEqual(rejected, 1)
        XCTAssertEqual(storedHeight, 72)
        p.collections.pointer(nil)
        p.collections.settle()
        XCTAssertEqual(storedHeight, 73, "the valid report retries the rejected height at the same revision")
        XCTAssertEqual(p.collections.entries[1]?.snapshot.revision, 1)
    }


    // @ref LLP 1010 §6.11 — a kept end row past a gap of unbuilt rows does
    // not end the travel: the rows the port will uncover are still owed.
    func testAKeptEndRowPastAGapStillLeadsTheSlice() throws {
        do {
            let (p, window, scroll) = try fixture([0, 1, 2, 3, 99])
            defer { p.reset(); window.isHidden = true }
            // The port reaches 260 of the run's 308; 200 points more need
            // ceil(152 / 72) = 3 rows, the kept last row notwithstanding.
            scroll.contentOffset.y = 60
            XCTAssertEqual(p.collections.rowsToCover(1, ahead: 200), 3)
            XCTAssertEqual(p.collections.rowsToCover(1, ahead: -200), 0, "the first item is built and joined")
        }
        do {
            let (p, window, scroll) = try fixture([0, 50, 51, 52, 53])
            defer { p.reset(); window.isHidden = true }
            // Rows 50–53 span 3620–3908; from 3680 back 200 points needs
            // ceil(140 / 72) = 2 rows, the kept first row notwithstanding.
            scroll.contentOffset.y = 3680
            XCTAssertEqual(p.collections.rowsToCover(1, ahead: -200), 2)
        }
    }
}
#endif
