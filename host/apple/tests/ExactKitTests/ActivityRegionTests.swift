// @ref LLP 1126.001 WP14 — WP10's sizing and nested privacy edges.
import XCTest
import SwiftUI
import ExactCore
@testable import ExactSnapshot

#if canImport(WidgetKit)
extension ActivityDriverTests {
    func testActivityDrawReusesTheMeasuredLayouts() throws {
        for (key, kind) in [("SNAPSHOT_DEPARTURE", "departure"), ("SNAPSHOT_SERVICE_ALERT", "service-alert")] {
            let render = try self.render(try Self.plan(key), kind: kind, state: nil)
            var measuring = 0, drawing = 0
            for placement in ActivityPlacement.allCases {
                let proposal = ProposedViewSize(width: 165, height: nil)
                let before = render.layoutCount
                _ = render.measure(placement, proposal: proposal)
                measuring += render.layoutCount - before
                let measured = render.layoutCount
                _ = try XCTUnwrap(image(SnapshotActivity.view(render, placement: placement, environment: environment(placement))
                    .frame(width: 165, alignment: .leading), proposal: proposal))
                drawing += render.layoutCount - measured
            }
            print("LAYOUTS \(kind): measurement=\(measuring) draw=\(drawing)")
            XCTAssertEqual(drawing, 0, "each GeometryReader must reuse the content-height layout that measurement computed")
        }
    }

