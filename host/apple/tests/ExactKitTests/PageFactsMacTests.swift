// @ref LLP 1069.000 D2 — `visibilityState` on the Mac, read of the machine.
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
    }
}
#endif
