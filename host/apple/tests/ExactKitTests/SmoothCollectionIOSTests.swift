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
    /// Runs the loop until `done`, failing with `what` after `timeout` s.
    private func wait(_ what: String, _ timeout: Double = 2, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(timeout)
        while !done(), Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.005)) }
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
    /// After a frame has shown, a retarget is a fresh ease from there.
    func testATargetBeforeTheFirstFrameIsTheMotionsOneTarget() throws {
        let p = presenter()
        list(p, correction: NSNull())
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 1700, "smooth": true])]))
        wait("the motion begins") { self.moving(p) }
        let driver = try XCTUnwrap(p.collections.offsetDrivers[1])
        if !driver.drawn {
            let began = driver.began
            p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": "0", "offset": 1690, "smooth": true])]))
            XCTAssertTrue(p.collections.offsetDrivers[1] === driver)
            XCTAssertEqual(driver.to.y, 1690, accuracy: 0.5)
            XCTAssertEqual(driver.began, began, "the same motion, not a second one")
        }
        wait("a frame shows") { driver.drawn }
        let began = driver.began
        p.apply(wireBatch([collections(revision: 4, correction: ["scrollSequence": "0", "offset": 1680, "smooth": true])]))
        XCTAssertGreaterThan(driver.began, began, "after a frame, a fresh ease from what shows")
        wait("the animation ended") { !p.collections.animating.contains(1) }
        XCTAssertEqual(try XCTUnwrap(p.views[1]?.scroll).contentOffset.y, 1680, accuracy: 0.5)
    }

    /// A smooth correction under half a point (a port rounded to a device
    /// pixel) is set, not eased: 0.3 s of frames for it moved nothing.
    func testASmoothCorrectionUnderHalfAPointIsSet() throws {
        let p = presenter()
        list(p, correction: NSNull())
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 0.3, "smooth": true])]))
        XCTAssertFalse(p.collections.animating.contains(1))
        XCTAssertNil(p.collections.offsetDrivers[1])
        XCTAssertEqual(scroll.contentOffset.y, 0.3, accuracy: 0.34, "set at once, to the pixel")
    }
}
#endif
