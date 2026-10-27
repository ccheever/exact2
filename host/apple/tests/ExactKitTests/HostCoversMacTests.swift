// @ref LLP 1115.002 D8.1 — covers on the Mac: a box a native presentation
// stands for leaves layout and comes back when the cover is withdrawn, and
// the kernel is told what stands after the batch, never a projection that
// was taken back inside it.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class HostCoversMacTests: XCTestCase {
    /// A report made inside a callback the runtime waits on is told after the
    /// callback returns, and recorded as delivered only then.
    func testAReportWaitsForTheServedCallbackToReturn() {
        _ = NSApplication.shared
        let p = Presenter()
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view"], ["op": "roots", "ids": [1]]]))
        for covered in [true, false] {
            let delivered = expectation(description: "the current cover reaches the runtime outside its callback")
            p.onCovers = { changes, _ in
                XCTAssertFalse(Owner.shared.isServingMain)
                XCTAssertEqual(changes.map { "\($0.0):\($0.1 == .whole ? "whole" : "none")" }, [covered ? "1:whole" : "1:none"])
                delivered.fulfill()
            }
            Owner.shared.sync {
                Owner.shared.callMain { p.covers.report("test", covered ? [1: .whole] : [:]) }
            }
            wait(for: [delivered], timeout: 5)
            XCTAssertEqual(p.covers.delivered, covered ? [1: .whole] : [:])
        }
    }

    /// A real session over the kernel: a column of a 50-point bar and a box
    /// that takes the rest of a 300 by 400 page.
    func testABoxCoveredWholeLeavesLayoutAndComesBackWhenTheCoverIsWithdrawn() throws {
        _ = NSApplication.shared
        let dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../target/host-covers-test-" + UUID().uuidString).standardizedFileURL
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        component App
          view
            column testId="page" width=300 height=400
              row testId="bar" height=50 flex-shrink=0
                text "Bar"
              column testId="rest" flex=1 min-height=0
                text "Rest"

        """
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try source.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "host-covers")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: CGSize(width: 300, height: 400)).error)
        let p = session.presenter
        func node(_ testId: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == testId }) }
        let bar = try node("bar"), rest = try node("rest")
        XCTAssertEqual(rest.frame, CGRect(x: 0, y: 50, width: 300, height: 350), "as authored: under the bar")
        XCTAssertEqual(bar.frame.height, 50)

        p.covers.report("test", [bar.id: .whole])
        XCTAssertEqual(rest.frame, CGRect(x: 0, y: 0, width: 300, height: 400), "the bar is the platform's now: the rest takes the page")
        XCTAssertEqual(p.covers.delivered, [bar.id: .whole])

        p.covers.report("test", [:])
        XCTAssertEqual(rest.frame, CGRect(x: 0, y: 50, width: 300, height: 350), "the cover withdrawn: the bar is laid out again")
        XCTAssertEqual(bar.frame, CGRect(x: 0, y: 0, width: 300, height: 50))
        XCTAssertEqual(p.covers.delivered, [:])
    }

    /// What the kernel is told is what stands once the batch has been
    /// applied: a report made inside a batch waits for it, one taken back
    /// inside the same batch is never told, and the same report twice is
    /// told once. A box that left the tree is not uncovered by name.
    func testTheKernelHearsWhatStandsAfterTheBatch() throws {
        _ = NSApplication.shared
        let p = Presenter()
        var told: [[String]] = []
        p.onCovers = { covers, _ in told.append(covers.map { "\($0.0):\($0.1 == .whole ? "whole" : "none")" }) }
        var inBatch: (() -> Void)?
        // Called while a batch that changes the root's `viewportFit` is being applied.
        p.onViewportFit = { XCTAssertTrue(p.isApplying); inBatch?() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
        ]))
        var fit = 0
        func batch(_ work: @escaping () -> Void) {
            inBatch = work
            fit += 1
            p.apply(wireBatch([["op": "props", "id": 1, "set": ["viewportFit": "fit-\(fit)"]]]))
            inBatch = nil
        }
        // Projected, then not, in one batch: nothing happened, nothing is told.
        batch { p.covers.report("list", [2: .whole]); XCTAssertEqual(told, []); p.covers.report("list", [:]) }
        XCTAssertEqual(told, [])
        // Projected: told once the batch is applied, and once.
        batch { p.covers.report("list", [2: .whole]); XCTAssertEqual(told, [], "not inside the batch") }
        XCTAssertEqual(told, [["2:whole"]])
        p.covers.report("list", [2: .whole])
        XCTAssertEqual(told, [["2:whole"]])
        // Another presentation's box beside it; the first one's ends.
        p.covers.report("panel", [3: .whole])
        p.covers.report("list", [:])
        XCTAssertEqual(told, [["2:whole"], ["3:whole"], ["2:none"]])
        // A covered box destroyed: the kernel has forgotten it, and is not told.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]], ["op": "destroy", "id": 3]]))
        p.covers.report("panel", [:])
        XCTAssertEqual(told.count, 3)
        // A restart: the new runner knows nothing, so the same cover is told again.
        p.covers.report("list", [2: .whole])
        p.reset()
        p.apply(wireBatch([["op": "create", "id": 2, "kind": "view"], ["op": "roots", "ids": [2]]]))
        p.covers.report("list", [2: .whole])
        XCTAssertEqual(told.suffix(2), [["2:whole"], ["2:whole"]])
    }
}
#endif