    func testPlacementDrawUsesItsProposalAfterAnUnspecifiedProbe() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_EDGES"), kind: "edges", state: nil)
        let layout = PlacementLayout(render: render, placement: .minimal)
        let picture = try XCTUnwrap(image(ProbeAfterProposal(layout: layout) {
            GeometryReader { proxy in
                // The placement's GeometryReader draws from the viewport placeSubviews proposes.
                if let laid = try? render.layout(.minimal, width: proxy.size.width, height: proxy.size.height) {
                    (self.texts(laid) == ["Thin"] ? Color.blue : Color.red)
                }
            }
        }, proposal: ProposedViewSize(width: 80, height: nil)))
        try write(picture, "proposal-after-unspecified")
        let center = try color(picture, x: 20, y: 12)
        XCTAssertGreaterThan(center[2], center[0] + 100, "the actual 80 pt proposal draws Thin, even after an ideal-size probe")
    }

    func testMinimumDoesNotTurnIntrinsicOverflowIntoAnAuthoredWidth() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_EDGES"), kind: "edges", state: nil)
        let measured = render.measure(.compactLeading, proposal: ProposedViewSize(width: 80, height: nil))
        let laid = try XCTUnwrap(measured.laid)
        XCTAssertFalse(laid.authoredWidth)
        XCTAssertEqual(laid.minimumWidth, 20)
        XCTAssertGreaterThan(laid.size.width, 80)
        XCTAssertEqual(measured.size.width, 80)
        let belowMinimum = render.size(.compactLeading, proposal: ProposedViewSize(width: 10, height: nil))
        XCTAssertEqual(belowMinimum.width, 20)
    }

    func testPercentageWidthKeepsItsOriginalContainingBlockWhenTextWraps() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_EDGES"), kind: "edges", state: nil)
        let measured = render.measure(.compactTrailing, proposal: ProposedViewSize(width: 165, height: nil))
        let laid = try XCTUnwrap(measured.laid)
        XCTAssertTrue(ActivityRender.wraps(under: laid.root, in: laid.snapshot))
        XCTAssertEqual(measured.size.width, 165)
        XCTAssertEqual(laid.viewport.width, 165)
        XCTAssertEqual(laid.root.frame.width, 165)
    }

    func testEqualReportedSizesKeepTheLayoutForEachProposal() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_EDGES"), kind: "edges", state: nil)
        let wide = ProposedViewSize(width: 120, height: nil), thin = ProposedViewSize(width: 80, height: nil)
        XCTAssertEqual(render.size(.minimal, proposal: wide), render.size(.minimal, proposal: thin))
        XCTAssertEqual(texts(try XCTUnwrap(render.laidOut(.minimal, proposal: wide))), ["Wide"])
        XCTAssertEqual(texts(try XCTUnwrap(render.laidOut(.minimal, proposal: thin))), ["Thin"])
        // Keep the region's proposal fixed while ImageRenderer negotiates its output image.
        for (proposal, red) in [(wide, true), (thin, false), (wide, true)] {
            let picture = try XCTUnwrap(image(SnapshotActivity.view(render, placement: .minimal, environment: environment(.minimal))
                .frame(width: proposal.width, alignment: .leading).background(Color.white), proposal: proposal))
            let p = try pixels(picture)
            var redInk = 0, blueInk = 0
            for at in stride(from: 0, to: p.data.count, by: 4) {
                if Int(p.data[at]) - Int(p.data[at + 2]) > 64 { redInk += 1 }
                if Int(p.data[at + 2]) - Int(p.data[at]) > 64 { blueInk += 1 }
            }
            XCTAssertGreaterThan(red ? redInk : blueInk, 10)
            XCTAssertEqual(red ? blueInk : redInk, 0)
        }
    }

    func testIntrinsicOverflowClipsAtTheReportedRegion() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_EDGES"), kind: "edges", state: nil)
        let picture = try XCTUnwrap(image(SnapshotActivity.view(render, placement: .compactLeading, environment: environment(.compactLeading))
            .frame(width: 80, alignment: .leading).frame(width: 300, height: 40, alignment: .topLeading).background(Color.white),
            proposal: ProposedViewSize(width: 300, height: 40)))
        XCTAssertGreaterThan(try ink(picture, in: CGRect(x: 0, y: 0, width: 80, height: 40), background: [255,255,255]), 20)
        XCTAssertEqual(try ink(picture, in: CGRect(x: 80, y: 0, width: 220, height: 40), background: [255,255,255]), 0)
    }

    func testPrivateDescendantInsidePublicContentRedactsOnlyItsOwnSubtree() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_EDGES"), kind: "edges", state: nil)
        let view = SnapshotActivity.view(render, placement: .lockScreen, environment: environment(.lockScreen)).background(Color.white)
        let clear = try XCTUnwrap(image(view, proposal: ProposedViewSize(width: 300, height: nil)))
        let locked = try XCTUnwrap(image(view.redacted(reason: .privacy), proposal: ProposedViewSize(width: 300, height: nil)))
        try write(clear, "nested-public-clear"); try write(locked, "nested-private-locked")
        let a = try pixels(clear), b = try pixels(locked)
        XCTAssertEqual(a.width, b.width); XCTAssertEqual(a.height, b.height)
        let laid = try XCTUnwrap(render.laidOut(.lockScreen, proposal: ProposedViewSize(width: 300, height: nil)))
        func descendants(_ node: HeadlessSession.Node) -> [HeadlessSession.Node] {
            [node] + node.children.compactMap { laid.snapshot.nodes[$0] }.flatMap(descendants)
        }
        let lines = descendants(laid.root).filter { $0.kind == "text" && !$0.lines.isEmpty }
        for line in lines {
            let rect = rect(of: line, in: laid)
            let shown = try region(clear, rect), hidden = try region(locked, rect)
            XCTAssertGreaterThan(try ink(clear, in: rect, background: [255,255,255]), 20, "the comparison contains the label's own ink")
            XCTAssertEqual(shown == hidden, line.lines.first?.text == "Public label", "only the private line redacts: \(line.lines.map(\.text))")
        }
        XCTAssertEqual(lines.count, 2)
    }

    func testPrivateViewRedactsItsOwnBackground() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_EDGES"), kind: "edges", state: nil)
        let proposal = ProposedViewSize(width: 300, height: nil)
        let laid = try XCTUnwrap(render.laidOut(.lockScreen, proposal: proposal))
        let node = try XCTUnwrap(laid.snapshot.nodes.values.first { $0.props["testId"] == "private-paint" })
        let rect = rect(of: node, in: laid)
        let view = SnapshotActivity.view(render, placement: .lockScreen, environment: environment(.lockScreen)).background(Color.white)
        let clear = try XCTUnwrap(image(view, proposal: proposal))
        let locked = try XCTUnwrap(image(view.redacted(reason: .privacy), proposal: proposal))
        let shown = try region(clear, rect), hidden = try region(locked, rect)
        func red(_ bytes: [UInt8]) -> Int { stride(from: 0, to: bytes.count, by: 3).filter { Int(bytes[$0]) > Int(bytes[$0 + 2]) + 100 }.count }
        XCTAssertGreaterThan(red(shown), 500)
        XCTAssertEqual(red(hidden), 0, "the private view's red background must not survive privacy redaction")
    }

    private func rect(of node: HeadlessSession.Node, in laid: ActivityRender.Laid) -> CGRect {
        var result = node.frame, current = node.id
        while current != laid.root.id, let parent = laid.snapshot.nodes.values.first(where: { $0.children.contains(current) }) {
            result = result.offsetBy(dx: parent.frame.minX, dy: parent.frame.minY); current = parent.id
        }
        return result
    }

    private func region(_ image: CGImage, _ rect: CGRect) throws -> [UInt8] {
        let p = try pixels(image)
        let rows = max(0, Int(rect.minY))..<min(p.height, Int(rect.maxY.rounded(.up)))
        let columns = max(0, Int(rect.minX))..<min(p.width, Int(rect.maxX.rounded(.up)))
        var out: [UInt8] = []
        for y in rows {
            for x in columns {
                let at = (y * p.width + x) * 4
                out.append(contentsOf: p.data[at..<(at + 3)])
            }
        }
        return out
    }
}

private struct ProbeAfterProposal: Layout {
    let layout: PlacementLayout
    typealias Cache = PlacementLayout.Cache
    func makeCache(subviews: Subviews) -> Cache { layout.makeCache(subviews: subviews) }
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout Cache) -> CGSize {
        layout.sizeThatFits(proposal: proposal, subviews: subviews, cache: &cache)
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout Cache) {
        let actual = ProposedViewSize(width: 80, height: nil)
        _ = layout.sizeThatFits(proposal: actual, subviews: subviews, cache: &cache)
        _ = layout.sizeThatFits(proposal: .unspecified, subviews: subviews, cache: &cache)
        layout.placeSubviews(in: bounds, proposal: actual, subviews: subviews, cache: &cache)
    }
}
#endif
