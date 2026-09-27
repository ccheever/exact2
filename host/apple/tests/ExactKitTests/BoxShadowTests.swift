import XCTest
@testable import ExactKit
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// CSS `box-shadow` (LLP 1064 D2): the rows cast through one contentless
/// layer at the bottom of the node's own, from the rounded border box and
/// masked to outside it; a node that clips its overflow clips its children
/// in a box of their own, so the shadow still falls outside it. AppKit runs
/// it under `bun host/apple/build.mjs --test`; UIKit as `BoxShadowIOSTests`.
class BoxShadowTests: XCTestCase {
    private let raised: NodeStyle = [
        "shadow_color": [0, 0, 0, 51], "shadow_offset": [0, 2], "shadow_radius": 12, "shadow_opacity": 1,
        "border_radius": 12, "border_radius_top_left": 12, "border_radius_top_right": 12,
        "border_radius_bottom_right": 12, "border_radius_bottom_left": 12,
    ]

    private func node(_ style: NodeStyle, children: Int = 0) -> NodeView {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        let n = NodeView(id: 1, kind: "view", presenter: p)
        p.views[n.id] = n
        n.frame = CGRect(x: 0, y: 0, width: 100, height: 40)
        for i in 0..<children {
            let child = NodeView(id: UInt32(2 + i), kind: "view", presenter: p)
            p.views[child.id] = child
            n.addSubview(child)
        }
        n.applyStyle(style)
        #if os(iOS)
        n.layer.displayIfNeeded()
        #endif
        return n
    }

    private func host(_ n: NodeView) -> CALayer {
        #if os(iOS)
        return n.layer
        #else
        return n.layer!
        #endif
    }

    func testTheRowsCastFromTheRoundedBoxOutsideIt() throws {
        let n = node(raised)
        let caster = try XCTUnwrap(n.shadowCaster)
        XCTAssertTrue(caster.superlayer === host(n))
        XCTAssertEqual(caster.shadowRadius, 6, "CSS's blur radius is twice the deviation")
        XCTAssertEqual(caster.shadowOffset, CGSize(width: 0, height: 2))
        XCTAssertEqual(caster.shadowColor?.alpha ?? 0, 0.2, accuracy: 0.002)
        XCTAssertNil(caster.contents)
        // The layer spans the reach; the path is the border box inside it.
        let path = try XCTUnwrap(caster.shadowPath)
        let box = caster.convert(path.boundingBox, to: host(n))
        for (got, want) in zip([box.minX, box.minY, box.width, box.height], [0, 0, 100, 40] as [CGFloat]) {
            XCTAssertEqual(got, want, accuracy: 1e-9)
        }
        // The mask shows what is outside the box, never the inside.
        let mask = try XCTUnwrap(caster.mask as? CAShapeLayer)
        XCTAssertEqual(mask.fillRule, .evenOdd)
        let inside = caster.convert(CGPoint(x: 50, y: 20), from: host(n))
        let outside = caster.convert(CGPoint(x: 50, y: 50), from: host(n))
        XCTAssertFalse(mask.path!.contains(inside, using: .evenOdd))
        XCTAssertTrue(mask.path!.contains(outside, using: .evenOdd))
    }

    func testNoneAndATransparentColourCastNothing() {
        var none = raised
        none["shadow_opacity"] = 0
        XCTAssertNil(node(none).shadowCaster)
        var clear = raised
        clear["shadow_color"] = [0, 0, 0, 0]
        XCTAssertNil(node(clear).shadowCaster)
        // Removing the rows removes the layer.
        let n = node(raised)
        n.applyStyle([:])
        #if os(iOS)
        n.layer.setNeedsDisplay(); n.layer.displayIfNeeded()
        #endif
        XCTAssertNil(n.shadowCaster)
    }

    func testAClippingNodeClipsItsChildrenInABoxSoTheShadowFallsOutside() throws {
        var clipped = raised
        clipped["overflow_x"] = "hidden"; clipped["overflow_y"] = "hidden"
        let n = node(clipped, children: 2)
        let box = try XCTUnwrap(n.clipBox)
        XCTAssertFalse(n.clipsToBounds, "the node's own layer would clip its shadow")
        XCTAssertTrue(box.clipsToBounds)
        XCTAssertEqual(box.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3])
        XCTAssertTrue(n.container === box)
        #if os(iOS)
        XCTAssertEqual(box.layer.cornerRadius, 12)
        #else
        XCTAssertEqual(box.layer?.cornerRadius, 12)
        #endif
        XCTAssertNotNil(n.shadowCaster)
        // Without a shadow the node clips itself again, children back home.
        var plain = clipped
        plain["shadow_opacity"] = 0
        n.applyStyle(plain)
        XCTAssertNil(n.clipBox)
        XCTAssertTrue(n.clipsToBounds)
        XCTAssertEqual(n.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3])
    }
}
