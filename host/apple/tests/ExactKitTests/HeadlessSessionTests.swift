import XCTest
import ExactCore

final class HeadlessSessionTests: XCTestCase {
    func testPlanLayoutsWithoutAView() throws {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_HEADLESS_PLAN"])
        let plan = try Data(contentsOf: URL(fileURLWithPath: path))
        let session = try HeadlessSession(plan: plan, size: CGSize(width: 160, height: 180))
        defer { session.close() }
        func node(_ id: String, in snapshot: HeadlessSession.Snapshot) throws -> HeadlessSession.Node {
            try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == id })
        }

        let initial = try session.snapshot()
        let root = try node("root", in: initial), box = try node("box", in: initial)
        let label = try node("label", in: initial)
        XCTAssertEqual(initial.roots, [root.id])
        XCTAssertEqual(box.frame, CGRect(x: 10, y: 10, width: 40, height: 20))
        XCTAssertEqual(label.frame.origin, CGPoint(x: 10, y: 36))
        XCTAssertEqual(label.contentBox.width, 140)
        XCTAssertEqual(label.lines.map(\.text).joined(), "alpha beta gamma")
        XCTAssertEqual(label.lines.count, 1)

        let narrow = try session.resize(to: CGSize(width: 90, height: 180))
        let wrapped = try node("label", in: narrow)
        XCTAssertEqual(narrow.size.width, 90)
        XCTAssertEqual(wrapped.contentBox.width, 70)
        XCTAssertEqual(wrapped.lines.map(\.text).joined(), "alpha beta gamma")
        XCTAssertGreaterThan(wrapped.lines.count, label.lines.count)
        XCTAssertEqual(wrapped.frame.height, CGFloat(wrapped.lines.count) * 20)
        XCTAssertEqual(wrapped.lines[1].origin.y - wrapped.lines[0].origin.y, 20, accuracy: 0.001)

        let refreshed = try session.refresh()
        XCTAssertEqual(try node("label", in: refreshed).lines.map(\.text), wrapped.lines.map(\.text))
        let restored = try session.resize(to: initial.size)
        XCTAssertEqual(try node("label", in: restored).frame, label.frame)
        XCTAssertEqual(try node("label", in: restored).lines.map(\.text), label.lines.map(\.text))

        let changed = try session.press(box.id)
        let changedLabel = try node("label", in: changed)
        XCTAssertEqual(changedLabel.lines.map(\.text).joined(), "alpha delta epsilon zeta")
        XCTAssertGreaterThan(changedLabel.frame.height, label.frame.height)
        XCTAssertEqual(try node("box", in: changed).frame, box.frame)
    }
}
