// @ref LLP 1126.001 WP14 — WP10's sizing and nested privacy edges.
import XCTest
import SwiftUI
import ExactCore
@testable import ExactSnapshot

#if canImport(WidgetKit)
extension ActivityDriverTests {
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
        let locked = try XCTUnwrap(image(view.environment(\.redactionReasons, .privacy), proposal: ProposedViewSize(width: 300, height: nil)))
        try write(clear, "nested-public-clear"); try write(locked, "nested-private-locked")
        let a = try pixels(clear), b = try pixels(locked)
        XCTAssertEqual(a.width, b.width); XCTAssertEqual(a.height, b.height)
        let publicRow = a.width * 4 * 20
        XCTAssertTrue(Array(a.data.prefix(publicRow)) == Array(b.data.prefix(publicRow)), "the public label stays visible")
        XCTAssertTrue(Array(a.data.dropFirst(publicRow)) != Array(b.data.dropFirst(publicRow)), "the private descendant redacts")
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
