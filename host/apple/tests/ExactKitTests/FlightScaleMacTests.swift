#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1013.000 D4.4 as amended (2026-10-05), on AppKit (`FlightScaleIOSTests`):
/// a flying view that is not an image keeps its own layout and is scaled
/// whole, its layer included, in a clip that is the shown box.
final class FlightScaleMacTests: XCTestCase {
    func testACardFliesScaledWholeInItsClipAndLandsAtItsLayout() throws {
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 402, height: 874), styleMask: [.borderless], backing: .buffered, defer: false)
        let content = try XCTUnwrap(window.contentView)
        content.wantsLayer = true
        let thumb = NodeView(id: 1, kind: "view", presenter: p)
        p.views[1] = thumb; content.addSubview(thumb)
        thumb.frame = NSRect(x: 300, y: 600, width: 56, height: 84)
        let card = NodeView(id: 2, kind: "view", presenter: p)
        p.views[2] = card; content.addSubview(card)
        card.frame = NSRect(x: 0, y: 62, width: 402, height: 714)
        card.applyStyle(["border_radius_top_left": 40, "border_radius_top_right": 40, "border_radius_bottom_left": 40, "border_radius_bottom_right": 40])
        let text = NodeView(id: 3, kind: "view", presenter: p)
        p.views[3] = text; card.container.addSubview(text)
        text.frame = NSRect(x: 100, y: 300, width: 200, height: 40)
        var op = BatchOp(op: .flight, nodeID: 2)
        op.payload["from"] = NSNumber(value: 1)
        p.beginFlight(op)
        p.presentFlight(2, 0.5)
        p.flightsBatchApplied()
        // Halfway from (300, 600, 56, 84) to (0, 62, 402, 714) in the
        // unflipped content view: from y 190 to 98 in the flipped flight layer.
        let clip = try XCTUnwrap(card.superview as? FlightClip)
        XCTAssertEqual(clip.frame, NSRect(x: 150, y: 144, width: 229, height: 399))
        XCTAssertEqual(clip.layer?.masksToBounds, true)
        XCTAssertEqual(clip.layer?.cornerRadius ?? 0, 20, accuracy: 0.01, "0 and 40 mixed")
        // AppKit's geometry knows nothing of a layer's scale: the card keeps
        // its own frame at the clip's top left, and its layer is scaled about
        // that origin, so what Core Animation draws is the card scaled whole.
        let s = 229.0 / 402.0
        let cardLayer = try XCTUnwrap(card.layer)
        XCTAssertEqual(card.frame, NSRect(x: 0, y: 0, width: 402, height: 714), "its own size: its surface as laid out")
        XCTAssertEqual(cardLayer.anchorPoint, .zero)
        XCTAssertEqual(cardLayer.affineTransform(), CGAffineTransform(scaleX: s, y: s))
        XCTAssertEqual(text.frame, NSRect(x: 100, y: 300, width: 200, height: 40), "its child at its laid-out place, inside the scale")
        // AppKit laying the card out again keeps the flight's scale.
        card.needsLayout = true; card.layoutSubtreeIfNeeded()
        XCTAssertEqual(cardLayer.affineTransform(), CGAffineTransform(scaleX: s, y: s))
        p.landFlight(try XCTUnwrap(p.flights[2]))
        XCTAssertNil(clip.superview, "the clip goes")
        XCTAssertEqual(card.frame, NSRect(x: 0, y: 62, width: 402, height: 714))
        XCTAssertEqual(cardLayer.affineTransform(), .identity, "unscaled again")
    }

    /// Caught mid-way, a flight flies on from its clip as shown, its rect and
    /// radius, with the content view unlayered as in an app: AppKit's
    /// geometry, which knows nothing of the card's scale, is the clip's.
    /// Its place gone before it lands, it stands at its clip's top left,
    /// unscaled at its own size, and the clip goes.
    func testAnInterruptedFlightAndALandingWithoutItsPlace() throws {
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 402, height: 874), styleMask: [.borderless], backing: .buffered, defer: false)
        let content = try XCTUnwrap(window.contentView)
        let thumb = NodeView(id: 1, kind: "view", presenter: p)
        p.views[1] = thumb; content.addSubview(thumb)
        thumb.frame = NSRect(x: 300, y: 600, width: 56, height: 84)
        let card = NodeView(id: 2, kind: "view", presenter: p)
        p.views[2] = card; content.addSubview(card)
        card.frame = NSRect(x: 0, y: 62, width: 402, height: 714)
        card.applyStyle(["border_radius_top_left": 40, "border_radius_top_right": 40, "border_radius_bottom_left": 40, "border_radius_bottom_right": 40])
        var op = BatchOp(op: .flight, nodeID: 2)
        op.payload["from"] = NSNumber(value: 1)
        p.beginFlight(op); p.presentFlight(2, 0.5); p.flightsBatchApplied()
        let clip = try XCTUnwrap(card.superview as? FlightClip)
        let other = NodeView(id: 4, kind: "view", presenter: p)
        p.views[4] = other; content.addSubview(other)
        other.frame = NSRect(x: 10, y: 10, width: 100, height: 100)
        var again = BatchOp(op: .flight, nodeID: 4)
        again.payload["from"] = NSNumber(value: 2)
        p.beginFlight(again)
        let source = try XCTUnwrap(p.flights[4]?.source)
        XCTAssertEqual(source.rect.width, 229, accuracy: 0.01, "the clip's width, not the card's 402")
        XCTAssertEqual(source.rect.height, 399, accuracy: 0.01)
        XCTAssertEqual(source.radius, 20, accuracy: 0.01)
        let flight = try XCTUnwrap(p.flights[2])
        flight.slot?.removeFromSuperview()
        p.landFlight(flight)
        XCTAssertNil(clip.superview, "the clip goes")
        XCTAssertNil(card.flightLook)
        XCTAssertEqual(card.layer?.affineTransform() ?? .identity, .identity, "unscaled")
        XCTAssertEqual(card.frame, NSRect(x: 150, y: 144, width: 402, height: 714), "at its clip's top left, its own size")
    }
}
#endif
