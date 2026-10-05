#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1013.000 D4.4 as amended (2026-10-05): a flying view that is not an
/// image keeps its own layout and is scaled whole to the shown width,
/// top-anchored, in a clip that is the shown box, as CSS draws a view
/// transition's snapshot.
final class FlightScaleIOSTests: XCTestCase {
    private struct Scene {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        let screen: UIView
        let thumb: NodeView, card: NodeView, text: NodeView
        init(cardIn parent: UIView? = nil) {
            screen = UIView(frame: window.bounds)
            window.addSubview(screen); window.makeKeyAndVisible()
            thumb = NodeView(id: 1, kind: "view", presenter: p)
            p.views[1] = thumb; screen.addSubview(thumb)
            thumb.frame = CGRect(x: 300, y: 600, width: 56, height: 84)
            thumb.applyStyle(["border_radius_top_left": 12, "border_radius_top_right": 12, "border_radius_bottom_left": 12, "border_radius_bottom_right": 12])
            card = NodeView(id: 2, kind: "view", presenter: p)
            p.views[2] = card; (parent ?? screen).addSubview(card)
            card.frame = CGRect(x: 0, y: 62, width: 402, height: 714)
            card.applyStyle(["border_width": 2, "border_color_top": .array([.number(255), .number(0), .number(0), .number(255)]), "border_radius_top_left": 40, "border_radius_top_right": 40, "border_radius_bottom_left": 40, "border_radius_bottom_right": 40])
            text = NodeView(id: 3, kind: "view", presenter: p)
            p.views[3] = text; card.container.addSubview(text)
            text.frame = CGRect(x: 100, y: 300, width: 200, height: 40)
        }
        func fly(from: UInt32 = 1, to: UInt32 = 2, _ progress: CGFloat) {
            var op = BatchOp(op: .flight, nodeID: to)
            op.payload["from"] = NSNumber(value: from)
            p.beginFlight(op)
            p.presentFlight(to, progress)
            p.flightsBatchApplied()
        }
    }

    func testTheScaleIsTheShownWidthOverTheLayouts() {
        XCTAssertEqual(FlightScale.of(shown: CGSize(width: 201, height: 300), layout: CGSize(width: 402, height: 714)), 0.5)
        XCTAssertNil(FlightScale.of(shown: .zero, layout: CGSize(width: 402, height: 714)))
        XCTAssertNil(FlightScale.of(shown: CGSize(width: 10, height: 10), layout: .zero))
    }

    /// A story card opening from its thumbnail: mid-flight its child is where
    /// the card's layout puts it, scaled with the card, inside a clip that is
    /// the shown box; the card keeps its own size, so its surface is drawn as
    /// laid out; landed, it is the card again.
    func testACardFliesScaledWholeInItsClipAndLandsAtItsLayout() throws {
        let scene = Scene(), card = scene.card, text = scene.text
        scene.fly(0.5)
        // Halfway from (300, 600, 56, 84) to (0, 62, 402, 714).
        let clip = try XCTUnwrap(card.superview as? FlightClip)
        let shown = clip.convert(clip.bounds, to: nil)
        XCTAssertEqual(shown, CGRect(x: 150, y: 331, width: 229, height: 399))
        XCTAssertTrue(clip.layer.masksToBounds)
        XCTAssertEqual(clip.layer.cornerRadius, (12 + 40) / 2, accuracy: 0.01, "the radius mixed in the shown box's units")
        XCTAssertEqual(card.bounds.size, CGSize(width: 402, height: 714), "its own size: its surface as laid out")
        let s = 229.0 / 402.0
        let drawn = text.convert(text.bounds, to: nil)
        XCTAssertEqual(drawn.minX, 150 + 100 * s, accuracy: 0.01, "scaled with the card")
        XCTAssertEqual(drawn.minY, 331 + 300 * s, accuracy: 0.01, "top-anchored")
        XCTAssertEqual(drawn.width, 200 * s, accuracy: 0.01)
        // Its surface (a red 2-pt border here) is drawn at its own size,
        // scaled, and cut by the clip: the bottom border is below the shown
        // box, not drawn at its edge.
        card.applyBoxLayer(); card.layoutIfNeeded()
        let border = card.layer.borderWidth > 0 ? card.layer : try XCTUnwrap(card.boxBorder, "the border's layer")
        XCTAssertEqual(border.bounds.size, CGSize(width: 402, height: 714), "the border at the full layout")
        XCTAssertLessThan(clip.bounds.height, border.bounds.height * s, "its bottom edge below the clip")
        // A style re-applying its transform (a new transform origin) keeps
        // the flight's scale.
        card.applyStyle(["border_width": 2, "transform_origin": .array([.number(0), .number(0)])])
        XCTAssertEqual(card.transform, CGAffineTransform(scaleX: s, y: s))
        // A layer anchored off its centre is placed the same.
        card.layer.anchorPoint = CGPoint(x: 0.1, y: 0.9)
        scene.p.presentFlight(2, 0.5); scene.p.flightsBatchApplied()
        let anchored = text.convert(text.bounds, to: nil)
        XCTAssertEqual(anchored.minX, 150 + 100 * s, accuracy: 0.01, "a non-centred anchor, placed the same")
        XCTAssertEqual(anchored.minY, 331 + 300 * s, accuracy: 0.01)
        scene.p.landFlight(try XCTUnwrap(scene.p.flights[2]))
        XCTAssertNil(clip.superview, "the clip goes")
        XCTAssertEqual(card.transform, .identity)
        XCTAssertEqual(card.convert(card.bounds, to: nil), CGRect(x: 0, y: 62, width: 402, height: 714))
        XCTAssertEqual(text.convert(text.bounds, to: nil), CGRect(x: 100, y: 362, width: 200, height: 40))
    }

