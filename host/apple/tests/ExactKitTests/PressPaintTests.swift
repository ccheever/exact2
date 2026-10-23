#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// The author's pressed paint (LLP 1061 D7) on UIKit and AppKit: while a
/// node is pressed it shows its `press_opacity` as an additive fade, its
/// `press_background_color` in place of its own, and its `press_color` as
/// the text colour of its subtree (not of a text with a colour of its own);
/// an engine write mid-press keeps the paint and its value comes back with
/// the release; the paint replaces the platform's unsaid press and composes
/// with the scale.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class PressPaintTests: XCTestCase {
    #if os(iOS)
    private var window: UIWindow!
    #else
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }
    #endif

    private let blue: [Double] = [37, 99, 235, 255], navy: [Double] = [29, 78, 216, 255]
    private let white: [Double] = [255, 255, 255, 255], yellow: [Double] = [253, 224, 71, 255], green: [Double] = [187, 247, 208, 255]
    /// A colour as the style holds it.
    private func ink(_ c: [Double]) -> BatchValue { .array(c.map(BatchValue.number)) }

    /// A pressable box (1) holding a text that takes its colour (2) and one
    /// with its own (3).
    private func fixture(_ press: [String: Any]) throws -> (Presenter, NodeView) {
        let p = Presenter()
        #if os(iOS)
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        #else
        _ = NSApplication.shared
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        #endif
        let style: [String: Any] = ["background_color": blue, "text_color": white].merging(press) { $1 }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "style": style],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Inherits"], "style": ["text_color": white]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Keeps"], "style": ["text_color": green, "written": ["text_color"]]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 50.0, "y": 50.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 20.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 20.0, "w": 200.0, "h": 20.0],
        ]))
        return (p, try XCTUnwrap(p.views[1]))
    }

    private func layer(_ v: NodeView) -> CALayer? { v.layer }
    private func fade(_ v: NodeView) -> CABasicAnimation? { layer(v)?.animation(forKey: "pressOpacity") as? CABasicAnimation }

    func testPressingShowsThePaintAndReleasingGivesTheEnginesBack() throws {
        let (p, v) = try fixture(["press_opacity": 0.5, "press_background_color": navy, "press_color": yellow])
        let inherits = try XCTUnwrap(p.views[2]), keeps = try XCTUnwrap(p.views[3])
        v.pressed = true
        let held = try XCTUnwrap(fade(v))
        XCTAssertTrue(held.isAdditive)
        XCTAssertEqual(try XCTUnwrap(held.fromValue as? Float), 0, "eased in from where it was")
        XCTAssertEqual(try XCTUnwrap(held.toValue as? Float), -0.5, accuracy: 1e-6)
        XCTAssertEqual(held.duration, PressFeedback.duration)
        XCTAssertFalse(held.isRemovedOnCompletion)
        XCTAssertEqual(v.style["background_color"], ink(navy))
        XCTAssertEqual(v.style["text_color"], ink(yellow))
        XCTAssertEqual(inherits.style["text_color"], ink(yellow), "the subtree takes the press colour")
        XCTAssertEqual(keeps.style["text_color"], ink(green), "a text with its own keeps it")
        XCTAssertNotNil(layer(v)?.animation(forKey: kCATransition), "the colours cross-fade")
        // An engine write mid-press: kept for the release, the paint still shown.
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": white, "text_color": white, "press_opacity": 0.5, "press_background_color": navy, "press_color": yellow]]]))
        XCTAssertEqual(v.style["background_color"], ink(navy))
        v.press.start -= 1; v.pressPaint.at -= 1 // released a moment later
        v.pressed = false
        let back = try XCTUnwrap(fade(v))
        XCTAssertEqual(try XCTUnwrap(back.toValue as? Float), 0)
        XCTAssertTrue(back.isRemovedOnCompletion)
        XCTAssertEqual(v.style["background_color"], ink(white), "the engine's newer value")
        XCTAssertEqual(inherits.style["text_color"], ink(white))
        XCTAssertEqual(keeps.style["text_color"], ink(green))
    }

    func testADroppedPressColourTakesTheHeldAncestorsColour() throws {
        let (p, outer) = try fixture(["press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["text_color": white, "press_color": green]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Label"], "style": ["text_color": white]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let inner = try XCTUnwrap(p.views[4]), label = try XCTUnwrap(p.views[5])
        outer.pressed = true
        inner.pressed = true
        XCTAssertEqual(inner.style["text_color"], ink(green))
        XCTAssertEqual(label.style["text_color"], ink(green))
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["text_color": white]]]))
        XCTAssertTrue(inner.pressed)
        XCTAssertEqual(inner.style["text_color"], ink(yellow))
        XCTAssertEqual(label.style["text_color"], ink(yellow))
        outer.pressPaint.at -= 1
        outer.pressed = false
        XCTAssertEqual(inner.style["text_color"], ink(white))
        XCTAssertEqual(label.style["text_color"], ink(white))
        inner.pressPaint.at -= 1
        inner.pressed = false
        XCTAssertEqual(inner.style["text_color"], ink(white))
    }

    /// The finger (the mouse) leaving the box lets the paint go; back inside it shows again.
    func testThePaintFollowsThePointer() throws {
        let (_, v) = try fixture(["press_background_color": navy])
        v.pressed = true
        v.pressPaint.at -= 1
        v.pressFollows(inside: false)
        XCTAssertEqual(v.style["background_color"], ink(blue))
        v.pressFollows(inside: true)
        XCTAssertEqual(v.style["background_color"], ink(navy))
        v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual(v.style["background_color"], ink(blue))
        XCTAssertNil(fade(v), "no opacity written, none faded")
    }

    /// A release in the same turn as the press (UIKit's delayed touches in a
    /// scroll view) waits until the press has eased in, so the tap shows.
    func testAQuickTapStillShowsThePaint() throws {
        let (_, v) = try fixture(["press_opacity": 0.5, "press_background_color": navy])
        v.pressed = true
        v.pressed = false
        XCTAssertEqual(v.style["background_color"], ink(navy), "held")
        XCTAssertEqual(try XCTUnwrap(fade(v)?.toValue as? Float), -0.5, accuracy: 1e-6)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: PressFeedback.duration + 0.05))
        XCTAssertEqual(v.style["background_color"], ink(blue), "then let go")
        // The release eases back to rest: read where it settles, not the fade
        // in flight, which a run loop slept past (a loaded machine) finds
        // already completed and gone.
        if let back = fade(v) { XCTAssertEqual(try XCTUnwrap(back.toValue as? Float), 0, "easing back") }
        XCTAssertFalse(v.pressPaint.down)
        XCTAssertEqual(v.pressPaint.ease.to, 0)
        let rest = v.pressPaint.ease.start + PressFeedback.duration
        while CACurrentMediaTime() < rest { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertEqual(v.pressPaint.ease.factor(at: CACurrentMediaTime()), 0, accuracy: 1e-6, "settled at rest")
    }

    /// An ancestor's quick release (it waits out `PressFeedback.unseen`), with
    /// a descendant pressed inside that wait: the release gives back only the
    /// text colour it lent, not the descendant's own press, which holds while
    /// it is down, nor a label the descendant's own press colours now (LLP
    /// 1061 D7, stage 9 G4).
    private func pressedInsideTheWait(_ inner: [String: Any]) throws -> (Presenter, NodeView, NodeView, NodeView) {
        try XCTSkipIf(ExactEnv.agentFreezes, "the agent's frozen clock lets a press go at once")
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white].merging(inner) { $1 }],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Label"], "style": ["text_color": white]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 40.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 200.0, "h": 20.0],
        ]))
        let (b, label) = (try XCTUnwrap(p.views[4]), try XCTUnwrap(p.views[5]))
        v.pressed = true
        XCTAssertEqual(b.style["text_color"], ink(yellow), "the ancestor's press lends its colour")
        XCTAssertEqual(label.style["text_color"], ink(yellow))
        v.pressPaint.at = CACurrentMediaTime() // a quick tap: its release waits
        v.pressed = false
        XCTAssertTrue(v.pressPaint.releaseHeld)
        b.pressed = true
        let until = Date(timeIntervalSinceNow: 2)
        while v.pressPaint.releaseHeld, Date() < until { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        XCTAssertFalse(v.pressPaint.releaseHeld, "the ancestor let go")
        XCTAssertEqual(v.style["background_color"], ink(blue))
        return (p, v, b, label)
    }

    func testAnAncestorsHeldReleaseLeavesADescendantsNewerPress() throws {
        let (_, _, b, label) = try pressedInsideTheWait(["press_background_color": green])
        XCTAssertEqual(b.style["background_color"], ink(green), "the descendant's press holds while it is down")
        XCTAssertEqual(b.style["text_color"], ink(white), "the lent colour given back")
        XCTAssertEqual(label.style["text_color"], ink(white))
        b.pressPaint.at -= 1
        b.pressed = false
        XCTAssertEqual(b.style["background_color"], ink(white))
    }

    func testAnAncestorsHeldReleaseLeavesTheDescendantsOwnInkBelowIt() throws {
        let (_, _, b, label) = try pressedInsideTheWait(["press_background_color": green, "press_color": navy])
        XCTAssertEqual(b.style["text_color"], ink(navy), "its own press colour")
        XCTAssertEqual(label.style["text_color"], ink(navy), "and its label takes it, the ancestor's let go")
        b.pressPaint.at -= 1
        b.pressed = false
        XCTAssertEqual(b.style["text_color"], ink(white))
        XCTAssertEqual(label.style["text_color"], ink(white), "its own again")
    }

    /// A descendant pressed and let go while the ancestor stays down keeps
    /// the colour the ancestor lends it until the ancestor lets go.
    func testADescendantsReleaseKeepsAHeldAncestorsColour() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_background_color": green]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 40.0, "w": 200.0, "h": 40.0],
        ]))
        let b = try XCTUnwrap(p.views[4])
        v.pressed = true
        b.pressed = true
        b.pressPaint.at -= 1
        b.pressed = false
        XCTAssertEqual(b.style["background_color"], ink(white), "its own press gone")
        XCTAssertEqual(b.style["text_color"], ink(yellow), "the held ancestor's colour stays")
        v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual(b.style["text_color"], ink(white))
    }

    /// A descendant with its own press colour let go while the ancestor is
    /// down: it and the label it coloured take the ancestor's colour again.
    func testADescendantsOwnInkGivesWayToAHeldAncestorsAtItsRelease() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_color": blue]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Label"], "style": ["text_color": white]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let (b, label) = (try XCTUnwrap(p.views[4]), try XCTUnwrap(p.views[5]))
        v.pressed = true
        b.pressed = true
        XCTAssertEqual([b.style["text_color"], label.style["text_color"]], [ink(blue), ink(blue)], "its own press colour, on its label too")
        b.pressPaint.at -= 1
        b.pressed = false
        XCTAssertEqual([b.style["text_color"], label.style["text_color"]], [ink(yellow), ink(yellow)], "the held ancestor's again")
        v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual([b.style["text_color"], label.style["text_color"]], [ink(white), ink(white)])
    }

    /// Three levels: a middle node pressed after its descendant's own press
    /// colour shows takes none of that descendant's colour.
    func testAMiddlePressLeavesAPressedDescendantsOwnColour() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_color": green]],
            ["op": "create", "id": 5, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_color": blue]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let (b, c) = (try XCTUnwrap(p.views[4]), try XCTUnwrap(p.views[5]))
        v.pressed = true
        c.pressed = true
        XCTAssertEqual(c.style["text_color"], ink(blue))
        b.pressed = true
        XCTAssertEqual(b.style["text_color"], ink(green))
        XCTAssertEqual(c.style["text_color"], ink(blue), "its own press is its boundary")
        c.pressPaint.at -= 1
        c.pressed = false
        XCTAssertEqual(c.style["text_color"], ink(green), "let go, it takes the nearest press's colour")
        b.pressPaint.at -= 1
        b.pressed = false
        XCTAssertEqual([b.style["text_color"], c.style["text_color"]], [ink(yellow), ink(yellow)], "then the outer one's")
        v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual([b.style["text_color"], c.style["text_color"]], [ink(white), ink(white)])
    }

    /// A pressed node whose own press writes no colour takes a nearer press's.
    func testABackgroundOnlyPressTakesANearerPressesColour() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_color": green]],
            ["op": "create", "id": 5, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_background_color": blue]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let (b, c) = (try XCTUnwrap(p.views[4]), try XCTUnwrap(p.views[5]))
        v.pressed = true
        c.pressed = true
        XCTAssertEqual(c.style["text_color"], ink(yellow))
        b.pressed = true
        XCTAssertEqual([c.style["text_color"], c.style["background_color"]], [ink(green), ink(blue)])
    }

    /// A style write to a pressed node whose own press writes no colour keeps
    /// the colour a held ancestor lends it.
    func testAStyleWriteKeepsALentColourOnAPressedNode() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_background_color": green]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let b = try XCTUnwrap(p.views[4])
        v.pressed = true
        b.pressed = true
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["background_color": white, "text_color": white, "press_background_color": green, "opacity": 0.9]]]))
        XCTAssertEqual([b.style["text_color"], b.style["background_color"]], [ink(yellow), ink(green)])
        b.pressPaint.at -= 1
        b.pressed = false
        XCTAssertEqual(b.style["text_color"], ink(yellow), "the held ancestor's still")
        v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual(b.style["text_color"], ink(white))
    }

    /// An ancestor with a written colour of its own still lends its press
    /// colour; a pressed node that comes to write its own ends the lent one
    /// below it.
    func testWrittenColoursAndLentOnes() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow, "written": ["text_color"]])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_color": blue]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Label"], "style": ["text_color": white]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let (b, label) = (try XCTUnwrap(p.views[4]), try XCTUnwrap(p.views[5]))
        v.pressed = true
        b.pressed = true
        b.pressPaint.at -= 1
        b.pressed = false
        XCTAssertEqual([b.style["text_color"], label.style["text_color"]], [ink(yellow), ink(yellow)], "the ancestor's press colour again")
        v.pressPaint.at -= 1
        v.pressed = false
        // A background-only press under a held ancestor, then a colour of its own.
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["background_color": white, "text_color": white, "press_background_color": green]]]))
        v.pressed = true
        b.pressed = true
        XCTAssertEqual(label.style["text_color"], ink(yellow))
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["background_color": white, "text_color": navy, "written": ["text_color"], "press_background_color": green]]]))
        XCTAssertEqual(b.style["text_color"], ink(navy))
        XCTAssertNotEqual(label.style["text_color"], ink(yellow), "the lent colour ends below a colour of its own")
    }

    /// Pressing and letting go of a descendant again and again under a held
    /// ancestor keeps the ancestor's list of what it colours bounded.
    func testRepeatedPressesUnderAHeldAncestorKeepItsListBounded() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_background_color": green]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let b = try XCTUnwrap(p.views[4])
        v.pressed = true
        let before = v.pressPaint.inked.count
        for _ in 0..<5 { b.pressed = true; b.pressPaint.at -= 1; b.pressed = false }
        XCTAssertEqual(v.pressPaint.inked.count, before)
        XCTAssertEqual(b.style["text_color"], ink(yellow))
    }

    /// A middle node that comes to write its own colour while a grandchild's
    /// own press is down: the grandchild keeps its press paint.
    func testAMiddleNodesOwnColourLeavesAPressedGrandchild() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["text_color": white]],
            ["op": "create", "id": 5, "kind": "view", "handlers": ["press"], "style": ["background_color": white, "text_color": white, "press_background_color": green]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let d = try XCTUnwrap(p.views[5])
        v.pressed = true
        d.pressed = true
        XCTAssertEqual(d.style["background_color"], ink(green))
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["text_color": white, "written": ["text_color"]]]]))
        XCTAssertEqual(d.style["background_color"], ink(green), "its press holds")
        XCTAssertEqual(d.style["text_color"], ink(white), "the lent colour gone with the middle's own")
    }

    /// A style that drops the pressed look while held: the dim and colours
    /// go with it, and the release still takes back the scale, though the
    /// platform's press now answers for the node.
    func testAPaintTheStyleDropsWhileHeldStillGoesAtTheRelease() throws {
        let (p, v) = try fixture(["press_opacity": 0.5, "press_background_color": navy, "press_scale": 0.9])
        v.pressed = true
        v.pressPaint.at -= 1; v.press.start -= 1
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": white, "text_color": white]]]))
        XCTAssertFalse(v.authorsPress)
        XCTAssertEqual(v.style["background_color"], ink(white), "the engine's")
        XCTAssertNil(fade(v), "no dim")
        v.pressed = false
        XCTAssertEqual(v.style["background_color"], ink(white))
        XCTAssertEqual(v.press.to, 1, "and the scale")
    }

    /// The press's colours read again from each engine style while held: a
    /// new press colour reaches the subtree; a text that comes to write its
    /// own colour keeps it; a background the press no longer names is the
    /// engine's again.
    func testThePaintFollowsTheEnginesStylesWhileHeld() throws {
        let (p, v) = try fixture(["press_background_color": navy, "press_color": yellow])
        let inherits = try XCTUnwrap(p.views[2])
        v.pressed = true
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": blue, "text_color": white, "press_color": green]]]))
        XCTAssertEqual(v.style["text_color"], ink(green))
        XCTAssertEqual(inherits.style["text_color"], ink(green), "the new colour reaches the subtree")
        XCTAssertEqual(v.style["background_color"], ink(blue), "no pressed background named")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["text_color": white, "written": ["text_color"]]]]))
        XCTAssertEqual(inherits.style["text_color"], ink(white), "its own now")
        v.pressPaint.at -= 1
        v.pressed = false
        XCTAssertEqual(v.style["text_color"], ink(white))
        XCTAssertEqual(inherits.style["text_color"], ink(white))
    }

    /// The pressed opacity follows the style and the node's own opacity
    /// while held, and a style that clears it takes the dim away.
    func testThePressOpacityFollowsWhileHeld() throws {
        let (p, v) = try fixture(["press_opacity": 0.5])
        v.pressed = true
        XCTAssertEqual(try XCTUnwrap(fade(v)?.toValue as? Float), -0.5, accuracy: 1e-6)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": blue, "text_color": white, "press_opacity": 0.8]]]))
        XCTAssertEqual(try XCTUnwrap(fade(v)?.toValue as? Float), -0.2, accuracy: 1e-6)
        p.apply(wireBatch([["op": "present", "id": 1, "property": "opacity", "x": 0.5]]))
        XCTAssertEqual(try XCTUnwrap(fade(v)?.toValue as? Float), -0.1, accuracy: 1e-6)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": blue, "text_color": white]]]))
        XCTAssertNil(fade(v), "gone at once, before the platform's press can answer")
    }

    /// A press colour changed in the same style as the node's own colour
    /// still reaches the views it coloured; a view below that comes to write
    /// its own colour takes what is below it back from the press.
    func testTheSubtreeFollowsAColourChangedWhileHeld() throws {
        let (p, v) = try fixture(["press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "view", "style": ["text_color": white]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Below"], "style": ["text_color": white]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 1, "ids": [2, 3, 5]],
        ]))
        let inherits = try XCTUnwrap(p.views[2]), between = try XCTUnwrap(p.views[5]), below = try XCTUnwrap(p.views[6])
        v.pressed = true
        XCTAssertEqual(below.style["text_color"], ink(yellow))
        let black: [Double] = [0, 0, 0, 255]
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": blue, "text_color": black, "press_color": green]]]))
        XCTAssertEqual(inherits.style["text_color"], ink(green))
        XCTAssertEqual(below.style["text_color"], ink(green))
        p.apply(wireBatch([["op": "style", "id": 5, "style": ["text_color": blue, "written": ["text_color"]]]]))
        XCTAssertEqual(between.style["text_color"], ink(blue))
        XCTAssertEqual(below.style["text_color"], ink(white), "its own again, until its engine style says the new one")
        XCTAssertEqual(inherits.style["text_color"], ink(green))
    }

    #if os(macOS)
    /// The window resigning key with the button held: AppKit delivers the up
    /// to no view of it, so the press ends there and takes its paint back.
    func testResigningKeyEndsAPress() throws {
        let (p, v) = try fixture(["press_background_color": navy])
        v.pressed = true
        v.pressPaint.at -= 1
        p.windowLostKey()
        XCTAssertFalse(v.pressed)
        XCTAssertEqual(v.style["background_color"], ink(blue))
    }
    #endif

    /// Author > platform: any pressed paint (an opacity of 1 included) is the
    /// author's press, so the platform's shows none; it composes with a scale.
    func testThePaintReplacesThePlatformsPressAndComposesWithTheScale() throws {
        for paint: [String: Any] in [["press_opacity": 1], ["press_color": yellow], ["press_background_color": navy]] {
            let (_, v) = try fixture(paint)
            XCTAssertTrue(v.authorsPress)
            #if os(iOS)
            XCTAssertFalse(v.platformPressDims, "\(paint)")
            v.pressed = true
            XCTAssertNil(v.layer.animation(forKey: "pressDim"), "\(paint)")
            #else
            XCTAssertFalse(v.platformPressTints, "\(paint)")
            #endif
        }
        let (_, v) = try fixture(["press_opacity": 0.4, "press_scale": 0.9])
        v.pressed = true
        XCTAssertEqual(v.press.to, 0.9)
        XCTAssertEqual(try XCTUnwrap(fade(v)?.toValue as? Float), -0.6, accuracy: 1e-6)
    }

    /// LLP 1115 D4, LLP 1121 D9: a platform field's text is its own colour,
    /// its own press's included, else FieldText: an inherited colour and an
    /// ancestor's press colour never tint it.
    func testAFieldsInkIsItsOwnOrItsOwnPressNeverAnAncestors() throws {
        let (p, v) = try fixture(["press_color": yellow])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "Inherits"], "style": ["text_color": white]],
            ["op": "create", "id": 5, "kind": "input", "props": ["value": "Own press"], "handlers": ["press"], "style": ["text_color": white, "press_color": green]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 40.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 70.0, "w": 200.0, "h": 30.0],
        ]))
        let field = try XCTUnwrap(p.views[4]), own = try XCTUnwrap(p.views[5])
        let rgba = { (c: CGColor) in (c.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent, options: nil)?.components ?? []).map { Double(($0 * 255).rounded()) } }
        let fieldText = rgba(ThemeRoles.color("FieldText", in: field, or: SystemColor.canvasText).cgColor)
        XCTAssertFalse(field.showsOwnTextColor)
        XCTAssertEqual(rgba(field.fieldInk.cgColor), fieldText, "an inherited colour is not the field's")
        v.pressed = true
        XCTAssertFalse(field.showsOwnTextColor, "an ancestor's press lends its colour")
        XCTAssertEqual(rgba(field.fieldInk.cgColor), fieldText)
        v.pressed = false
        own.pressed = true
        XCTAssertTrue(own.showsOwnTextColor, "its own press's colour is its own")
        XCTAssertEqual(rgba(own.fieldInk.cgColor), green)
    }
    func testAPresslessLinkTracksActivationWithoutShowingActivePaintOrPlatformFeedback() throws {
        let (p, _) = try fixture([:])
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "button", "props": ["href": "/destination"]],
            ["op": "frame", "id": 4, "x": 50, "y": 200, "w": 120, "h": 40],
            ["op": "roots", "ids": [1, 4]]
        ]))
        let link = try XCTUnwrap(p.views[4])
        for authored in [false, true] {
            link.applyStyle(authored ? ["press_scale": .number(0.9), "press_background_color": ink(navy)] : [:])
            link.pressed = true
            XCTAssertEqual(link.defaultLink, "/destination")
            XCTAssertEqual(link.press.to, 1)
            XCTAssertFalse(link.pressPaint.down)
            #if os(iOS)
            XCTAssertFalse(link.platformPressDims)
            XCTAssertNil(link.press.dimmedAt)
            #else
            XCTAssertFalse(link.pressTinted)
            #endif
            link.pressFollows(inside: true)
            XCTAssertEqual(link.press.to, 1)
            XCTAssertFalse(link.pressPaint.down)
            link.pressed = false
        }
        link.handlers = ["press"]
        link.pressed = true
        XCTAssertEqual(link.press.to, 0.9, "a declared handler gets its active feedback")
        XCTAssertTrue(link.pressPaint.down)
    }

}
#endif
