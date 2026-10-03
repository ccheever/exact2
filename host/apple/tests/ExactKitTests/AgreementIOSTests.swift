#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// `layout agree` (LLP 1080.001 D2, D5): the walk over what the presenter
/// mounted against the kernel's frames. A clean tree reports nothing; each
/// kind fails when its fact is broken by hand, and a parked or leaving
/// tree's classification never excuses it from showing or taking input.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class AgreementIOSTests: XCTestCase {
    private var window: UIWindow!

    private func collections(_ rows: [(view: Int, root: Int)]) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": rows.map { ["view": $0.view, "root": $0.root, "epoch": 1] }, "correction": NSNull()]]]
    }
    /// NodePoolIOSTests' row: a box (`base`) holding a symbol image and a button.
    private func rowOps(_ base: Int) -> [[String: Any]] {
        [
            ["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)"], "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": base + 1, "kind": "image", "props": ["imageSource": "symbol:bookmark", "symbolName": "bookmark"], "style": ["font_size": 17.0]],
            ["op": "create", "id": base + 2, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Save"]],
            ["op": "children", "id": base, "ids": [base + 1, base + 2]],
            ["op": "frame", "id": base, "x": 0.0, "y": 0.0, "w": 300.0, "h": 44.0],
            ["op": "frame", "id": base + 1, "x": 8.0, "y": 10.0, "w": 20.0, "h": 24.0],
            ["op": "frame", "id": base + 2, "x": 200.0, "y": 0.0, "w": 44.0, "h": 44.0],
        ]
    }
    private func fixture() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([collections([(10, 10)]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + rowOps(10)
            + [["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        return p
    }
    /// The kernel's frames for the list and, when `row`, its row.
    private func kernel(row: Bool = true, complete: Bool = true) -> KernelFrames {
        var rows = [KernelFrame(id: 1, parent: nil, rect: CGRect(x: 0, y: 0, width: 300, height: 300), bits: 0)]
        if row {
            rows += [KernelFrame(id: 10, parent: 1, rect: CGRect(x: 0, y: 0, width: 300, height: 44), bits: 0),
                     KernelFrame(id: 11, parent: 10, rect: CGRect(x: 8, y: 10, width: 20, height: 24), bits: 0),
                     KernelFrame(id: 12, parent: 10, rect: CGRect(x: 200, y: 0, width: 44, height: 44), bits: 0)]
        }
        var k = KernelFrames(rows)
        k.complete = complete
        return k
    }
    private func agree(_ p: Presenter, _ k: KernelFrames) -> AgreementReport {
        let report = AgreementReport(tolerance: 1 / 3)
        p.inspectAgreement(roots: [(p.viewport, "viewport")], kernel: k, inFlight: false, report: report) { $0.convert($0.bounds, to: p.viewport) }
        return report
    }

    func testACleanTreeReportsNothingAndComparesItsFrames() {
        let p = fixture()
        let r = agree(p, kernel())
        XCTAssertEqual(r.counts, [:], "\(r.found)")
        XCTAssertTrue(r.incomplete.isEmpty)
        XCTAssertEqual(r.compared, 3, "the list, the glyph and the button by origin")
        XCTAssertEqual(r.sizeOnly, 1, "the row, in its list's content")
        XCTAssertGreaterThan(r.judged, 0)
        XCTAssertEqual(r.hiddenCompared, 4)
    }

    func testAParkedRowIsCountedAndMustStayHidden() throws {
        let p = fixture()
        let row = try XCTUnwrap(p.views[10])
        p.apply(wireBatch([collections([])] + [10, 11, 12].map { ["op": "destroy", "id": $0] } + [["op": "children", "id": 1, "ids": []]]))
        XCTAssertTrue(p.pool.isParked(row))
        var r = agree(p, kernel(row: false))
        XCTAssertEqual(r.counts, [:], "a parked tree is accounted for: \(r.found)")
        XCTAssertEqual(r.parkedRoots, 1)
        row.isHidden = false
        r = agree(p, kernel(row: false))
        XCTAssertEqual(r.counts["parked-visible"], 1, "classification never excuses a parked root that shows")
    }

    func testAForgottenViewInAListsScrollViewIsAStray() throws {
        let p = fixture()
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        scroll.flashScrollIndicators()
        scroll.layoutIfNeeded()
        XCTAssertEqual(agree(p, kernel()).counts["stray"], nil, "UIKit's own indicators are named, not strays")
        scroll.addSubview(UIView(frame: CGRect(x: 0, y: 44, width: 300, height: 44)))
        let r = agree(p, kernel())
        XCTAssertEqual(r.counts["stray"], 1)
        XCTAssertEqual(r.found.first?["under"] as? Int, 1)
    }

    func testAMovedViewDisagreesAndATransformedOneIsSkipped() throws {
        let p = fixture()
        try XCTUnwrap(p.views[11]).frame.origin.y += 2
        let button = try XCTUnwrap(p.views[12])
        button.transform = CGAffineTransform(scaleX: 0.95, y: 0.95)
        let r = agree(p, kernel())
        XCTAssertEqual(r.counts["frame"], 1, "\(r.found)")
        XCTAssertEqual(r.found.first?["id"] as? Int, 11)
        XCTAssertEqual(r.skipped["transformed"], 1, "press feedback's transform is skipped, not judged")
    }

    func testAViewHiddenByNoHostDisagrees() throws {
        let p = fixture()
        try XCTUnwrap(p.views[12]).isHidden = true
        let r = agree(p, kernel())
        XCTAssertEqual(r.counts["hidden"], 1)
        XCTAssertEqual(r.found.first?["id"] as? Int, 12)
    }

    func testALeavingTreeMustNotTakeInput() throws {
        let p = fixture()
        p.beginExit(12)
        let leaving = try XCTUnwrap(p.leaving[12]?.view)
        var k = kernel()
        k.frames[12] = nil; k.order.removeAll { $0 == 12 }
        var r = agree(p, k)
        XCTAssertEqual(r.counts, [:], "\(r.found)")
        XCTAssertEqual(r.leaving, 1)
        leaving.isUserInteractionEnabled = true
        r = agree(p, k)
        XCTAssertEqual(r.counts["leaving-interactive"], 1)
    }

    func testFramesCutAtTheirCapMakeTheWalkIncomplete() {
        let p = fixture()
        let r = agree(p, kernel(complete: false))
        XCTAssertEqual(r.incomplete, ["frames-cap"])
        XCTAssertEqual(r.json(limit: 32)["complete"] as? Bool, false)
    }
}
#endif
