#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1070.000 §6.2, LLP 1010 §6.8: a correction marked smooth (a smooth
/// `scrollIntoView`, or a `scroll-behavior: smooth` list following its end)
/// is one 0.3 s ease-in-out motion of the port; while it runs the fill
/// reports no travel (which would cancel a request); a later smooth target
/// retargets it from what shows; an ordinary correction is set at once.
final class SmoothCollectionIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }
    private func presenter() -> Presenter {
        let p = Presenter()
        // In the test host's scene: UIKit animates a scroll view only in a
        // window on a screen.
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        return p
    }
    private func collections(revision: Int, correction: Any, extent: Double = 2000) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": revision, "scrollSequence": 0, "count": 50, "totalExtent": extent,
            "rows": [["view": 10, "root": 10, "epoch": 1]], "correction": correction]]]
    }
    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
    /// Runs the loop until `done`, failing with `what` after `timeout` s.
    private func wait(_ what: String, _ timeout: Double = 2, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(timeout)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.005)) }
        XCTAssertTrue(done(), what)
    }
    private func moving(_ p: Presenter) -> Bool { p.collections.offsetDrivers[1] != nil }
    private func list(_ p: Presenter, correction: Any) {
        p.apply(wireBatch([collections(revision: 1, correction: correction),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"], ["op": "create", "id": 11, "kind": "view"],
            ["op": "children", "id": 10, "ids": [11]], ["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
    }

    func testASmoothCorrectionAnimatesAndReportsNoTravelWhileItRuns() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        XCTAssertEqual(scroll.contentOffset.y, 0)
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 1700, "smooth": true])]))
        XCTAssertTrue(p.collections.animating.contains(1), "the host animates to it")
        XCTAssertEqual(try XCTUnwrap(p.collections.geometry(1)).offset, 1700, accuracy: 0.5, "the runner plans from where it is headed")
        XCTAssertNil(p.collections.motion?(1), "no travel reported while it runs")
        wait("the port animates to it, from the next turn") { self.moving(p) }
        wait("the animation ended") { !p.collections.animating.contains(1) }
        XCTAssertEqual(scroll.contentOffset.y, 1700, accuracy: 0.5)
        // A smooth correction while one runs retargets it at once, from what
        // shows: never held to land without animation.
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": seq, "offset": 1000, "smooth": true])]))
        XCTAssertTrue(p.collections.animating.contains(1))
        p.apply(wireBatch([collections(revision: 4, correction: ["scrollSequence": seq, "offset": 1200, "smooth": true])]))
        XCTAssertNil(p.collections.owedTargets[1], "retargeted, not held")
        XCTAssertEqual(p.collections.animationTargets[1]?.y, 1200)
        wait("moving") { self.moving(p) }
        wait("landed") { !p.collections.animating.contains(1) }
        XCTAssertEqual(scroll.contentOffset.y, 1200, accuracy: 0.5)
    }

    /// Mid-flight, a new target retargets from where the port is (no jump),
    /// and a drag stops it where it is. The offset each frame is the one on
    /// screen, so a reader of `contentOffset` never sees the destination early.
    func testAMovingCorrectionRetargetsFromWhereItIsAndADragStopsIt() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        let sequence = p.collections.entries[1]?.cursor.sequence
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": seq, "offset": 1700, "smooth": true])]))
        wait("on its way") { scroll.contentOffset.y > 200 }
        let mid = scroll.contentOffset.y
        XCTAssertLessThan(mid, 1700, "on its way, the offset is what shows")
        XCTAssertEqual(p.collections.entries[1]?.cursor.sequence, sequence, "its frames are not the reader's travel")
        p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": seq, "offset": 1200, "smooth": true])]))
        XCTAssertEqual(scroll.contentOffset.y, mid, accuracy: 0.5, "retargeted where it is, no jump")
        wait("landed") { !p.collections.animating.contains(1) }
        XCTAssertEqual(scroll.contentOffset.y, 1200, accuracy: 0.5)
        // A target past the content is driven only as far as it can go.
        p.apply(wireBatch([collections(revision: 4, correction: ["scrollSequence": seq, "offset": 1500, "smooth": true])]))
        wait("on its way again") { scroll.contentOffset.y > 1320 }
        scroll.contentSize.height = 1600 // the content shrank under it: the end is now 1300
        var past = false
        wait("landed at the edge") {
            if p.collections.animating.contains(1), scroll.contentOffset.y > 1300.5 { past = true }
            return !p.collections.animating.contains(1)
        }
        XCTAssertFalse(past, "no frame past the new edge")
        XCTAssertEqual(scroll.contentOffset.y, 1300, accuracy: 0.5)
        scroll.contentSize.height = 2000
        // A drag stops a moving one where it is.
        p.apply(wireBatch([collections(revision: 5, correction: ["scrollSequence": seq, "offset": 300, "smooth": true])]))
        wait("on its way down") { scroll.contentOffset.y < 1250 }
        let stopped = scroll.contentOffset.y
        p.views[1]?.scrollViewWillBeginDragging(scroll)
        XCTAssertFalse(moving(p)); XCTAssertFalse(p.collections.animating.contains(1))
        spin(0.2)
        XCTAssertEqual(scroll.contentOffset.y, stopped, accuracy: 0.5, "left where the drag found it")
    }

    /// A sent message's end-follow arrives in the batch whose composer
    /// shrinks back: the port's resize is not the reader moving, so the
    /// correction planned before it still lands.
    func testACorrectionInTheBatchThatResizesThePortLands() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": seq, "offset": 900]),
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 280.0]]))
        XCTAssertEqual(scroll.contentOffset.y, 900, accuracy: 0.5)
    }

    /// An ordinary correction stops a running animation, even to where the
    /// list already is, and the list reports where it is again.
    func testAnOrdinaryCorrectionStopsASmoothOne() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": seq, "offset": 1700, "smooth": true])]))
        XCTAssertTrue(p.collections.animating.contains(1))
        let at = Double(scroll.contentOffset.y)
        p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": seq, "offset": at])]))
        XCTAssertFalse(p.collections.animating.contains(1))
        XCTAssertEqual(try XCTUnwrap(p.collections.geometry(1)).offset, at, accuracy: 0.5)
    }

    /// The end of an animation that was stopped and replaced ends nothing:
    /// only the running one's completion ends it.
    func testAStoppedAnimationsEndLeavesTheNextOneRunning() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": seq, "offset": 1700, "smooth": true])]))
        p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": seq, "offset": 0])]))
        XCTAssertFalse(moving(p), "an ordinary correction stops it where it is")
        XCTAssertEqual(scroll.contentOffset.y, 0, accuracy: 0.5)
        p.apply(wireBatch([collections(revision: 4, correction: ["scrollSequence": seq, "offset": 900, "smooth": true])]))
        XCTAssertTrue(p.collections.animating.contains(1))
        wait("its own end") { !p.collections.animating.contains(1) }
        XCTAssertNil(p.collections.animationSerial[1], "nothing kept for a list that is not animating")
        XCTAssertEqual(scroll.contentOffset.y, 900, accuracy: 0.5)
    }

    func testAnOrdinaryCorrectionIsSetAtOnce() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 900])]))
        XCTAssertFalse(p.collections.animating.contains(1))
        XCTAssertEqual(scroll.contentOffset.y, 900, accuracy: 0.5)
    }

    /// A followed end's measured row comes a report after its estimate,
    /// well inside a frame: a target that arrives before the motion's first
    /// frame is still its one target, begun when it began (LLP 1010 §6.8).
    /// After a frame has shown, or once the port moved under it, a retarget
    /// is a fresh ease from there; one that folds back onto the port is no
    /// motion. No frame can run between two calls on the main thread here.
    func testATargetBeforeTheFirstFrameIsTheMotionsOneTarget() throws {
        _ = presenter()
        let scroll = UIScrollView(frame: window.bounds)
        scroll.contentSize = CGSize(width: 400, height: 3000)
        window.addSubview(scroll)
        scroll.contentOffset = CGPoint(x: 0, y: 1000)
        var ended: [Bool] = []
        let driver = OffsetDriver(scroll: scroll, to: CGPoint(x: 0, y: 1700), duration: 0.3, serial: 1,
                                  done: { _, finished in ended.append(finished) }, reclamp: { _ in })
        driver.start()
        let began = driver.began
        driver.retarget(CGPoint(x: 0, y: 1690), serial: 2)
        XCTAssertFalse(driver.drawn)
        XCTAssertEqual(driver.to.y, 1690)
        XCTAssertEqual(driver.began, began, "the same motion, not a second one")
        XCTAssertEqual(scroll.contentOffset.y, 1000, "nothing moved yet")
        wait("a frame shows") { driver.drawn }
        driver.retarget(CGPoint(x: 0, y: 1680), serial: 3)
        XCTAssertGreaterThan(driver.began, began, "after a frame, a fresh ease from what shows")
        wait("it ends") { !ended.isEmpty }
        XCTAssertEqual(scroll.contentOffset.y, 1680, accuracy: 0.5)
        driver.cancel()

        // Content that shrank under it before its first frame: from where
        // the port was clamped to, not from where it began.
        scroll.contentOffset = CGPoint(x: 0, y: 1000)
        let clamped = OffsetDriver(scroll: scroll, to: CGPoint(x: 0, y: 1700), duration: 0.3, serial: 4, done: { _, _ in }, reclamp: { _ in })
        clamped.start()
        let first = clamped.began
        scroll.contentOffset = CGPoint(x: 0, y: 900)
        clamped.retarget(CGPoint(x: 0, y: 800), serial: 5)
        XCTAssertNotEqual(clamped.began, first, "a fresh ease from the clamped port")
        wait("its first frame") { clamped.drawn }
        XCTAssertLessThanOrEqual(scroll.contentOffset.y, 900, "from 900 toward 800, never back toward 1000")
        XCTAssertGreaterThanOrEqual(scroll.contentOffset.y, 800)
        clamped.cancel()

        // A target folded back onto the port before any frame: set, done.
        scroll.contentOffset = CGPoint(x: 0, y: 900)
        var folded: Bool?
        let fold = OffsetDriver(scroll: scroll, to: CGPoint(x: 0, y: 964), duration: 0.3, serial: 6,
                                done: { _, finished in folded = finished }, reclamp: { _ in })
        fold.start()
        fold.retarget(CGPoint(x: 0, y: 900.49), serial: 7)
        XCTAssertEqual(folded, true)
        XCTAssertFalse(FrameClock.shared.wants(fold), "no frames for it")
        XCTAssertGreaterThan(scroll.contentOffset.y, 900.2, "set there, to the pixel")
    }

    /// A deferred start whose target folded back to the port by the time it
    /// runs (an estimate, then the measured row that undoes it) is no motion.
    func testADeferredStartFoldedOntoThePortRunsNoFrames() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 64, "smooth": true])]))
        p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": "0", "offset": 0.49, "smooth": true])]))
        wait("the deferred start ran") { !p.collections.startOwed.contains(1) }
        XCTAssertNil(p.collections.offsetDrivers[1], "no motion")
        XCTAssertFalse(p.collections.animating.contains(1))
        XCTAssertGreaterThan(scroll.contentOffset.y, 0.2, "set there, to the pixel")
    }

    /// A row removed at a followed end: UIKit pulls the offset in to the
    /// shorter content at once, and the port lands there; its layer moves
    /// from where it showed by an additive animation of the difference, as
    /// a collection view's batch update does, so a batch meanwhile (a
    /// receipt growing on the row above) moves the port under it without
    /// stopping it.
    func testAFollowedEndThatMovedUpMovesFromWhereThePortShowed() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 1700, "smooth": false])]))
        XCTAssertEqual(scroll.contentOffset.y, 1700, accuracy: 0.5, "at its end")
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": seq, "offset": 1652, "smooth": true], extent: 1952),
            ["op": "content", "id": 1, "w": 300.0, "h": 1952.0]]))
        XCTAssertEqual(scroll.contentOffset.y, 1652, accuracy: 0.5, "the port is at the new end")
        let motions = { (scroll.layer.animationKeys() ?? []).compactMap { scroll.layer.animation(forKey: $0) as? CASpringAnimation } }
        let motion = try XCTUnwrap(motions().first, "its layer moves")
        XCTAssertEqual(motion.keyPath, "bounds.origin.y")
        XCTAssertTrue(motion.isAdditive)
        XCTAssertEqual(try XCTUnwrap(motion.fromValue as? CGFloat), 48, accuracy: 0.5, "from where it showed")
        XCTAssertEqual(motion.toValue as? Int, 0)
        XCTAssertEqual(motion.perceptualDuration, 0.3, accuracy: 0.001, "on a batch update's spring")
        XCTAssertEqual(motion.bounce, 0, accuracy: 0.001)
        XCTAssertFalse(p.collections.animating.contains(1), "no offset motion: the runner reads the port where it is")
        p.apply(wireBatch([collections(revision: 4, correction: ["scrollSequence": seq, "offset": 1660, "smooth": true], extent: 1960),
            ["op": "content", "id": 1, "w": 300.0, "h": 1960.0]]))
        XCTAssertEqual(motions().count, 1, "a batch meanwhile keeps it")
        wait("the end it moved to") { abs(scroll.contentOffset.y - 1660) < 0.5 && !p.collections.animating.contains(1) }
    }

    /// A row that leaves with an exit animation at a followed end (a deleted
    /// message fading) moves the rows as a collection view's batch update
    /// does: each from where it showed, on the batch's spring. The rows
    /// before it come down over the gap with it, a row the port's move
    /// mounts above them comes down with them, and the row after it stays.
    func testARemovalMovesItsRowsFromWhereTheyShowed() throws {
        let p = presenter()
        let rows = { (ids: [Int]) in ids.map { ["view": $0, "root": $0, "epoch": 1] } }
        p.apply(wireBatch([["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000, "rows": rows([10, 12, 14]), "correction": NSNull()]]],
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"], ["op": "create", "id": 12, "kind": "view"], ["op": "create", "id": 14, "kind": "view"],
            ["op": "children", "id": 1, "ids": [10, 12, 14]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 1880.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 1920.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 14, "x": 0.0, "y": 1960.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        p.apply(wireBatch([["op": "collections", "items": [["view": 1, "revision": 2, "scrollSequence": 0, "count": 50, "totalExtent": 2000, "rows": rows([10, 12, 14]),
            "correction": ["scrollSequence": "0", "offset": 1700, "smooth": false]]]]]))
        XCTAssertEqual(scroll.contentOffset.y, 1700, accuracy: 0.5, "at its end")
        let leaving = try XCTUnwrap(p.views[12]), seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([["op": "collections", "items": [["view": 1, "revision": 3, "scrollSequence": 0, "count": 49, "totalExtent": 1960, "rows": rows([8, 10, 14]),
            "correction": ["scrollSequence": seq, "offset": 1660, "smooth": true]]]],
            ["op": "exit", "id": 12],
            ["op": "create", "id": 8, "kind": "view"],
            ["op": "children", "id": 1, "ids": [8, 10, 14]],
            ["op": "frame", "id": 8, "x": 0.0, "y": 1840.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 14, "x": 0.0, "y": 1920.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 1960.0]]))
        XCTAssertEqual(scroll.contentOffset.y, 1660, accuracy: 0.5, "the port is at the new end")
        let ride = { (view: UIView) in (view.layer.animationKeys() ?? []).compactMap { view.layer.animation(forKey: $0) as? CASpringAnimation }.first { $0.keyPath == "position.y" } }
        XCTAssertTrue((scroll.layer.animationKeys() ?? []).isEmpty, "the rows move, not the port")
        let above = try XCTUnwrap(ride(try XCTUnwrap(p.views[10])), "the row before it moves")
        XCTAssertTrue(above.isAdditive)
        XCTAssertEqual(try XCTUnwrap(above.fromValue as? CGFloat), -40, accuracy: 0.5, "from 40 points up, where it showed")
        XCTAssertEqual(above.perceptualDuration, 0.3, accuracy: 0.001)
        XCTAssertEqual(try XCTUnwrap(ride(leaving)?.fromValue as? CGFloat), -40, accuracy: 0.5, "the leaving row comes down with it")
        XCTAssertEqual(try XCTUnwrap(ride(try XCTUnwrap(p.views[8]))?.fromValue as? CGFloat), -40, accuracy: 0.5, "and the row the move mounted above it")
        XCTAssertNil(ride(try XCTUnwrap(p.views[14])), "the row after it stays where it showed")
    }

    /// A smooth correction under half a point (a port rounded to a device
    /// pixel) is set, not eased: 0.3 s of frames for it moved nothing.
    func testASmoothCorrectionUnderHalfAPointIsSet() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 0.4, "smooth": true])]))
        XCTAssertFalse(p.collections.animating.contains(1))
        XCTAssertNil(p.collections.offsetDrivers[1])
        XCTAssertGreaterThan(scroll.contentOffset.y, 0.1, "set at once")
    }

    /// LLP 1116.000 D1.3: a transcript opened at its end below the bar, then
    /// moved under it (a route's scroller goes under a collapsing title once
    /// its push lands). UIKit clamps the offset to the taller frame and then
    /// compensates for the new inset, which together put the end an inset
    /// short of the port's; the port's start (CSS's scrollTop) stays, so the
    /// end the runner follows is still the end.
    func testAPortThatGoesUnderTheBarKeepsWhereItStarts() throws {
        let p = presenter()
        let host = UIViewController()
        host.additionalSafeAreaInsets.top = 100
        window.rootViewController = host
        host.view.frame = window.bounds
        host.view.layoutIfNeeded()
        let bar = host.view.safeAreaInsets.top
        XCTAssertGreaterThanOrEqual(bar, 100)
        p.apply(wireBatch([collections(revision: 1, correction: NSNull()),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"], ["op": "create", "id": 11, "kind": "view"],
            ["op": "children", "id": 10, "ids": [11]], ["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": Double(bar), "w": 300.0, "h": Double(400 - bar)],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        // In the controller's view, as a route's content is, where the bar's
        // safe area reaches it.
        let node = try XCTUnwrap(p.views[1])
        host.view.addSubview(node)
        let scroll = try XCTUnwrap(node.scroll)
        // Keep the bottom adjustment constant while the top moves under the bar.
        scroll.contentInsetAdjustmentBehavior = .always
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 2000, "smooth": false])]))
        let end = { scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height }
        XCTAssertEqual(scroll.contentOffset.y, end(), accuracy: 0.5, "opened at its end, below the bar")
        let start = try XCTUnwrap(p.collections.geometry(1)).offset
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 400.0]]))
        XCTAssertEqual(scroll.adjustedContentInset.top, bar, accuracy: 0.5, "under the bar, which UIKit insets it by")
        XCTAssertEqual(try XCTUnwrap(p.collections.geometry(1)).offset, start, accuracy: 0.5, "the port starts where it did")
        XCTAssertEqual(scroll.contentOffset.y, end(), accuracy: 0.5, "and its end is still the end")
    }
}
#endif
