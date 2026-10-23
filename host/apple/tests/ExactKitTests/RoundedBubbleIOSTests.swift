#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class RoundedBubbleIOSTests: XCTestCase {
    private let style: NodeStyle = ["background_color": [20, 110, 220, 255],
        "border_radius_top_left": 18, "border_radius_top_right": 18,
        "border_radius_bottom_right": 4, "border_radius_bottom_left": 18]

    func testAnHDRFillTakesTheInitialAndChangedDynamicRangeLimit() throws {
        guard #available(iOS 26, *) else { throw XCTSkip("preferredDynamicRange needs iOS 26") }
        let p = Presenter(), bubble = NodeView(id: 1, kind: "view", presenter: p)
        bubble.frame = CGRect(x: 0, y: 0, width: 280, height: 70)
        var look = style
        look["background_color"] = ["cs": [["s": "srgb-linear", "v": [4, 4, 4, 1]]], "c": [255, 255, 255, 255]]
        for (limit, range) in [("standard", CALayer.DynamicRange.standard), ("constrained", .constrainedHigh), ("no-limit", .high), ("standard", .standard)] {
            look["dynamic_range_limit"] = .string(limit)
            bubble.applyStyle(look); bubble.layer.displayIfNeeded()
            let fill = try XCTUnwrap(bubble.boxFill)
            XCTAssertEqual(fill.preferredDynamicRange, range, limit)
            XCTAssertNil(bubble.layer.contents)
        }
    }

    func testAChatTranscriptKeepsNoCPUBubbleBackingStores() {
        let p = Presenter()
        let before = footprint()
        var bubbles: [NodeView] = []
        for id in UInt32(1)...80 {
            let bubble = NodeView(id: id, kind: "view", presenter: p)
            p.views[id] = bubble
            p.root.addSubview(bubble)
            bubble.frame = CGRect(x: id.isMultiple(of: 2) ? 16 : 90, y: Int(id) * 80, width: 280, height: 70)
            bubble.applyStyle(style)
            bubble.layer.displayIfNeeded()
            bubbles.append(bubble)
        }
        CATransaction.flush()
        let backingStores = bubbles.filter { $0.layer.contents != nil }.count
        print("CHAT_BUBBLES count=80 backingStores=\(backingStores) footprintBefore=\(before) footprintAfter=\(footprint())")
        XCTAssertEqual(backingStores, 0, "uneven corners must not allocate one CPU bitmap per bubble")
        withExtendedLifetime(bubbles) {}
    }

    func testTheFillMaskResizesWithoutClippingOverflowAndClearsWithTheStyle() throws {
        let p = Presenter(), bubble = NodeView(id: 1, kind: "view", presenter: p)
        bubble.frame = CGRect(x: 0, y: 0, width: 280, height: 70)
        bubble.applyStyle(style)
        bubble.layer.displayIfNeeded()
        let fill = try XCTUnwrap(bubble.layer.sublayers?.first { $0.mask is CAShapeLayer })
        let mask = try XCTUnwrap(fill.mask as? CAShapeLayer)
        XCTAssertEqual(mask.path, bubble.roundedPath(in: bubble.bounds).cgPath)
        XCTAssertNil(bubble.layer.mask, "visible overflow stays outside the fill's mask")
        XCTAssertFalse(bubble.layer.masksToBounds)
        bubble.frame.size = CGSize(width: 100, height: 30)
        bubble.applyBoxLayer()
        XCTAssertEqual(fill.frame, bubble.bounds)
        XCTAssertEqual(mask.path, bubble.roundedPath(in: bubble.bounds).cgPath)
        XCTAssertNil(mask.animationKeys(), "the geometry changes without implicit animation")
        bubble.applyStyle([:])
        bubble.layer.displayIfNeeded()
        XCTAssertNil(fill.superlayer)
        XCTAssertNil(bubble.layer.contents)
    }

    func testAUniformBorderAndGradientFollowTheCornersWithoutABitmap() throws {
        let p = Presenter(), bubble = NodeView(id: 1, kind: "view", presenter: p)
        bubble.frame = CGRect(x: 0, y: 0, width: 280, height: 70)
        var look = style
        look["border_width"] = 2
        look["border_color_top"] = [255, 255, 255, 255]
        look["background_image"] = ["linear": 180, "stops": [0, 5, 82, 240, 255, 1, 44, 107, 237, 255]]
        bubble.applyStyle(look)
        bubble.layer.displayIfNeeded()
        XCTAssertNil(bubble.layer.contents)
        let border = try XCTUnwrap(bubble.boxBorder as? CAShapeLayer)
        let path = try XCTUnwrap(border.path)
        XCTAssertTrue(path.contains(CGPoint(x: 1, y: 35), using: .evenOdd))
        XCTAssertFalse(path.contains(CGPoint(x: 140, y: 35), using: .evenOdd))
        XCTAssertEqual((bubble.boxGradient?.mask as? CAShapeLayer)?.path, bubble.roundedPath(in: bubble.bounds).cgPath)
        let layers = try XCTUnwrap(bubble.layer.sublayers)
        XCTAssertTrue(layers.first === bubble.boxFill)
        XCTAssertTrue(layers.last === border)
    }

    private func footprint() -> UInt64 {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
        let status = withUnsafeMutablePointer(to: &info) { pointer in
            pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
            }
        }
        return status == KERN_SUCCESS ? info.phys_footprint : 0
    }
}
#endif
