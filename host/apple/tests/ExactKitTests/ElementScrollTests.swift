import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class ElementScrollTests: XCTestCase {
    func testPaddingAndMarginsAlignStartAndNearest() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["scrollend"], "style": ["overflow_y": "scroll", "scroll_padding_top": 24, "scroll_padding_bottom": 12]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "target"], "style": ["scroll_margin_top": 8, "scroll_margin_bottom": 6]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 400.0, "w": 100.0, "h": 50.0],
        ]))
        #if os(macOS)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = p.viewport
        let sv = try XCTUnwrap(p.views[1]?.scroll)
        sv.documentView?.setFrameSize(CGSize(width: 200, height: 1000))
        let offset = { sv.contentView.bounds.minY }
        #else
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        let sv = try XCTUnwrap(p.views[1]?.scrollView)
        sv.contentSize = CGSize(width: 200, height: 1000)
        let offset = { sv.contentOffset.y }
        #endif
        defer { withExtendedLifetime(window) {} }
        p.scrollElementIntoView(["target", "nearest", "nearest", "instant"])
        XCTAssertEqual(offset(), 268, accuracy: 0.5)
        p.scrollElementIntoView(["target", "start", "nearest", "instant"])
        XCTAssertEqual(offset(), 368, accuracy: 0.5)
        p.scrollElementIntoView(["target", "nearest", "nearest", "instant"])
        XCTAssertEqual(offset(), 368, accuracy: 0.5)

        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }
        // UIKit animates only in a scene's window: its motion is
        // `testUIKitRevealsAnimateAndAReplacementEndsThem`.
        #if os(macOS)
        sv.contentView.scroll(to: .zero)
        window.orderFront(nil)
        var ended = 0
        let motion = observeMotion(clip: sv.contentView, target: 368, marked: { p.views[1]?.scrollEnd?.elementAnimation != nil }) { completed in
            p.onScrollend = { _ in ended += 1; completed() }
            p.scrollElementIntoView(["target", "start", "nearest", "smooth"])
        }
        if ExactEnv.agentFreezes {
            XCTAssertEqual(motion.first, 368, accuracy: 0.5, "the frozen agent lands authored motion immediately")
        } else {
            XCTAssertLessThan(motion.first, 368, "a smooth element jump must not land synchronously")
            XCTAssertTrue(motion.intermediate, "the real-time scroll has intermediate frames")
        }
        XCTAssertEqual(offset(), 368, accuracy: 0.5)
        XCTAssertEqual(ended, 1)
        #endif
        DisplayPreferences.agent = (true, false)
        #if os(macOS)
        sv.contentView.scroll(to: .zero)
        #else
        sv.setContentOffset(.zero, animated: false)
        #endif
        p.scrollElementIntoView(["target", "start", "nearest", "smooth"])
        XCTAssertEqual(offset(), 368, accuracy: 0.5, "reduced motion lands immediately")
    }

    #if os(macOS)
    func testPageRevealTracksCompletionReplacementAndReset() throws {
        _ = NSApplication.shared
        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "home"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "target"]],
            ["op": "roots", "ids": [1, 2]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 50.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 600.0, "w": 200.0, "h": 400.0],
        ]))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = p.viewport
        window.orderFront(nil)
        defer { p.reset(); window.orderOut(nil); withExtendedLifetime(window) {} }
        let clip = p.viewport.contentView
        for arguments: [Any] in [["target", "start", "nearest"],
                                 ["target", "start", "nearest", "auto"],
                                 ["target", "start", "nearest", "smooth"]] {
            p.scrollElementIntoView(["home", "start", "nearest", "instant"])
            p.scrollElementIntoView(arguments)
            if !ExactEnv.agentFreezes {
                XCTAssertNotNil(p.pageElementAnimation)
                XCTAssertLessThan(clip.bounds.minY, 600)
            }
            let deadline = Date().addingTimeInterval(10)
            while p.pageElementAnimation != nil && Date() < deadline {
                _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
            }
            XCTAssertNil(p.pageElementAnimation)
            XCTAssertEqual(clip.bounds.minY, 600, accuracy: 0.5)
        }
        p.scrollElementIntoView(["home", "start", "nearest", "instant"])
        p.scrollElementIntoView(["target", "start", "nearest"])
        p.scrollElementIntoView(["home", "start", "nearest", "instant"])
        XCTAssertNil(p.pageElementAnimation, "an instant replacement retires the page reveal")
        XCTAssertEqual(clip.bounds.minY, 0, accuracy: 0.5)
        p.scrollElementIntoView(["target", "start", "nearest"])
        p.reset()
        XCTAssertNil(p.pageElementAnimation, "a new document cannot inherit the old reveal")
    }

    func testUnsaidAndAutoRevealUseAppKitAnimation() throws {
        _ = NSApplication.shared
        let target = NSPoint(x: 0, y: 368)

        let reference = NSScrollView(frame: NSRect(x: 0, y: 0, width: 200, height: 200))
        reference.documentView = FlippedReferenceView(frame: NSRect(x: 0, y: 0, width: 200, height: 1000))
        let referenceWindow = NSWindow(contentRect: reference.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        referenceWindow.contentView = reference
        referenceWindow.orderFront(nil)
        defer { withExtendedLifetime(referenceWindow) {} }
        let appKit = observeMotion(clip: reference.contentView, target: target.y) { completed in
            NSAnimationContext.runAnimationGroup({ _ in
                reference.contentView.animator().setBoundsOrigin(target)
            }, completionHandler: completed)
        }
        XCTAssertLessThan(appKit.first, target.y, "a hand-built NSScrollView does not land synchronously")
        XCTAssertTrue(appKit.intermediate, "the hand-built NSScrollView has intermediate frames")
        XCTAssertEqual(appKit.last, target.y, accuracy: 0.5)

        for arguments: [Any] in [
            ["target", "start", "nearest"],
            ["target", "start", "nearest", "auto"],
        ] {
            let exact = try exactAnimation(arguments)
            if ExactEnv.agentFreezes {
                XCTAssertEqual(exact.first, target.y, accuracy: 0.5, "the frozen agent lands platform motion immediately")
            } else {
                XCTAssertLessThan(exact.first, target.y, "an ordinary element reveal must not land synchronously")
                XCTAssertTrue(exact.intermediate, "an ordinary element reveal has AppKit's intermediate frames")
            }
            XCTAssertEqual(exact.last, target.y, accuracy: 0.5)
            XCTAssertEqual(exact.ends, 1, "the reveal completes once")
        }
    }

    func testRevealPolicySurvivesStyleChangesAndScrollTopAutoStaysImmediate() throws {
        let (p, scroll, window) = try exactFixture()
        defer { withExtendedLifetime(window) {} }

        p.apply(wireBatch([
            ["op": "content", "id": 1, "w": 200.0, "h": 1000.0],
            ["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scroll_behavior": "smooth"]],
        ]))
        let smoothStyle = observeExact(p, scroll: scroll, arguments: ["target", "start", "nearest"])
        if !ExactEnv.agentFreezes {
            XCTAssertLessThan(smoothStyle.first, 368)
            XCTAssertTrue(smoothStyle.intermediate)
        }
        XCTAssertEqual(smoothStyle.last, 368, accuracy: 0.5)
        XCTAssertEqual(smoothStyle.ends, 1)

        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scroll_behavior": "auto"]]]))
        let restoredAuto = observeExact(p, scroll: scroll, arguments: ["home", "start", "nearest", "auto"])
        if !ExactEnv.agentFreezes {
            XCTAssertGreaterThan(restoredAuto.first, 0)
            XCTAssertTrue(restoredAuto.intermediate)
        }
        XCTAssertEqual(restoredAuto.last, 0, accuracy: 0.5)
        XCTAssertEqual(restoredAuto.ends, 1)

        p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "368"]]]))
        XCTAssertEqual(scroll.contentView.bounds.minY, 368, accuracy: 0.5, "an auto scrollTop write lands immediately")
        XCTAssertNil(p.views[1]?.scrollEnd?.elementAnimation)
        p.scrollElementIntoView(["home", "start", "nearest", "instant"])
        XCTAssertEqual(scroll.contentView.bounds.minY, 0, accuracy: 0.5, "explicit instant still lands immediately")
        XCTAssertNil(p.views[1]?.scrollEnd?.elementAnimation)
    }

    func testAuthoredScrollTopReplacesOrdinaryElementAnimation() throws {
        guard !ExactEnv.agentFreezes else { return }
        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }

        for behavior in ["auto", "smooth"] {
            let (p, scroll, window) = try exactFixture()
            defer { withExtendedLifetime(window) {} }
            p.apply(wireBatch([
                ["op": "content", "id": 1, "w": 200.0, "h": 1000.0],
                ["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scroll_behavior": behavior]],
            ]))
            var ended = 0
            p.onScrollend = { _ in ended += 1 }

            p.scrollElementIntoView(["target", "start", "nearest"])
            XCTAssertNotNil(p.views[1]?.scrollEnd?.elementAnimation, "the ordinary reveal is in flight")
            p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "120"]]]))

            XCTAssertNil(p.views[1]?.scrollEnd?.elementAnimation, "the authored offset invalidates the reveal token")
            if behavior == "auto" {
                XCTAssertEqual(scroll.contentView.bounds.minY, 120, accuracy: 0.5, "an auto offset write lands immediately")
            }
            let limit = Date().addingTimeInterval(10)
            while ended == 0 && Date() < limit {
                _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
            }
            XCTAssertEqual(scroll.contentView.bounds.minY, 120, accuracy: 0.5, "the superseded reveal must not overwrite the authored offset")
            XCTAssertNil(p.views[1]?.scrollEnd?.elementAnimation)
            XCTAssertEqual(ended, 1, "the replacement settles once")
        }
    }

    private func exactFixture() throws -> (Presenter, NSScrollView, NSWindow) {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["scrollend"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "target"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "home"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 368.0, "w": 100.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
        ]))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = p.viewport
        window.orderFront(nil)
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        scroll.documentView?.setFrameSize(CGSize(width: 200, height: 1000))
        return (p, scroll, window)
    }

    private func exactAnimation(_ arguments: [Any]) throws -> (first: CGFloat, intermediate: Bool, last: CGFloat, ends: Int) {
        let (p, scroll, window) = try exactFixture()
        defer { withExtendedLifetime(window) {} }
        return observeExact(p, scroll: scroll, arguments: arguments)
    }

    private func observeExact(_ p: Presenter, scroll: NSScrollView,
                              arguments: [Any]) -> (first: CGFloat, intermediate: Bool, last: CGFloat, ends: Int) {
        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }
        var ends = 0
        let target: CGFloat = (arguments.first as? String) == "home" ? 0 : 368
        let observed = observeMotion(clip: scroll.contentView, target: target, marked: { p.views[1]?.scrollEnd?.elementAnimation != nil }) { completed in
            p.onScrollend = { _ in ends += 1; completed() }
            p.scrollElementIntoView(arguments)
        }
        return (observed.first, observed.intermediate, observed.last, ends)
    }

    /// AppKit's default animation length (`NSAnimationContext`'s 0.25 s),
    /// which an element reveal and the hand-built reference both take.
    private static let motionDuration: TimeInterval = 0.25

    /// A scroll motion `start` begins on `clip` toward `target`, observed by
    /// the clip's own bounds changes (each frame AppKit draws), not by a poll
    /// of the test's turns, which a stalled run loop slept through (this
    /// class's flake: a 10 ms poll saw no in-between frame in a run starved
    /// to 5.4 s); its end waited for by EventWaits. `marked`: whether the
    /// motion marked itself an animation (an element reveal's
    /// `elementAnimation`), checked at once and, after the end, gone. A
    /// motion that a stall longer than itself collapsed to one drawn frame
    /// counts as having intermediate frames only when it also marked itself
    /// animated, and is kept as a LATE-PASS (theme2's ruling: one frame, the
    /// stall and the mark, all three); else no frame between still fails.
    private func observeMotion(clip: NSClipView, target: CGFloat, marked: (() -> Bool)? = nil,
                               start: (@escaping () -> Void) -> Void) -> (first: CGFloat, intermediate: Bool, last: CGFloat) {
        var positions: [CGFloat] = []
        clip.postsBoundsChangedNotifications = true
        let watch = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification, object: clip, queue: nil) { _ in
            positions.append(clip.bounds.minY)
        }
        defer { NotificationCenter.default.removeObserver(watch) }
        var completed = false
        start { completed = true }
        let first = clip.bounds.minY
        let animated = marked?() ?? false
        if marked != nil, !ExactEnv.agentFreezes, !completed {
            XCTAssertTrue(animated, "an animated reveal marks its element animation")
        }
        var slowest = 0.0, turn = Date()
        waitFor("the scroll completes", late: 1, giveUp: {
            slowest = max(slowest, Date().timeIntervalSince(turn)); turn = Date()
            return false
        }, facts: {
            // (and the turn that completed it, which ends the loop before its give-up runs: Astra's round 1)
            slowest = max(slowest, Date().timeIntervalSince(turn))
            return "\(positions.count) frames drawn, the slowest turn \(String(format: "%.3f", slowest)) s"
        }) { completed }
        if let marked { XCTAssertFalse(marked(), "the element animation's mark is gone after its end") }
        let low = min(first, target), high = max(first, target)
        var intermediate = positions.contains { $0 > low + 1 && $0 < high - 1 }
        if !intermediate, positions.count == 1, slowest > Self.motionDuration, animated {
            latePass("the motion collapsed to one frame", "slowest turn \(String(format: "%.3f", slowest)) s against a \(Self.motionDuration) s motion, marked animated")
            intermediate = true
        }
        return (first, intermediate, clip.bounds.minY)
    }
    #else
    /// A scroller with `scrollend`, `target` at 368 and `home` at 0, in a
    /// shown window of the test host's scene.
    private func revealFixture(_ scene: UIWindowScene) throws -> (Presenter, UIScrollView, UIWindow) {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["scrollend"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "target"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "home"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 368.0, "w": 100.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
        ]))
        let window = UIWindow(windowScene: scene)
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let sv = try XCTUnwrap(p.views[1]?.scrollView)
        sv.contentSize = CGSize(width: 200, height: 1000)
        return (p, sv, window)
    }

    /// Run the main loop until `done`, noting whether the offset passed
    /// strictly between `from` and `to`.
    private func run(_ sv: UIScrollView, from: CGFloat, to: CGFloat, until done: () -> Bool) -> Bool {
        var intermediate = false
        let limit = Date().addingTimeInterval(10)
        while !done() && Date() < limit {
            let y = sv.contentOffset.y
            if y > min(from, to) + 1 && y < max(from, to) - 1 { intermediate = true }
            _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
        }
        XCTAssertTrue(done(), "it settles")
        return intermediate
    }

    /// An unsaid behavior reveals with UIKit's animation, as a hand-built
    /// `setContentOffset(_:animated: true)` does, and ends once, when the
    /// animation does. An instant request or an authored `scrollTop`
    /// replaces a running reveal where it is: the old one moves nothing
    /// after it, and the scroll ends once.
    func testUIKitRevealsAnimateAndAReplacementEndsThem() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "UIKit animates a scroll only in a scene's window") }
        guard !ExactEnv.agentFreezes else { return }
        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }

        let reference = UIScrollView(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        reference.contentSize = CGSize(width: 200, height: 1000)
        let referenceWindow = UIWindow(windowScene: scene)
        referenceWindow.addSubview(reference)
        referenceWindow.makeKeyAndVisible()
        let began = Date()
        reference.setContentOffset(CGPoint(x: 0, y: 368), animated: true)
        XCTAssertTrue(run(reference, from: 0, to: 368) { reference.contentOffset.y == 368 }, "a hand-built scroll view has intermediate frames")
        let uikit = Date().timeIntervalSince(began)
        referenceWindow.isHidden = true

        let (p, sv, window) = try revealFixture(scene)
        defer { window.isHidden = true }
        var ended = 0, endedAt: [CGFloat] = []
        p.onScrollend = { _ in ended += 1; endedAt.append(sv.contentOffset.y) }
        p.scrollElementIntoView(["target", "start", "nearest"])
        XCTAssertLessThan(sv.contentOffset.y, 368, "an unsaid reveal must not land synchronously")
        XCTAssertTrue(p.elementScrolls.revealsInFlight)
        XCTAssertTrue(run(sv, from: 0, to: 368) { ended > 0 }, "the reveal has UIKit's intermediate frames")
        XCTAssertEqual(sv.contentOffset.y, 368, accuracy: 0.5)
        XCTAssertEqual(endedAt, [368], "one scrollend, once the reveal has arrived")
        XCTAssertFalse(p.elementScrolls.revealsInFlight)

        for replace in ["instant", "scrollTop"] {
            p.scrollElementIntoView(["home", "start", "nearest", "instant"])
            _ = run(sv, from: 0, to: 0) { ended > 0 }
            ended = 0
            p.scrollElementIntoView(["target", "start", "nearest", "smooth"])
            _ = run(sv, from: 0, to: 368) { sv.contentOffset.y > 50 }
            if replace == "instant" { p.scrollElementIntoView(["home", "start", "nearest", "instant"]) }
            else { p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "120"]]])) }
            let rest: CGFloat = replace == "instant" ? 0 : 120
            XCTAssertEqual(sv.contentOffset.y, rest, accuracy: 0.5, "\(replace) lands at once")
            XCTAssertFalse(p.elementScrolls.revealsInFlight, "\(replace) ends the reveal")
            _ = run(sv, from: rest, to: rest) { ended > 0 }
            // Past the replaced animation's own end.
            let after = Date().addingTimeInterval(uikit)
            while Date() < after { _ = RunLoop.main.run(mode: .default, before: after) }
            XCTAssertEqual(sv.contentOffset.y, rest, accuracy: 0.5, "the replaced reveal moves nothing after \(replace)")
            XCTAssertEqual(ended, 1, "\(replace): one scrollend")
        }
    }

    /// Nested scrollers animate together: the outer port aims at where the
    /// target will be once the inner one arrives, not where it is now.
    func testANestedUIKitRevealAimsTheOuterPortAtTheInnerDestination() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "UIKit animates a scroll only in a scene's window") }
        guard !ExactEnv.agentFreezes else { return }
        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["scrollend"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["scrollend"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "inner-target"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 400.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 300.0, "w": 100.0, "h": 50.0],
        ]))
        let window = UIWindow(windowScene: scene)
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        let outer = try XCTUnwrap(p.views[1]?.scrollView), inner = try XCTUnwrap(p.views[2]?.scrollView)
        outer.contentSize = CGSize(width: 200, height: 1000)
        inner.contentSize = CGSize(width: 200, height: 600)
        var ended: [UInt32] = []
        p.onScrollend = { ended.append($0) }
        p.scrollElementIntoView(["inner-target", "start", "nearest"])
        XCTAssertEqual(inner.contentOffset.y, 0, "neither lands synchronously")
        XCTAssertEqual(outer.contentOffset.y, 0)
        _ = run(outer, from: 0, to: 400) { ended.count >= 2 }
        XCTAssertEqual(inner.contentOffset.y, 300, accuracy: 0.5)
        XCTAssertEqual(outer.contentOffset.y, 400, accuracy: 0.5, "the outer port aims at the inner one's destination")
        XCTAssertEqual(ended.sorted(), [1, 2], "each scroller ends once")

        // Content moving under a running reveal (an anchor's correction)
        // sends it where the target now is, still one scroll.
        p.scrollElementIntoView(["inner-target", "start", "nearest", "instant"])
        inner.setContentOffset(.zero, animated: false)
        _ = run(inner, from: 0, to: 0) { ended.count >= 3 }
        ended = []
        p.scrollElementIntoView(["inner-target", "start", "nearest"])
        _ = run(inner, from: 0, to: 300) { inner.contentOffset.y > 30 }
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 0.0, "y": 450.0, "w": 100.0, "h": 50.0]]))
        p.elementScrolls.retarget(inner)
        XCTAssertEqual(p.elementScrolls.revealing(inner)?.y ?? inner.contentOffset.y, 450, accuracy: 0.5,
                       "the reveal heads for the moved target, including when layout already retargeted it")
        _ = run(inner, from: 0, to: 450) { ended.contains(2) }
        XCTAssertEqual(inner.contentOffset.y, 450, accuracy: 0.5)
        XCTAssertEqual(ended.filter { $0 == 2 }.count, 1, "the retargeted reveal ends once")
    }

    /// A strip of chips in a horizontal scroller (euxy's New Pattern icons):
    /// `chip-a` at x 300, `chip-b` at 600, and `chip-c`, made but not yet
    /// attached (a sheet's subtree before UIKit mounts it).
    private func strip() throws -> (Presenter, UIScrollView) {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "chip-a"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "chip-b"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "chip-c"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 2, "x": 300.0, "y": 0.0, "w": 56.0, "h": 56.0],
            ["op": "frame", "id": 3, "x": 600.0, "y": 0.0, "w": 56.0, "h": 56.0],
        ]))
        let sv = try XCTUnwrap(p.views[1]?.scrollView)
        sv.contentSize = CGSize(width: 1000, height: 60)
        return (p, sv)
    }

    /// A sheet's views enter the window a turn after the batch that made
    /// them: a request waits for its target's window, and one per element
    /// is kept; a target that loses its id meanwhile cancels its own.
    func testAScrollAskedBeforeItsTargetIsInAWindowLandsWhenItIs() throws {
        let (p, sv) = try strip()
        p.scrollElementIntoView(["chip-a", "nearest", "start", "instant"])
        p.scrollElementIntoView(["chip-b", "nearest", "start", "instant"])
        XCTAssertEqual(sv.contentOffset.x, 0, "nothing to scroll outside a window")
        XCTAssertEqual(p.elementScrolls.names, ["chip-a", "chip-b"])
        let window = UIWindow(frame: p.viewport.frame)
        defer { withExtendedLifetime(window) {} }
        window.addSubview(p.viewport)
        XCTAssertEqual(sv.contentOffset.x, 600, accuracy: 0.5, "each request lands, in order, as its target enters the window")
        XCTAssertEqual(p.elementScrolls.names, [])

        p.viewport.removeFromSuperview()
        sv.setContentOffset(.zero, animated: false)
        p.scrollElementIntoView(["chip-a", "nearest", "start", "instant"])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["id": "other"]]]))
        window.addSubview(p.viewport)
        XCTAssertEqual(sv.contentOffset.x, 0, "a target that lost its id is not scrolled to")
        XCTAssertEqual(p.elementScrolls.names, [])
    }

    /// A target mounted by a batch is scrolled to when that batch ends (its
    /// own `didMoveToWindow` comes mid-batch), and one in a window with no
    /// size yet waits for its frame; a request for another element, which
    /// lands at once, leaves the held one be.
    func testAHeldScrollLandsAtTheEndOfTheBatchThatLaysItsTargetOut() throws {
        let (p, sv) = try strip()
        let window = UIWindow(frame: p.viewport.frame)
        defer { withExtendedLifetime(window) {} }
        window.addSubview(p.viewport)
        p.scrollElementIntoView(["chip-c", "nearest", "start", "instant"])
        p.scrollElementIntoView(["chip-a", "nearest", "start", "instant"])
        XCTAssertEqual(sv.contentOffset.x, 300, accuracy: 0.5)
        XCTAssertEqual(p.elementScrolls.names, ["chip-c"], "another element's scroll keeps the held one")
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 3, 4]]]))
        XCTAssertEqual(p.elementScrolls.names, ["chip-c"], "mounted with no size: it still waits")
        sv.contentSize = CGSize(width: 1000, height: 60)
        p.apply(wireBatch([["op": "frame", "id": 4, "x": 800.0, "y": 0.0, "w": 56.0, "h": 56.0]]))
        XCTAssertEqual(sv.contentOffset.x, 800, accuracy: 0.5, "laid out by a batch: scrolled to as it ends")
        XCTAssertEqual(p.elementScrolls.names, [])
    }

    /// The person taking the scroller a request would move drops it, and
    /// it stays dropped: the target laid out after the gesture is not
    /// scrolled to.
    func testAHeldScrollYieldsToThePersonsDrag() throws {
        let (p, sv) = try strip()
        let window = UIWindow(frame: p.viewport.frame)
        defer { withExtendedLifetime(window) {} }
        window.addSubview(p.viewport)
        // In the strip, with no size yet: held.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 3, 4]]]))
        p.scrollElementIntoView(["chip-c", "nearest", "start", "instant"])
        XCTAssertEqual(p.elementScrolls.names, ["chip-c"])
        try XCTUnwrap(p.views[1]).scrollViewWillBeginDragging(sv)
        XCTAssertEqual(p.elementScrolls.names, [], "the drag drops it")
        sv.contentSize = CGSize(width: 1000, height: 60)
        p.apply(wireBatch([["op": "frame", "id": 4, "x": 800.0, "y": 0.0, "w": 56.0, "h": 56.0]]))
        XCTAssertEqual(sv.contentOffset.x, 0, accuracy: 0.5, "nothing scrolls the strip back after the gesture")
    }

    /// A navigation's settle carries out a held request, as a sheet's fit
    /// does, rather than leaving it to some later, unrelated batch.
    func testANavigationSettleCarriesOutAHeldScroll() throws {
        let session = ExactApp.shared.makeSession(label: "element-scroll-navigation")
        defer { session.destroy() }
        let view = ExactView(session: session), host = UIViewController()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 400))
        window.rootViewController = host
        window.isHidden = false
        host.view.addSubview(view)
        view.frame = host.view.bounds
        defer { window.isHidden = true }
        // The view finishes mounting (its navigation's `mounted`) at layout.
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "chip-a"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "chip-c"]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 2, "x": 300.0, "y": 0.0, "w": 56.0, "h": 56.0],
        ]))
        let sv = try XCTUnwrap(p.views[1]?.scrollView)
        sv.contentSize = CGSize(width: 1000, height: 60)
        let chip = try XCTUnwrap(p.views[4])
        XCTAssertNotNil(chip.window, "the strip is in the window")
        p.scrollElementIntoView(["chip-c", "nearest", "start", "instant"])
        XCTAssertEqual(p.elementScrolls.names, ["chip-c"], "no size yet: held")
        // Laid out with no batch, as UIKit's own layout at a transition's end.
        chip.frame = CGRect(x: 800, y: 0, width: 56, height: 56)
        XCTAssertNotNil(p.session?.view?.window, "the session's view is in the window")
        XCTAssertTrue(p.canScrollIntoView(chip), "laid out")
        XCTAssertFalse(p.settlesElementScroll, "nothing is settling")
        let calls = p.navigation.syncCalls
        p.navigation.sync(wireBatch([]))
        XCTAssertEqual(p.navigation.syncCalls, calls + 1)
        XCTAssertEqual(sv.contentOffset.x, 800, accuracy: 0.5, "the settle carries it out")
        XCTAssertEqual(p.elementScrolls.names, [])
    }

    /// A held request goes with its target: recycled into another row (a
    /// new incarnation, LLP 1068 §4.9), inside a sheet or dialog that
    /// closes, or a presenter that resets; none of them scrolls.
    func testAHeldScrollIsCancelledWithItsTarget() throws {
        let (p, sv) = try strip()
        p.scrollElementIntoView(["chip-b", "nearest", "start", "instant"])
        try XCTUnwrap(p.views[3]).incarnation = NodePool.issue()
        let window = UIWindow(frame: p.viewport.frame)
        defer { withExtendedLifetime(window) {} }
        window.addSubview(p.viewport)
        XCTAssertEqual(sv.contentOffset.x, 0, "a recycled target is not scrolled to")
        XCTAssertEqual(p.elementScrolls.names, [])

        p.scrollElementIntoView(["chip-c", "nearest", "start", "instant"])
        XCTAssertEqual(p.elementScrolls.names, ["chip-c"])
        p.elementScrolls.cancel(inside: try XCTUnwrap(p.views[4]))
        XCTAssertEqual(p.elementScrolls.names, [], "a closing sheet's requests go with it")
        p.scrollElementIntoView(["chip-c", "nearest", "start", "instant"])
        p.reset()
        XCTAssertEqual(p.elementScrolls.names, [], "a new document holds none of the old one's")
    }
    #endif
}

#if os(macOS)
private final class FlippedReferenceView: NSView {
    override var isFlipped: Bool { true }
}
#endif
