import XCTest
import ExactCore
import CExact

final class HeadlessSessionTests: XCTestCase {
    func testPlanLayoutsWithoutAView() throws {
        let plan = try fixturePlan("HEADLESS")
        let session = try HeadlessSession(plan: plan, size: CGSize(width: 200, height: 400),
                                          facts: SessionFacts(displayScale: 2, preferences: 0))
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
        XCTAssertEqual(label.contentBox.width, 180)
        XCTAssertFalse(label.hasComputedLines)
        XCTAssertFalse(try node("padded", in: session.snapshot()).hasComputedLines)
        XCTAssertEqual(label.lines.map(\.text).joined(), "alpha beta gamma")
        XCTAssertEqual(label.lines.count, 1)
        XCTAssertTrue(try node("label", in: session.snapshot()).hasComputedLines)
        let padded = try node("padded", in: initial)
        XCTAssertEqual(padded.contentBox.origin, CGPoint(x: 8, y: 8))
        XCTAssertEqual(padded.lines.first?.origin.y, 16)
        XCTAssertEqual(try node("hairline", in: initial).frame.width, SessionFacts.defaultRootFontSize * 2)
        XCTAssertEqual(try node("hairline", in: initial).frame.height, 0.5)
        XCTAssertEqual(try node("body", in: initial).style["font_size"]?.number, Double(SessionFacts.defaultRootFontSize))

        let narrow = try session.resize(to: CGSize(width: 90, height: 180))
        let wrapped = try node("label", in: narrow)
        XCTAssertEqual(narrow.size.width, 90)
        XCTAssertEqual(wrapped.contentBox.width, 70)
        XCTAssertEqual(wrapped.lines.map(\.text).joined(), "alpha beta gamma")
        XCTAssertGreaterThan(wrapped.lines.count, label.lines.count)
        XCTAssertEqual(wrapped.frame.height, CGFloat(wrapped.lines.count) * 20)
        XCTAssertEqual(wrapped.lines.map(\.origin.y), [16, 36, 56])

        let refreshed = try session.refresh()
        XCTAssertEqual(try node("label", in: refreshed).lines.map(\.text), wrapped.lines.map(\.text))
        let restored = try session.resize(to: initial.size)
        XCTAssertEqual(try node("label", in: restored).frame, label.frame)
        XCTAssertEqual(try node("label", in: restored).lines.map(\.text), label.lines.map(\.text))

        let changed = try session.press(box.id)
        let changedLabel = try node("label", in: changed)
        XCTAssertEqual(changedLabel.lines.map(\.text).joined(), "alpha delta epsilon zeta")
        XCTAssertEqual(label.lines.map(\.text).joined(), "alpha beta gamma", "an old snapshot keeps its lines")
        XCTAssertEqual(try node("box", in: changed).frame, box.frame)
    }
    func testMarkdownIsRefusedAtBootIncludingUnmeasuredNodes() throws {
        for (key, name) in [("HEADLESS_MARKDOWN", "markdown-body"), ("HEADLESS_HIDDEN_MARKDOWN", "hidden-markdown")] {
            let plan = try fixturePlan(key)
            XCTAssertThrowsError(try HeadlessSession(plan: plan, size: CGSize(width: 200, height: 400),
                facts: SessionFacts(displayScale: 1, preferences: 0))) { error in
                XCTAssertTrue(String(describing: error).contains(name), "\(error)")
                XCTAssertTrue(String(describing: error).contains("TextPresentation"), "\(error)")
            }
        }
    }

    func testPackageCompositionCanSupplyMarkdownWithoutAView() throws {
        let adapter = MarkdownExpansion()
        let session = try HeadlessSession(plan: fixturePlan("HEADLESS_MARKDOWN"), size: CGSize(width: 200, height: 400),
            facts: SessionFacts(displayScale: 1, preferences: 0), assets: AssetResolver(root: Bundle.main.bundleURL), presentation: adapter)
        defer { session.close() }
        let node = try XCTUnwrap(session.snapshot().nodes.values.first { $0.props["testId"] == "markdown-body" })
        XCTAssertEqual(node.lines.map(\.text).joined(), "expanded")
        XCTAssertGreaterThan(adapter.expansions, 1, "both measurement and snapshot use the installed expansion")
    }

}


private final class MarkdownExpansion: TextPresentation {
    var expansions = 0
    func reset() {}
    func checkpoint() -> () -> Void { {} }
    func controlFont(_ engine: TextEngine, size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont? { nil }
    func measure(_ engine: TextEngine, _ request: ExactMeasureRequest) -> ExactMetrics? { nil }
    func markdown(_ engine: TextEngine, source: Run) -> [Run]? {
        expansions += 1
        var run = source; run.text = source.text.replacingOccurrences(of: "**", with: ""); return [run]
    }
    func attributes(_ run: Run, color: [Double], attributes: inout [NSAttributedString.Key: Any]) {}
}
