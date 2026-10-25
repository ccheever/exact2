// @ref LLP 1069.000 D2 — `visibilityState` on the Mac, read of the machine,
// and `persisted` and `restores` for a window its close hid (LLP 1069.010 D8).
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// Nothing of the app can be seen when every window on screen or in the
/// Dock is minimized or fully occluded. Chrome and Electron read a window
/// minimized with ⌘M as `hidden` and fire `visibilitychange`; AppKit's
/// `isVisible` is false for a minimized window, so it used to be left out
/// and an app whose one window was minimized read `visible`.
final class PageFactsMacTests: XCTestCase {
    private func window(_ x: CGFloat) -> NSWindow {
        let made = NSWindow(contentRect: NSRect(x: x, y: 100, width: 300, height: 200), styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
        made.isReleasedWhenClosed = false
        return made
    }
    /// A run-loop pass at a time until `done`, for at most ten seconds.
    private func wait(_ done: () -> Bool) {
        let end = Date().addingTimeInterval(10)
        while !done(), Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
    }

    func testAMinimizedWindowIsHiddenAndBothChangesAreHeard() {
        _ = NSApplication.shared
        XCTAssertFalse(PageFacts.substituted, "the machine's own readings: no agent's stand in")
        let only = window(100)
        defer { only.close() }
        only.orderFront(nil)
        // On screen it is hidden only when occluded (a locked screen occludes every window).
        XCTAssertEqual(PageFacts.hidden, !only.occlusionState.contains(.visible))
        var heard: [Bool] = []
        let tokens = PageFacts.observe { heard.append(PageFacts.hidden) }
        defer { PageFacts.forget(tokens) }

        only.miniaturize(nil)
        wait { only.isMiniaturized && heard.last == true }
        XCTAssertTrue(only.isMiniaturized, "AppKit minimized the window")
        XCTAssertTrue(PageFacts.hidden, "with its only window in the Dock nothing of the app is seen")
        XCTAssertEqual(heard.last, true, "the minimize is told to the page: \(heard)")
        XCTAssertFalse(PageFacts.persisted(only), "a minimized page is hidden, not put away: no pagehide")

        // Another window on screen: the app reads as that window's occlusion says.
        let other = window(500)
        defer { other.close() }
        other.orderFront(nil)
        wait { other.isVisible }
        XCTAssertEqual(PageFacts.hidden, !other.occlusionState.contains(.visible), "a window on screen beside the minimized one")
        other.orderOut(nil)
        XCTAssertTrue(PageFacts.hidden, "a window ordered out counts for nothing")

        heard = []
        only.deminiaturize(nil)
        wait { !only.isMiniaturized && !heard.isEmpty }
        XCTAssertFalse(only.isMiniaturized, "AppKit restored the window")
        XCTAssertFalse(heard.isEmpty, "the restore is told to the page")
        XCTAssertEqual(PageFacts.hidden, !only.occlusionState.contains(.visible), "restored, it reads as its occlusion says")
        // Only the transitions: persisted (1024) and the restores (the high half); the low bits are facts.
        XCTAssertEqual(PageFacts.bits(nil, page: only, readsOnLine: false) & (1024 | 0xFFFF << 16), 0, "nor a pageshow: nothing put away, no restore")
    }

    /// LLP 1069.010 D8: the one window of an app that keeps running is
    /// ordered out by its close, its page kept. The page reads `hidden` and
    /// hears it, and hears the reopen that shows it again. A window ordered
    /// out otherwise, or not yet shown, counts for nothing, so a page boots
    /// visible; a window closed for good is forgotten.
    func testAWindowItsCloseHidIsHiddenAndBothChangesAreHeard() {
        _ = NSApplication.shared
        let unshown = window(900)
        defer { unshown.close() }
        XCTAssertFalse(PageFacts.hidden || unshown.isVisible, "a window not yet shown leaves the app visible")
        let only = window(100)
        defer { only.close() }
        only.orderFront(nil)
        wait { only.isVisible }
        let onScreen = !only.occlusionState.contains(.visible)
        XCTAssertEqual(PageFacts.hidden, onScreen)
        var heard: [Bool] = []
        let tokens = PageFacts.observe { heard.append(PageFacts.hidden) }
        defer { PageFacts.forget(tokens) }

        XCTAssertFalse(PageFacts.persisted(only) || PageFacts.restores(only) != 0, "on screen: never put away")
        XCTAssertTrue(ExactDocuments.hidesOnClose(only, only: true, keepsRunning: true))
        XCTAssertFalse(only.isVisible || only.isMiniaturized, "ordered out: neither on screen nor in the Dock")
        XCTAssertTrue(PageFacts.hidden, "with its only window hidden by its close nothing of the app is seen")
        XCTAssertEqual(heard.last, true, "the hide is told to the page at once: \(heard)")
        XCTAssertTrue(PageFacts.persisted(only), "the page is put away: pagehide, persisted")
        XCTAssertEqual(PageFacts.bits(nil, page: only, readsOnLine: false) & 1024, 1024, "bit 10 on the wire")
        XCTAssertFalse(PageFacts.persisted(unshown), "only the page whose window the close hid")

        heard = []
        only.makeKeyAndOrderFront(nil) // the reopen's `front()`
        wait { !heard.isEmpty && heard.last == !only.occlusionState.contains(.visible) }
        XCTAssertFalse(heard.isEmpty, "the reopen is told to the page")
        XCTAssertEqual(PageFacts.hidden, !only.occlusionState.contains(.visible), "shown again, it reads as its occlusion says")
        XCTAssertFalse(PageFacts.persisted(only), "no longer put away")
        XCTAssertEqual(PageFacts.restores(only), 1, "pageshow, persisted: the page came back once")
        XCTAssertEqual(PageFacts.bits(nil, page: only, readsOnLine: false) >> 16, 1, "the count above bit 16 on the wire")

        // Minimized and restored after the return: neither a pagehide nor a pageshow.
        only.miniaturize(nil)
        wait { only.isMiniaturized }
        XCTAssertFalse(PageFacts.persisted(only), "a minimize puts nothing away")
        only.deminiaturize(nil)
        wait { !only.isMiniaturized }
        XCTAssertEqual(PageFacts.restores(only), 1, "a restore from the Dock is no return from the cache")

        // The same window hidden again, then closed for good while hidden: it counts for nothing.
        _ = ExactDocuments.hidesOnClose(only, only: true, keepsRunning: true)
        XCTAssertTrue(PageFacts.hidden)
        XCTAssertTrue(PageFacts.persisted(only))
        PageFacts.closed(only)
        only.close()
        XCTAssertFalse(PageFacts.hidden, "a window closed for good, like one never shown, leaves the app visible")
        XCTAssertFalse(PageFacts.persisted(only) || PageFacts.restores(only) != 0, "and is forgotten")
    }
}
#endif
