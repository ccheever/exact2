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
        #if os(macOS)
        sv.contentView.scroll(to: .zero)
        window.orderFront(nil)
        #else
        sv.setContentOffset(.zero, animated: false)
        window.makeKeyAndVisible()
        #endif
        var ended = 0
        #if os(macOS)
        var intermediate = false
        #endif
        p.onScrollend = { _ in ended += 1 }
        p.scrollElementIntoView(["target", "start", "nearest", "smooth"])
        #if os(macOS)
        if ExactEnv.agentFreezes {
            XCTAssertEqual(offset(), 368, accuracy: 0.5, "the frozen agent lands authored motion immediately")
        } else {
            XCTAssertLessThan(offset(), 368, "a smooth element jump must not land synchronously")
        }
        #else
        XCTAssertEqual(offset(), 368, accuracy: 0.5, "UIKit falls back to an instant element jump (LLP 1070.000 §14)")
        #endif
        let limit = Date().addingTimeInterval(10)
        while ended == 0 && Date() < limit {
            #if os(macOS)
            if offset() > 1 && offset() < 367 { intermediate = true }
            #endif
            _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
        }
        #if os(macOS)
        if !ExactEnv.agentFreezes {
            XCTAssertTrue(intermediate, "the real-time scroll has intermediate frames")
        }
        #endif
        XCTAssertEqual(offset(), 368, accuracy: 0.5)
        XCTAssertEqual(ended, 1)
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
        let appKit = observeAnimation(offset: { reference.contentView.bounds.minY }, target: target.y) { completed in
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

        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scroll_behavior": "smooth"]]]))
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
            p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scroll_behavior": behavior]]]))
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
        let observed = observeAnimation(offset: { scroll.contentView.bounds.minY }, target: target) { completed in
            p.onScrollend = { _ in ends += 1; completed() }
            p.scrollElementIntoView(arguments)
        }
        return (observed.first, observed.intermediate, observed.last, ends)
    }

    private func observeAnimation(offset: () -> CGFloat, target: CGFloat,
                                  start: (@escaping () -> Void) -> Void) -> (first: CGFloat, intermediate: Bool, last: CGFloat) {
        var completed = false
        start { completed = true }
        let first = offset()
        var intermediate = false
        let limit = Date().addingTimeInterval(10)
        while !completed && Date() < limit {
            let position = offset()
            let low = min(first, target), high = max(first, target)
            if position > low + 1 && position < high - 1 { intermediate = true }
            _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
        }
        XCTAssertTrue(completed, "the scroll completes")
        return (first, intermediate, offset())
    }
    #else
    /// A sheet's views enter the window a turn after the batch that made
    /// them, when UIKit starts presenting it (euxy's New Pattern icon strip,
    /// whose preselected icon an answer landing in that turn named): the
    /// request waits for the target's window, the latest one wins, and a
    /// target replaced meanwhile cancels it.
    func testAScrollAskedBeforeItsTargetIsInAWindowLandsWhenItIs() throws {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "chip-a"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "chip-b"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 2, "x": 300.0, "y": 0.0, "w": 56.0, "h": 56.0],
            ["op": "frame", "id": 3, "x": 600.0, "y": 0.0, "w": 56.0, "h": 56.0],
        ]))
        let sv = try XCTUnwrap(p.views[1]?.scrollView)
        sv.contentSize = CGSize(width: 1000, height: 60)
        p.scrollElementIntoView(["chip-a", "nearest", "start", "instant"])
        p.scrollElementIntoView(["chip-b", "nearest", "start", "instant"])
        XCTAssertEqual(sv.contentOffset.x, 0, "nothing to scroll outside a window")
        let window = UIWindow(frame: p.viewport.frame)
        defer { withExtendedLifetime(window) {} }
        window.addSubview(p.viewport)
        XCTAssertEqual(sv.contentOffset.x, 600, accuracy: 0.5, "the latest request lands as its target enters the window")
        XCTAssertNil(p.pendingElementScroll)

        p.viewport.removeFromSuperview()
        sv.setContentOffset(.zero, animated: false)
        p.scrollElementIntoView(["chip-a", "nearest", "start", "instant"])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["id": "other"]]]))
        window.addSubview(p.viewport)
        XCTAssertEqual(sv.contentOffset.x, 0, "a target that lost its id is not scrolled to")
        XCTAssertNil(p.pendingElementScroll)
    }
    #endif
}

#if os(macOS)
private final class FlippedReferenceView: NSView {
    override var isFlipped: Bool { true }
}
#endif
