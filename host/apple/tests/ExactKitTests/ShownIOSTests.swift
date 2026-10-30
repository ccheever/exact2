#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A batch applied inside an animation UIKit has open (the keyboard's, a
/// rotation's, a transition's alongside block) moves only the views the last
/// frame showed; a view made, or shown again, since takes its place at once
/// (`Presenter.moveOrPlace`). UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class ShownIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        super.tearDown()
    }

    /// A scene's window: only it reaches the screen, and a commit held back is waited for only there
    /// (`Presenter.committed`). Each test asks for the scene itself, so it runs hosted.
    private func presenter(_ label: String, in scene: UIWindowScene) -> Presenter {
        let session = ExactApp.shared.makeSession(label: label)
        self.session = session
        let p = session.presenter
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        return p
    }

    /// `change` inside an animation UIKit has open, which is checked open: a
    /// view that does not move did not for want of one.
    private func animating(_ change: () -> Void) {
        var open = false
        withoutActuallyEscaping(change) { change in
            UIView.animate(withDuration: 0.5) { open = UIView.inheritedAnimationDuration > 0; change() }
        }
        XCTAssertTrue(open, "UIKit had an animation open")
    }

    /// The turn ends and Core Animation commits it: a frame shows what it holds.
    private func nextFrame() { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }

    /// Every layer under `layer` that has an animation, by its view's class or its own.
    private func moving(_ layer: CALayer) -> [String] {
        var found: [String] = []
        func walk(_ layer: CALayer) {
            if let keys = layer.animationKeys(), !keys.isEmpty { found.append("\(layer.delegate.map { "\(type(of: $0))" } ?? "\(type(of: layer))") \(keys.sorted())") }
            layer.sublayers?.forEach(walk)
        }
        walk(layer)
        return found
    }

    func testAViewMadeInsideAnAnimationTakesItsPlaceWhileOneThatShowedMoves() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-made", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Shown"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
        ]))
        nextFrame()
        let shown = try XCTUnwrap(p.views[2])
        animating {
            p.apply(wireBatch([
                ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [255, 0, 0, 255]]],
                ["op": "create", "id": 4, "kind": "text", "props": ["text": "Collections"], "style": ["font_size": 20.0]],
                ["op": "children", "id": 3, "ids": [4]],
                ["op": "children", "id": 1, "ids": [2, 3]],
                ["op": "frame", "id": 2, "x": 0.0, "y": 200.0, "w": 100.0, "h": 40.0],
                ["op": "frame", "id": 3, "x": 50.0, "y": 50.0, "w": 300.0, "h": 40.0],
                ["op": "frame", "id": 4, "x": 10.0, "y": 5.0, "w": 200.0, "h": 30.0],
            ]))
            // Its words painted, and moved, before a frame showed them.
            p.paintVisibleText()
            p.apply(wireBatch([["op": "fieldContent", "id": 4, "rect": [30.0, 0.0, 150.0, 30.0]]]))
        }
        let made = try XCTUnwrap(p.views[3]), text = try XCTUnwrap(p.views[4])
        XCTAssertNotNil(text.textRasterLayer, "painted")
        XCTAssertNotNil(shown.layer.animation(forKey: "position"), "what showed moves with the animation")
        XCTAssertEqual(moving(made.layer), [], "what no frame showed is where it is laid out, its words too")
    }

    /// A route laid out in one turn, then again inside an animation before
    /// any frame showed it (the Landmarks port's Collections grid).
    func testAViewLaidOutAgainInsideAnAnimationBeforeAFrameShowedItTakesItsPlace() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-again", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Favorites"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 28.0, "y": 0.0, "w": 200.0, "h": 30.0],
        ]))
        animating {
            p.apply(wireBatch([
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 380.0],
                ["op": "frame", "id": 2, "x": 348.0, "y": 0.0, "w": 40.0, "h": 30.0],
            ]))
        }
        XCTAssertEqual(moving(try XCTUnwrap(p.views[1]).layer), [])
    }

    /// A commit an open explicit transaction holds back shows nothing: a view
    /// made under it is not shown after a turn of the run loop, and takes its
    /// place at once inside an animation.
    func testAViewACommitHeldBackHasNotShown() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-held", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
        ]))
        nextFrame()
        CATransaction.begin()
        p.apply(wireBatch([
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Held"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ]))
        nextFrame()
        let made = try XCTUnwrap(p.views[2])
        XCTAssertFalse(p.showed(made), "not committed")
        animating {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 40.0, "y": 40.0, "w": 100.0, "h": 30.0]]))
        }
        XCTAssertEqual(moving(made.layer), [])
        CATransaction.commit()
        nextFrame()
        XCTAssertTrue(p.showed(made), "committed")
    }

    /// A view that showed, taken out and put back under a held commit, with
    /// a new place: what Core Animation has of it is its old place, so it has
    /// not shown again until the commit, and takes its new place at once.
    func testAViewPutBackUnderAHeldCommitHasNotShownAgain() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-back", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Back"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ]))
        nextFrame()
        let view = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.showed(view))
        CATransaction.begin()
        p.apply(wireBatch([["op": "children", "id": 1, "ids": []]]))
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]], ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 100.0, "h": 30.0]]))
        nextFrame()
        XCTAssertFalse(p.showed(view), "its new place is not committed")
        animating {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 40.0, "y": 140.0, "w": 100.0, "h": 30.0]]))
        }
        XCTAssertEqual(moving(view.layer), [])
        CATransaction.commit()
        nextFrame()
        XCTAssertTrue(p.showed(view), "committed")
    }

    /// A view hidden in a frame and shown again in place under a held
    /// commit: what Core Animation has of it is hidden, so it has not shown
    /// again until the commit, and takes its place at once.
    func testAViewShownAgainInPlaceUnderAHeldCommitHasNotShown() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-unhidden", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Again"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ]))
        nextFrame()
        let view = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 20.0, "display": "none"]]]))
        nextFrame()
        XCTAssertTrue(view.isHidden)
        CATransaction.begin()
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 20.0]]]))
        nextFrame()
        XCTAssertFalse(p.showed(view), "shown again, not committed")
        animating {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 40.0, "y": 40.0, "w": 100.0, "h": 30.0]]))
        }
        XCTAssertEqual(moving(view.layer), [])
        CATransaction.commit()
        nextFrame()
        XCTAssertTrue(p.showed(view), "committed")
    }

    /// A text's box placed again when a moved parent's origin changes no
    /// drawn frame (`Presenter.regridChildren`): a text no frame has shown
    /// takes its place at once there too.
    func testATextsBoxPlacedByARegridOfItsParentTakesItsPlace() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-regrid", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Shown"], "style": ["font_size": 12.0]],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.2, "y": 0.0, "w": 300.0, "h": 100.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 60.0, "w": 100.0, "h": 20.0],
        ]))
        nextFrame()
        let parent = try XCTUnwrap(p.views[2])
        XCTAssertEqual(parent.window?.screen.scale, 3, "the pixel arithmetic is at 3x")
        animating {
            p.apply(wireBatch([
                ["op": "create", "id": 3, "kind": "text", "props": ["text": "Regrid"], "style": ["font_size": 20.0]],
                ["op": "children", "id": 2, "ids": [3, 4]],
                ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                ["op": "fieldContent", "id": 3, "rect": [0.2, 0.0, 180.0, 30.0]],
            ]))
            p.paintVisibleText()
            // Drawn where it was (0.2 and 0.3 both round to a third), its origin 0.1 later: the text's box, 0.4 then 0.5 from the window's edge, moves a pixel.
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.3, "y": 0.0, "w": 300.0, "h": 100.0]]))
        }
        let text = try XCTUnwrap(p.views[3])
        XCTAssertNotNil(text.textBoxLayer)
        XCTAssertEqual(moving(text.layer), [])
    }

    /// `display: none` lifted inside an animation on a box a frame showed
    /// before it was hidden: the box, what the op lifting it changes and its
    /// child appear where they are laid out, as CSS starts no transition for
    /// a box that was not rendered.
    func testABoxShownAgainInsideAnAnimationTakesItsPlace() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-display", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Again"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ]))
        nextFrame()
        let box = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.showed(box))
        p.apply(wireBatch([
            ["op": "style", "id": 2, "style": ["background_color": [255, 0, 0, 255], "display": "none"]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0],
        ]))
        nextFrame()
        XCTAssertTrue(box.isHidden)
        animating {
            p.apply(wireBatch([
                ["op": "style", "id": 2, "style": ["background_color": [0, 0, 255, 255], "opacity": 0.5]],
                ["op": "frame", "id": 2, "x": 40.0, "y": 40.0, "w": 100.0, "h": 40.0],
                // The child showed, under the box that did not
                ["op": "frame", "id": 3, "x": 10.0, "y": 5.0, "w": 80.0, "h": 30.0],
            ]))
        }
        XCTAssertFalse(box.isHidden)
        XCTAssertEqual(moving(box.layer), [])
    }

    /// A box placed again by its parent's regrid (`Presenter.regridChildren`,
    /// through `applyFrame`): its parent moved a third of a pixel, drawn where
    /// it was, and the box, made inside the animation, is drawn a pixel on.
    /// One no frame has shown takes its place at once there too.
    func testABoxPlacedByARegridOfItsParentTakesItsPlace() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-regrid-frame", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Shown"], "style": ["font_size": 12.0]],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.2, "y": 0.0, "w": 300.0, "h": 100.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 60.0, "w": 100.0, "h": 20.0],
        ]))
        nextFrame()
        let parent = try XCTUnwrap(p.views[2])
        XCTAssertEqual(parent.window?.screen.scale, 3, "the pixel arithmetic is at 3x")
        animating {
            p.apply(wireBatch([
                ["op": "create", "id": 3, "kind": "text", "props": ["text": "Regrid"], "style": ["font_size": 20.0]],
                ["op": "children", "id": 2, "ids": [3, 4]],
                ["op": "frame", "id": 3, "x": 0.25, "y": 0.0, "w": 200.0, "h": 30.0],
            ]))
            // The parent at 0.3 is drawn at a third, as at 0.2; the box, at 0.55 from the edge, at two thirds, not one
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.3, "y": 0.0, "w": 300.0, "h": 100.0]]))
        }
        let box = try XCTUnwrap(p.views[3])
        XCTAssertEqual(box.frame.minX, 1.0 / 3, accuracy: 0.001, "drawn a pixel on by the regrid")
        XCTAssertEqual(moving(box.layer), [])
    }

    /// A view hidden and shown again before any commit (a capture hides
    /// boxes and shows them in one turn): no frame showed it hidden, so it
    /// still moves with an animation (`Presenter.shownAgain`).
    func testAViewHiddenAndShownInOneTurnStillMoves() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-capture", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Kept"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ]))
        nextFrame()
        let view = try XCTUnwrap(p.views[2])
        view.isHidden = true
        view.isHidden = false
        XCTAssertTrue(p.showed(view), "the last frame showed it, unhidden")
        animating { p.apply(wireBatch([["op": "frame", "id": 2, "x": 40.0, "y": 40.0, "w": 100.0, "h": 30.0]])) }
        XCTAssertNotNil(view.layer.animation(forKey: "position"), "it moves with the animation")
    }

    /// A view moved to another parent inside an animation takes its place
    /// there at once: the last frame showed it elsewhere. One moved among its
    /// own parent's children moves with the animation.
    func testAViewMovedToAnotherParentTakesItsPlaceWhileOneReorderedMoves() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-reparent", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Moved"], "style": ["font_size": 20.0]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Stays"], "style": ["font_size": 20.0]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Reordered"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 2, "ids": [4, 5, 6]],
            ["op": "create", "id": 7, "kind": "text", "props": ["text": "Other"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 3, "ids": [7]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 200.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 40.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 80.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
        ]))
        nextFrame()
        let moved = try XCTUnwrap(p.views[4]), reordered = try XCTUnwrap(p.views[6])
        animating {
            p.apply(wireBatch([
                // Taken by its new parent first: it moves between parents without leaving the window
                ["op": "children", "id": 3, "ids": [7, 4]],
                ["op": "children", "id": 2, "ids": [6, 5]],
                ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                ["op": "frame", "id": 4, "x": 0.0, "y": 40.0, "w": 200.0, "h": 30.0],
            ]))
        }
        XCTAssertTrue(moved.superview === p.views[3]?.container, "in its new parent")
        XCTAssertEqual(moving(moved.layer), [], "placed in its new parent at once")
        XCTAssertNotNil(reordered.layer.animation(forKey: "position"), "a view reordered in its parent moves with the animation")
    }

    /// A flat leaf (a layer) that a parent's children op makes a view inside
    /// an animation (`FlatLeaves.promote`, a leaf moved into a scroller):
    /// the view is new and takes its place at once.
    func testALeafPromotedInsideAnAnimationTakesItsPlace() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-promote", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Scroller"], "style": ["font_size": 20.0]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
            ["op": "content", "id": 3, "w": 400.0, "h": 300.0],
        ]))
        nextFrame()
        XCTAssertNil(p.views[2], "a leaf, drawn as a layer")
        animating {
            p.apply(wireBatch([
                ["op": "children", "id": 1, "ids": [3]],
                ["op": "children", "id": 3, "ids": [4, 2]],
                ["op": "frame", "id": 2, "x": 0.0, "y": 40.0, "w": 100.0, "h": 40.0],
            ]))
        }
        let promoted = try XCTUnwrap(p.views[2], "a view in the scroller")
        XCTAssertEqual(moving(promoted.layer), [])
    }

    /// The views the host makes for a node inside an animation take their
    /// place at once, as a new view does: the native control over a tablist
    /// made in it (`Segments.sync`, after the batch's ops), and a glass
    /// group's container for a box that showed and is made a group in it.
    func testTheViewsTheHostMakesInsideAnAnimationTakeTheirPlace() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("glass groups are iOS 26's") }
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-host-views", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 20, "kind": "view", "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255]]],
            ["op": "create", "id": 21, "kind": "view", "props": ["backgroundMaterial": "glass"], "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255]]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 1, "ids": [20]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 20, "x": 0.0, "y": 200.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 40.0, "h": 40.0],
        ]))
        nextFrame()
        p.buttonFace = { id in var face = ButtonFace(); face.title = id == 11 ? "Reply" : "Map"; return face }
        var tabs: [[String: Any]] = [["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "tablist"], "style": ["text_color": [0, 0, 0, 255]]],
                                     ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0]]
        for (i, id) in [11, 12].enumerated() {
            tabs += [["op": "create", "id": id, "kind": "control", "handlers": ["press"],
                      "props": ["type": "button", "accessibilityRole": "tab", "accessibilityLabel": ["Reply", "Map"][i], "accessibilitySelected": i == 0 ? "true" : "false"],
                      "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                     ["op": "frame", "id": id, "x": Double(i) * 100, "y": 0.0, "w": 100.0, "h": 40.0]]
        }
        animating {
            p.apply(wireBatch(tabs + [
                ["op": "children", "id": 10, "ids": [11, 12]],
                ["op": "children", "id": 1, "ids": [10, 20]],
                ["op": "props", "id": 20, "set": ["glassGroup": "12"]],
            ]))
        }
        // A native control or tab bar over the tablist, as the tabs ask
        let bar = try XCTUnwrap(p.views[10]?.subviews.first { $0 is UISegmentedControl || $0 is UITabBar }, "the tablist's native view")
        XCTAssertEqual(bar.layer.animationKeys() ?? [], [], "the native view where it goes at once")
        let group = try XCTUnwrap(p.views[20]?.glassGroupView, "a glass group's container")
        XCTAssertEqual(group.layer.animationKeys() ?? [], [], "the container where it goes at once")
    }

    /// A virtualized list's row view lent to a new row inside an animation,
    /// which the batch gives no frame (`NodePool.end`): the fresh view's
    /// frame, at once, as the view is another node's.
    func testAViewLentToANewRowTakesAFreshFrameAtOnce() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-pool", in: scene)
        func collections(_ rows: [Int]) -> [String: Any] {
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
                "rows": rows.map { ["view": $0, "root": $0, "epoch": 1] }, "correction": NSNull()]]]
        }
        func row(_ base: Int) -> [[String: Any]] {
            [["op": "create", "id": base, "kind": "button", "handlers": ["press"], "props": ["testId": "row-\(base)"]],
             ["op": "create", "id": base + 1, "kind": "text", "props": ["text": "Chat \(base)"]],
             ["op": "children", "id": base, "ids": [base + 1]]]
        }
        p.apply(wireBatch([collections([10]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 9, "kind": "view"]]
            + row(10)
            + [["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0], ["op": "frame", "id": 11, "x": 8.0, "y": 8.0, "w": 200.0, "h": 20.0],
               ["op": "children", "id": 1, "ids": [10]], ["op": "children", "id": 9, "ids": [1]], ["op": "roots", "ids": [9]],
               ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 400.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        nextFrame()
        let view = try XCTUnwrap(p.views[10])
        animating {
            p.apply(wireBatch([collections([20]), ["op": "destroy", "id": 10], ["op": "destroy", "id": 11]] + row(20)
                + [["op": "children", "id": 1, "ids": [20]]]))
        }
        XCTAssertTrue(p.views[20] === view, "the same view, lent to the next row")
        XCTAssertEqual(view.frame, .zero, "a fresh view's frame")
        XCTAssertEqual(view.layer.animationKeys() ?? [], [], "at once")
    }

    /// A context preview made inside an animation (`Presenter.positionContexts`):
    /// its panel is moved over its source at once, as a new view is placed.
    func testAContextPreviewMadeInsideAnAnimationTakesItsPlace() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-context", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["position_type": "relative"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "source"]],
            ["op": "children", "id": 1, "ids": [5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 20.0, "w": 100.0, "h": 50.0],
        ]))
        nextFrame()
        animating {
            p.apply(wireBatch([
                ["op": "create", "id": 2, "kind": "view"],
                ["op": "create", "id": 3, "kind": "view", "style": ["position_type": "absolute"]],
                ["op": "create", "id": 4, "kind": "view", "props": ["contextTarget": "source", "contextMagnify": "false"]],
                ["op": "children", "id": 1, "ids": [2, 5]],
                ["op": "children", "id": 2, "ids": [3]],
                ["op": "children", "id": 3, "ids": [4]],
                ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 50.0],
                ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
                ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
            ]))
        }
        let panel = try XCTUnwrap(p.views[3])
        XCTAssertEqual(panel.convert(panel.bounds, to: p.views[1]).minY, 20, accuracy: 0.01, "over its source")
        XCTAssertEqual(moving(panel.layer), [])
    }

    /// The offset UIKit sets on a scroller it insets (a bar's) to keep it at
    /// its start as a safe area reaches it inside an animation: a scroller
    /// no frame has shown is at its start under the inset at once, and one
    /// that showed moves with the animation (`ScrollView.bounds`).
    func testAnOffsetUIKitSetsInsideAnAnimationMovesOnlyIfItsScrollerShowed() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-offset", in: scene)
        let host = UIViewController()
        let window = try XCTUnwrap(self.window)
        window.rootViewController = host
        host.view.addSubview(p.viewport)
        func scroller(_ id: Int, x: Double) -> [[String: Any]] {
            [
                ["op": "create", "id": id, "kind": "view", "style": ["overflow_y": "scroll"]],
                ["op": "create", "id": id + 1, "kind": "view"],
                ["op": "children", "id": id, "ids": [id + 1]],
                ["op": "frame", "id": id, "x": x, "y": 0.0, "w": 200.0, "h": 400.0],
                ["op": "frame", "id": id + 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 1000.0],
                ["op": "content", "id": id, "w": 200.0, "h": 1000.0],
            ]
        }
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view"], ["op": "roots", "ids": [1]], ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0]]
            + scroller(10, x: 0) + [["op": "children", "id": 1, "ids": [10]]]))
        nextFrame()
        p.apply(wireBatch(scroller(20, x: 200) + [["op": "children", "id": 1, "ids": [10, 20]]]))
        let shown = try XCTUnwrap(p.views[10]?.scroll), made = try XCTUnwrap(p.views[20]?.scroll)
        // At their start under the window's own safe area, as a bar's scroller opens
        for sv in [shown, made] { sv.contentInsetAdjustmentBehavior = .always }
        window.layoutIfNeeded()
        for sv in [shown, made] { sv.contentOffset.y = -sv.adjustedContentInset.top }
        XCTAssertTrue(p.showed(try XCTUnwrap(p.views[10])))
        XCTAssertFalse(p.showed(try XCTUnwrap(p.views[20])))
        let rest = made.adjustedContentInset.top
        animating {
            host.additionalSafeAreaInsets.top = 82
            host.view.layoutIfNeeded()
        }
        for sv in [shown, made] { XCTAssertEqual(sv.contentOffset.y, -sv.adjustedContentInset.top, "at its start under the inset") }
        XCTAssertEqual(made.adjustedContentInset.top - rest, 82, "the safe area reached it")
        XCTAssertEqual(shown.adjustedContentInset.top, made.adjustedContentInset.top, "and the one that showed")
        XCTAssertNotNil(shown.layer.animation(forKey: "bounds.origin"), "what showed moves with the animation")
        XCTAssertEqual(made.layer.animationKeys() ?? [], [], "what no frame showed is at its start at once")
    }

    /// A node's parts its layout places (a symbol's image view) take their
    /// place with it when UIKit lays it out inside the animation.
    func testANewViewsPartsAreLaidOutWhereTheyEnd() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "what the last frame showed is read in a scene's window") }
        let p = presenter("shown-parts", in: scene)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
        ]))
        nextFrame()
        animating {
            p.apply(wireBatch([
                ["op": "create", "id": 2, "kind": "image", "props": ["imageSource": "symbol:star", "symbolName": "star"], "style": ["font_size": 17.0]],
                ["op": "children", "id": 1, "ids": [2]],
                ["op": "frame", "id": 2, "x": 40.0, "y": 40.0, "w": 60.0, "h": 60.0],
            ]))
            self.window?.layoutIfNeeded()
        }
        let image = try XCTUnwrap(p.views[2])
        XCTAssertNotNil(image.symbolView, "a symbol's own view")
        XCTAssertEqual(moving(image.layer), [])
    }
}
#endif
