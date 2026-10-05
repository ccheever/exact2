#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// What an AppKit node clips (LLP 1054 P2/P3): `overflow: hidden` clips its
/// subviews to the rounded border box, as UIKit does, and a clamped
/// paragraph clips its own drawing, as CSS's line-clamp implies overflow.
/// And what the mouse finds there: visible overflow, and through a
/// `pointer-events: none` box.
final class ClipMacTests: XCTestCase {
    private func node(_ kind: String, _ style: NodeStyle) -> NodeView {
        _ = NSApplication.shared
        let p = Presenter()
        let node = NodeView(id: 1, kind: kind, presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 48, height: 48)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        return node
    }

    func testHiddenOverflowClipsToTheRoundedBox() {
        // The port's avatar: an image in a clipped circle.
        let n = node("view", ["overflow_x": "hidden", "overflow_y": "hidden", "border_radius": 24])
        XCTAssertTrue(n.clipsToBounds)
        XCTAssertEqual(n.layer?.masksToBounds, true)
        XCTAssertEqual(n.layer?.cornerRadius, 24)
    }

    func testVisibleOverflowDoesNotClip() {
        let n = node("view", ["border_radius": 24])
        XCTAssertFalse(n.clipsToBounds)
        XCTAssertEqual(n.layer?.masksToBounds, false)
        // Clipping off again drops the mask's radius with it.
        n.applyStyle(["overflow_x": "hidden", "overflow_y": "hidden", "border_radius": 24])
        n.applyStyle(["border_radius": 24])
        XCTAssertEqual(n.layer?.cornerRadius, 0)
    }