    /// Its slot under an ancestor scaled by half: the layout is the slot's
    /// own size, not its shown one, so at the end the card is its own size
    /// scaled by half, as it will stand once landed.
    func testAScaledAncestorScalesTheLandingNotTheLayout() throws {
        let holder = UIView(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        let scene = Scene(cardIn: holder)
        scene.screen.addSubview(holder)
        holder.transform = CGAffineTransform(scaleX: 0.5, y: 0.5)
        scene.fly(1)
        XCTAssertEqual(scene.card.bounds.size, CGSize(width: 402, height: 714))
        XCTAssertEqual(scene.card.transform.a, 0.5, accuracy: 0.001)
        let clip = try XCTUnwrap(scene.card.superview as? FlightClip)
        XCTAssertEqual(clip.layer.cornerRadius, 20, accuracy: 0.01, "its 40 at half scale")
    }

    /// Two flights overlapping in the flight layer paint by their views'
    /// ranks: the clip stands in for its card among them.
    func testAClipRanksAsItsViewAmongFlights() throws {
        let scene = Scene()
        scene.card.setRank(6)
        let photoFrom = NodeView(id: 5, kind: "view", presenter: scene.p)
        scene.p.views[5] = photoFrom; scene.screen.addSubview(photoFrom)
        photoFrom.frame = CGRect(x: 0, y: 0, width: 50, height: 50)
        let photo = NodeView(id: 6, kind: "image", presenter: scene.p)
        scene.p.views[6] = photo; scene.screen.addSubview(photo)
        photo.frame = CGRect(x: 0, y: 100, width: 402, height: 300)
        photo.setRank(2)
        scene.fly(0.5)
        scene.fly(from: 5, to: 6, 0.5)
        let clip = try XCTUnwrap(scene.card.superview as? FlightClip)
        XCTAssertTrue(photo.superview === clip.superview, "both in the flight layer")
        XCTAssertGreaterThan(clip.layer.zPosition, photo.layer.zPosition, "the card's rank over the photo's")
        scene.card.setRank(1)
        XCTAssertLessThan(clip.layer.zPosition, photo.layer.zPosition, "and again when the card's rank changes")
    }

    /// A flight caught mid-way flies on from its clip, as shown: its rect and
    /// radius, not the scaled card's.
    func testAnInterruptedFlightFliesOnFromItsClip() throws {
        let scene = Scene()
        scene.fly(0.5)
        let clip = try XCTUnwrap(scene.card.superview as? FlightClip)
        let other = NodeView(id: 4, kind: "view", presenter: scene.p)
        scene.p.views[4] = other; scene.screen.addSubview(other)
        other.frame = CGRect(x: 10, y: 10, width: 100, height: 100)
        var op = BatchOp(op: .flight, nodeID: 4)
        op.payload["from"] = NSNumber(value: 2)
        scene.p.beginFlight(op)
        let source = try XCTUnwrap(scene.p.flights[4]?.source)
        XCTAssertEqual(source.rect, clip.convert(clip.bounds, to: nil))
        XCTAssertEqual(source.radius, 26, accuracy: 0.01)
    }

    /// Its place gone before it lands: it stands at its clip's top left,
    /// unscaled, at its own size, and the clip goes.
    func testALandingWithoutItsPlaceDropsTheClipAndTheScale() throws {
        let scene = Scene()
        scene.fly(0.5)
        let flight = try XCTUnwrap(scene.p.flights[2]), clip = try XCTUnwrap(flight.clip)
        flight.slot?.removeFromSuperview()
        scene.p.landFlight(flight)
        XCTAssertNil(clip.superview)
        XCTAssertEqual(scene.card.transform, .identity)
        XCTAssertNil(scene.card.flightLook)
        XCTAssertNotNil(scene.card.superview)
        XCTAssertEqual(scene.card.convert(scene.card.bounds, to: nil), CGRect(x: 150, y: 331, width: 402, height: 714))
    }
}
#endif
