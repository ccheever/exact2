import XCTest
@testable import ExactKit
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// CSS `box-shadow` (LLP 1064 D2, LLP 1077 D4): the list's outer shadows
/// cast through contentless layers in one container at the bottom of the
/// node's own, from the rounded border box (grown by a spread) and masked to
/// outside it; inset ones in a container masked to the padding box. A node
/// that clips its overflow clips its children in a box of their own, so the
/// shadow still falls outside it. AppKit runs
/// it under `bun host/apple/build.mjs --test`; UIKit as `BoxShadowIOSTests`.
class BoxShadowTests: XCTestCase {
    private let raised: NodeStyle = [
        "box_shadow": [["o": [0, 2], "b": 12, "s": 0, "c": [0, 0, 0, 51]]],
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
        let cast = try XCTUnwrap(caster.sublayers?.last)
        XCTAssertEqual(cast.shadowRadius, 6, "CSS's blur radius is twice the deviation")
        XCTAssertEqual(cast.shadowOffset, CGSize(width: 0, height: 2))
        XCTAssertEqual(cast.shadowColor?.alpha ?? 0, 0.2, accuracy: 0.002)
        XCTAssertNil(cast.contents)
        // The layer spans the reach; the path is the border box inside it.
        let path = try XCTUnwrap(cast.shadowPath)
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
        none["box_shadow"] = nil
        XCTAssertNil(node(none).shadowCaster)
        var clear = raised
        clear["box_shadow"] = [["o": [0, 2], "b": 12, "s": 0, "c": [0, 0, 0, 0]]]
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
        plain["box_shadow"] = nil
        n.applyStyle(plain)
        XCTAssertNil(n.clipBox)
        XCTAssertTrue(n.clipsToBounds)
        XCTAssertEqual(n.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3])
    }

    func testAListCastsEachShadowAndAnInsetOneInsideThePaddingBox() throws {
        var listed = raised
        listed["border_width"] = 2
        listed["box_shadow"] = [["o": [0, 2], "b": 12, "s": 4, "c": [0, 0, 0, 51]],
                                ["o": [0, 8], "b": 0, "s": 0, "c": [255, 0, 0, 255]],
                                ["o": [1, 1], "b": 2, "s": 3, "i": 1, "c": [0, 0, 255, 255]]]
        let n = node(listed)
        let caster = try XCTUnwrap(n.shadowCaster)
        XCTAssertEqual(caster.sublayers?.count, 2)
        // The first shadow is on top, its outline grown by the spread.
        let top = try XCTUnwrap(caster.sublayers?.last)
        let grown = caster.convert(try XCTUnwrap(top.shadowPath).boundingBox, to: host(n))
        XCTAssertEqual(grown.minX, -4, accuracy: 1e-9)
        XCTAssertEqual(grown.width, 108, accuracy: 1e-9)
        XCTAssertEqual(caster.sublayers?.first?.shadowOffset, CGSize(width: 0, height: 8))
        let inset = try XCTUnwrap(n.insetCaster)
        XCTAssertTrue(inset.superlayer === host(n))
        XCTAssertEqual(inset.sublayers?.count, 1)
        // Masked to the padding box: the border's band is outside it.
        let mask = try XCTUnwrap(inset.mask as? CAShapeLayer)
        XCTAssertFalse(mask.path!.contains(inset.convert(CGPoint(x: 1, y: 20), from: host(n))))
        XCTAssertTrue(mask.path!.contains(inset.convert(CGPoint(x: 50, y: 20), from: host(n))))
    }
}