    /// An ellipsized label (`overflow-x: hidden`, whose other axis CSS
    /// computes to `auto`) clips its own text and holds no scroll view: one
    /// took the clicks its button should hear (files diary F15).
    func testAnEllipsizedLabelClipsAndLeavesItsButtonTheClick() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"], "style": ["display": "flex"]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "A rather long label that clips"],
             "style": ["overflow_x": "hidden", "overflow_y": "auto", "text_overflow": "ellipsis", "white_space": "nowrap"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 28.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 28.0],
        ]))
        let button = try XCTUnwrap(p.views[1]), label = try XCTUnwrap(p.views[2])
        XCTAssertNil(label.scroll, "a paragraph has nothing a scroll view would hold")
        XCTAssertTrue(label.clipsToBounds, "its overflow clips")
        let hit = button.hitTest(button.superview!.convert(NSPoint(x: 100, y: 14), from: button))
        XCTAssertTrue(hit === label, "the label is hit, and hands its press to the button: \(String(describing: hit))")
        XCTAssertTrue(label.hasPressableAncestor)
    }

    /// Visible overflow is hit where it paints, as CSS hit-tests it: a
    /// popup positioned beyond its parent's box takes the click (ledger
    /// F13), and a raised sibling's overflow is hit over the content after
    /// it (shop F18); a clipping parent's overflow is not hit.
    func testVisibleOverflowIsHitWhereItPaints() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["display": "flex"]],
            ["op": "create", "id": 2, "kind": "view", "style": ["z_index": 5.0]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "style": ["position_type": "absolute"]],
            ["op": "create", "id": 4, "kind": "view", "style": [:]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 10.0, "y": 40.0, "w": 200.0, "h": 120.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 50.0, "w": 400.0, "h": 200.0],
        ]))
        let root = try XCTUnwrap(p.views[1]), bar = try XCTUnwrap(p.views[2]), popup = try XCTUnwrap(p.views[3])
        let at = { (x: CGFloat, y: CGFloat) in root.hitTest(root.superview!.convert(NSPoint(x: x, y: y), from: root)) }
        XCTAssertTrue(at(100, 140) === popup, "below the bar, over the content after it")
        XCTAssertTrue(at(100, 45) === popup)
        XCTAssertTrue(at(300, 140) === p.views[4], "beside the popup, the content")
        bar.applyStyle(["z_index": 5.0, "overflow_x": "hidden", "overflow_y": "hidden"])
        XCTAssertTrue(at(100, 140) === p.views[4], "a clipped popup is not hit beyond the clip")
    }

    /// `pointer-events` is inherited: a box under a `none` parent carries the
    /// parent's `none` (the Apple host's computed row) and passes the click
    /// through wherever it paints, out in the parent's visible overflow too
    /// (feed's toast translated over the tab bar, x2apps repro
    /// pointer-events-inherit-translate); one that sets `auto` again is hit.
    func testAnInheritedPointerEventsNoneInVisibleOverflowLetsTheClickThrough() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": [:]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "style": ["position_type": "absolute"]],
            ["op": "create", "id": 3, "kind": "view", "style": ["position_type": "absolute", "pointer_events": "none"]],
            ["op": "create", "id": 4, "kind": "view", "style": ["pointer_events": "none"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 20.0, "y": 220.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 300.0, "w": 300.0, "h": 60.0],
            // Painted 80 above the row's box, over the button.
            ["op": "frame", "id": 4, "x": 0.0, "y": -80.0, "w": 300.0, "h": 60.0],
        ]))
        let root = try XCTUnwrap(p.views[1]), toast = try XCTUnwrap(p.views[4])
        let at = { (x: CGFloat, y: CGFloat) in root.hitTest(root.superview!.convert(NSPoint(x: x, y: y), from: root)) }
        XCTAssertTrue(at(100, 240) === p.views[2], "through the inheriting toast to the button")
        toast.applyStyle(["pointer_events": "auto"])
        XCTAssertTrue(at(100, 240) === toast, "a toast that sets auto again takes the click")
    }

    /// A placement (or a projection) that hides a `display: none` box and
    /// later restores it restores what the host had said, not CSS's bit: the
    /// box shows once its `display` does (review B1).
    func testADisplayNoneBoxHiddenAndRestoredByTheHostShowsOnceDisplayed() {
        let n = node("view", ["display": "none"])
        XCTAssertTrue(n.isHidden, "display: none hides it")
        XCTAssertFalse(n.hiddenByHost)
        n.placementHidden = true
        n.placementHidden = false
        let saved = n.hiddenByHost // a tablist projection's save, then its restore
        n.isHidden = true
        n.isHidden = saved
        XCTAssertTrue(n.isHidden, "still display: none")
        n.applyStyle(["display": "block"])
        XCTAssertFalse(n.isHidden, "displayed again, nothing the host said hides it")
    }

    /// Writing back what `isHidden` read of a `display: none` box is not the
    /// host hiding it (review B1's setter guard).
    func testWritingBackADisplayNoneBoxsHiddenIsNotTheHostsWord() {
        let n = node("view", ["display": "none"])
        n.isHidden = n.isHidden
        XCTAssertFalse(n.hiddenByHost)
        n.applyStyle(["display": "block"])
        XCTAssertFalse(n.isHidden)
        n.isHidden = true // the host's own word still holds
        n.applyStyle(["display": "none"]); n.applyStyle(["display": "block"])
        XCTAssertTrue(n.isHidden)
    }

    /// A native module's view inside a `pointer-events: none` box takes no
    /// click: the button around it does (paint F9), as on the web.
    func testAPointerEventsNoneBoxsPlatformViewLetsTheClickThrough() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": [:]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "style": [:]],
            ["op": "create", "id": 3, "kind": "view", "style": ["pointer_events": "none"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 20.0, "y": 20.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
        ]))
        let root = try XCTUnwrap(p.views[1]), thumbnail = try XCTUnwrap(p.views[3])
        thumbnail.addSubview(NSView(frame: thumbnail.bounds)) // the module's view
        XCTAssertTrue(root.hitTest(root.superview!.convert(NSPoint(x: 100, y: 100), from: root)) === p.views[2])
    }

    /// CSS reduces a radius larger than the box (`border-radius: 100px` on a
    /// 56 × 28 pill is 14); Core Animation given the authored 100 draws
    /// nothing at all (the xheavy glass row's rating capsule vanished,
    /// text and material included).
    func testAClipRadiusPastTheBoxIsReducedAsCSSReducesIt() {
        let radii: NodeStyle = ["border_radius_top_left": 100, "border_radius_top_right": 100,
                                "border_radius_bottom_right": 100, "border_radius_bottom_left": 100]
        var style = radii; style["overflow_x"] = "hidden"; style["overflow_y"] = "hidden"
        let n = node("view", style)
        XCTAssertEqual(n.layer?.cornerRadius ?? 0, 24, accuracy: 0.001, "48 × 48: half the side")
        n.frame = NSRect(x: 0, y: 0, width: 56, height: 28)
        XCTAssertEqual(n.layer?.cornerRadius ?? 0, 14, accuracy: 0.001, "the kernel's new size reduces it again")
    }

    func testPercentageBackdropClipsAnEllipseAndFollowsResize() throws {
        var style: NodeStyle = ["backdrop_blur": 4]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node("view", style)
        n.frame.size = CGSize(width: 160, height: 80)
        let mask = try XCTUnwrap(n.layer?.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 20, y: 5)))
        XCTAssertTrue(try XCTUnwrap(mask.path).contains(CGPoint(x: 80, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        let resized = try XCTUnwrap(n.layer?.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 20)))
        n.applyStyle([:])
        XCTAssertNil(n.layer?.mask)
    }

    func testAMaterialsRadiusIsReducedToo() {
        let n = node("view", ["border_radius_top_left": 100, "border_radius_top_right": 100,
                              "border_radius_bottom_right": 100, "border_radius_bottom_left": 100])
        n.frame = NSRect(x: 0, y: 0, width: 56, height: 28)
        n.props["backgroundMaterial"] = "ultra-thin"
        n.updateMaterial()
        XCTAssertEqual(n.materialView?.layer?.cornerRadius ?? 0, 14, accuracy: 0.001)
    }

    func testAClampedParagraphClipsItsDrawing() {
        XCTAssertTrue(node("text", ["line_clamp": 2]).clipsToBounds)
        XCTAssertFalse(node("text", [:]).clipsToBounds)
    }
}
#endif
