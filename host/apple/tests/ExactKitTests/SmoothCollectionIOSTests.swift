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
    private func collections(revision: Int, correction: Any) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": revision, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": [["view": 10, "root": 10, "epoch": 1]], "correction": correction]]]
    }
    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
    /// Waits for `done` (EventWaits' `waitFor`), giving up only once the
    /// app's display link has delivered more than `frames` s of frames since
    /// it began (a motion is 0.3 s), or at the 30 s cap, never on wall time
    /// alone: in the full parallel suite a 2 s deadline failed a motion the
    /// link had starved. What the link did goes in a failure's message and a
    /// LATE-PASS attachment (theme2's ruling); a found livelock (the link
    /// running, the port still) fails at the budget.
    private func wait(_ what: String, frames budget: CFTimeInterval = 0.6, _ done: () -> Bool) {
        let clock = FrameClock.shared
        var first: CFTimeInterval?, last: CFTimeInterval = clock.link?.timestamp ?? 0, seen = 0, gap = 0.0, at = CACurrentMediaTime()
        let delivered = { first.map { last - $0 } ?? 0 }
        // Each new link timestamp a frame delivered (sampled each turn, under a frame apart, and once more for the facts).
        let sample = {
            let now = CACurrentMediaTime()
            if let t = clock.link?.timestamp, t > last {
                if first == nil { first = t }
                last = t; seen += 1; gap = max(gap, now - at); at = now
            }
        }
        waitFor(what, giveUp: {
            sample()
            return delivered() > budget
        }, facts: {
            sample()
            let link = clock.link
            return "frames seen \(seen) over \(String(format: "%.3f", delivered())) s of link time, largest gap \(String(format: "%.3f", max(gap, CACurrentMediaTime() - at))) s, link \(link == nil ? "none" : link!.isPaused ? "paused" : "running"), app \(UIApplication.shared.applicationState.rawValue), scene \(window?.windowScene?.activationState.rawValue ?? -9)"
        }, done)
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

    /// From a port UIKit reads back a ULP off the pixel grid (5/3 at scale 3
    /// comes back 1.6666666666666665), a smooth correction still eases to its
    /// target: the driver took that ULP for the port moving under it and
    /// restarted its ease every frame, so it never left its first frame (the
    /// full suite's flake, its link running at full rate; PixelGrid.same).
    func testASmoothCorrectionFromAPortReadBackOffTheGridLands() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        // At 3x (the grid 5/3 sits on), whatever the simulator's own scale (Astra's round 2).
        scroll.traitOverrides.displayScale = 3
        scroll.updateTraitsIfNeeded()
        // A grid offset UIKit hands back not exactly as given (the precondition, shown here).
        let scale = max(1, scroll.traitCollection.displayScale)
        var start: CGFloat?
        for k in 1...600 {
            let y = PixelGrid.offset(CGPoint(x: 0, y: CGFloat(k) / scale), in: scroll).y
            scroll.contentOffset.y = y
            if scroll.contentOffset.y != PixelGrid.offset(scroll.contentOffset, in: scroll).y { start = y; break }
        }
        guard let y0 = start else { throw XCTSkip("this display hands every grid offset back exactly at scale \(scale): nothing to drive") }
        XCTAssertTrue(PixelGrid.same(scroll.contentOffset.y, y0, in: scroll), "a ULP off, within the read-back slop")
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": seq, "offset": 1700, "smooth": true])]))
        wait("moving") { self.moving(p) }
        wait("landed") { !p.collections.animating.contains(1) }
        XCTAssertEqual(scroll.contentOffset.y, 1700, accuracy: 0.5, "from \(y0), read back \(scroll.contentOffset.y)")
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
