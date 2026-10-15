#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class GroupedButtonSizingIOSTests: GroupedListFixture {
    private func nativeButtonsPresenter(extraRows: Int = 0, intrinsic: (([(UInt32, CGSize?)]) -> Void)? = nil) -> Presenter {
        ExactGroupedLists.install()
        let p = Presenter()
        p.onIntrinsic = intrinsic
        host(p).model = { id in
            guard id == 1 else { return nil }
            return GroupedListModel(sections: [
                .init(view: 2, header: nil, footer: nil, rows: [Row(view: 10, custom: true), Row(view: 11, custom: true)]
                    + (0..<extraRows).map { Row(view: UInt32(30 + $0), custom: true) }),
            ])
        }
        p.buttonFace = { id in
            var face = ButtonFace()
            face.title = id == 10 ? "Direct" : id == 12 ? "Nested" : "Standalone"
            return face
        }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let create = { (id: Int, kind: String, props: [String: String]) -> [String: Any] in
            var style: [String: Any] = ["text_color": [0, 0, 0, 255]]
            if kind == "control" { style["appearance"] = "auto" }
            return ["op": "create", "id": id, "kind": kind, "props": props,
                    "handlers": kind == "control" ? ["press"] : [], "style": style]
        }
        let frame = { (id: Int, x: Double, y: Double, w: Double, h: Double) -> [String: Any] in
            ["op": "frame", "id": id, "x": x, "y": y, "w": w, "h": h]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped"], "style": ["overflow_y": "scroll"]],
            create(2, "view", ["semanticTag": "section"]), create(3, "view", [:]),
            create(10, "control", ["type": "button", "accessibilityRole": "button", "groupedRowSeparator": "true"]),
            create(11, "view", [:]),
            create(12, "control", ["type": "button", "accessibilityRole": "button"]),
            create(20, "control", ["type": "button", "accessibilityRole": "button"]),
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 11, "ids": [12]],
            ["op": "roots", "ids": [1, 20]],
            frame(1, 0, 0, 402, 874), frame(2, 0, 0, 402, Double(104 + extraRows * 52)),
            frame(3, 16, 0, 370, Double(104 + extraRows * 52)),
            frame(10, 0, 0, 370, 52), frame(11, 0, 52, 370, 52),
            frame(12, 16, 8, 120, 34), frame(20, 0, 740, 120, 34),
        ]
        for index in 0..<extraRows {
            ops.append(create(30 + index, "view", [:]))
            ops.append(frame(30 + index, 0, Double(104 + index * 52), 370, 52))
        }
        ops.append(["op": "children", "id": 3, "ids": [10, 11] + (0..<extraRows).map { 30 + $0 }])
        p.apply(wireBatch(ops))
        return p
    }

    private func groupedButtonRect(_ bounds: CGRect) -> CGRect {
        CGRect(x: bounds.minX + 16, y: bounds.minY, width: max(0, bounds.width - 16), height: bounds.height)
    }

    private func assertAlignmentRect(_ button: NativeButtonIOS, _ expected: CGRect,
                                     file: StaticString = #filePath, line: UInt = #line) {
        let actual = button.alignmentRect(forFrame: button.frame)
        XCTAssertEqual(actual.minX, expected.minX, accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(actual.minY, expected.minY, accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(actual.width, expected.width, accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(actual.height, expected.height, accuracy: 0.001, file: file, line: line)
    }

    func testNativeButtonInsetFollowsGroupedMembershipThroughProjection() throws {
        let p = nativeButtonsPresenter()
        let row = try XCTUnwrap(p.views[10])
        let native = try XCTUnwrap(p.controls.controls[10] as? NativeButtonIOS)
        let nested = try XCTUnwrap(p.controls.controls[12] as? NativeButtonIOS)
        let standalone = try XCTUnwrap(p.controls.controls[20] as? NativeButtonIOS)
        let projected = try cell(p, 10)
        XCTAssertTrue(row.superview === projected.contentView)
        XCTAssertEqual(row.frame, CGRect(x: 0, y: 0, width: 370, height: 52), "the authored row remains the full cell slot")
        XCTAssertEqual(projected.bounds.height, 52, accuracy: 0.5)
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        assertAlignmentRect(nested, try XCTUnwrap(p.views[12]).bounds)
        assertAlignmentRect(standalone, try XCTUnwrap(p.views[20]).bounds)
        XCTAssertEqual(try XCTUnwrap(p.views[11]).frame.size, CGSize(width: 370, height: 52))

        // Cell chrome changes independently of the semantic 16-point slot inset.
        projected.contentView.layoutMargins = UIEdgeInsets(top: 5, left: 22, bottom: 7, right: 26)
        projected.setNeedsLayout(); projected.layoutIfNeeded()
        host(p).sync(changed: []); p.controls.sync()
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        host(p).prepare()
        XCTAssertTrue(row.superview === p.views[3]?.container)
        p.controls.sync()
        assertAlignmentRect(native, groupedButtonRect(row.bounds))

        p.apply(wireBatch([["op": "props", "id": 10, "set": ["groupedRowSeparator": "false"]]]))
        assertAlignmentRect(native, groupedButtonRect(row.bounds), line: #line)
        p.apply(wireBatch([["op": "props", "id": 10, "clear": ["groupedRowSeparator"]]]))
        assertAlignmentRect(native, row.bounds, line: #line)
    }

    func testProjectedNativeButtonRowsKeepAuthoredSizeAndClampNarrowSlots() throws {
        let p = nativeButtonsPresenter()
        let row = try XCTUnwrap(p.views[10])
        let native = try XCTUnwrap(p.controls.controls[10] as? NativeButtonIOS)
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 250.0, "h": 72.0]]))
        let projected = try cell(p, 10)
        XCTAssertEqual(row.frame.size, CGSize(width: 250, height: 72))
        XCTAssertEqual(projected.bounds.height, 72, accuracy: 0.5)
        assertAlignmentRect(native, CGRect(x: 16, y: 0, width: 234, height: 72))

        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 12.0, "h": 10.0]]))
        _ = try cell(p, 10)
        XCTAssertEqual(row.frame.size, CGSize(width: 12, height: 10), "the projection does not resize the authored slot")
        assertAlignmentRect(native, CGRect(x: 16, y: 0, width: 0, height: 10))
    }

    private func drainProjectionTurns() {
        let done = expectation(description: "projection and trait refresh")
        DispatchQueue.main.async { DispatchQueue.main.async { done.fulfill() } }
        wait(for: [done], timeout: 2)
    }

    func testOffscreenNativeRowKeepsInsetAndNeverPublishesProvisionalIntrinsicSizes() throws {
        var reports: [(UInt32, CGSize?)] = []
        let p = nativeButtonsPresenter(extraRows: 30) { reports.append(contentsOf: $0) }
        let row = try XCTUnwrap(p.views[10])
        let native = try XCTUnwrap(p.controls.controls[10] as? NativeButtonIOS)
        _ = try cell(p, 10)
        let collection = try list(p).collection
        collection.setContentOffset(CGPoint(x: 0, y: 800), animated: false)
        collection.layoutIfNeeded()
        XCTAssertNil(try list(p).cell(10), "the regression requires the native row to be offscreen")
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["testId": "unrelated-change"]]]))
        XCTAssertNil(try list(p).cell(10))
        XCTAssertTrue(row.superview === p.views[3]?.container, "an offscreen row remains in its authored hierarchy")
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        XCTAssertEqual(row.bounds.height, 52)
        drainProjectionTurns()
        XCTAssertTrue(reports.isEmpty, "native measurements are synchronous; projection publishes no raw or slot sizes")

        let cache = ButtonMeasureCache()
        cache.configure(window.traitCollection, in: p.viewport)
        let face = p.controls.face(10)
        let regular = cache.measure(face, widthKind: 0, width: row.bounds.width - 16, traits: window.traitCollection)
        window.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
        window.layoutIfNeeded(); native.updateTraitsIfNeeded()
        p.controls.sync(); drainProjectionTurns()
        let large = cache.measure(face, widthKind: 0, width: row.bounds.width - 16, traits: window.traitCollection)
        XCTAssertGreaterThan(large.height, regular.height, "UIKit's real height-for-width answer grows with Dynamic Type")
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        XCTAssertEqual(row.bounds.height, 52, "the host waits for the kernel's measured frame")
        XCTAssertTrue(reports.isEmpty)

        // Apply the synchronous kernel answer while the row is still offscreen.
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 370.0, "h": Double(large.height)]]))
        XCTAssertNil(try list(p).cell(10))
        assertAlignmentRect(native, CGRect(x: 16, y: 0, width: 354, height: CGFloat(large.height)))
        collection.setContentOffset(.zero, animated: false); collection.layoutIfNeeded()
        let remounted = try cell(p, 10)
        XCTAssertTrue(row.superview === remounted.contentView)
        XCTAssertEqual(row.bounds.height, CGFloat(large.height), accuracy: 0.001)
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        p.apply(wireBatch([])); drainProjectionTurns()
        XCTAssertTrue(reports.isEmpty, "remounting cannot replace the kernel's answer with raw control dimensions")
    }

    func testACarriedRowKeepsItsAuthoredBorderAndUsesUIKitSeparator() throws {
        let p = presenter { self.model(custom: true) }
        let row = try XCTUnwrap(p.views[21])
        row.props["groupedRowSeparator"] = "true"
        row.applyStyle(["border_width_bottom": 4, "border_color_bottom": [255, 0, 0, 255]])
        p.apply(wireBatch([["op": "frame", "id": 21, "x": 0.0, "y": 52.0, "w": 370.0, "h": 64.0]]))
        row.applyBoxLayer()
        XCTAssertNil(row.groupedSeparatorRect, "UIKit owns the carried row's system separator")
        XCTAssertEqual(try cell(p, 21).bounds.height, 64, accuracy: 0.5)
        XCTAssertEqual(row.style["border_width_bottom"]?.number, 4)
    }
}
#endif
