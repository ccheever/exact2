import AppKit
import XCTest

// Lane r8-pointer (D14) with real AppKit: the window comes back where and as large as
// it was left. The host restores its frame autosave once the window has its final
// style (exact2 #113, main #164), so the app keeps no frame record of its own; what
// is left to the app is that its own title row (T3WindowChrome's empty unified
// toolbar, added when the composer first mounts) does not change that frame.
final class R8PointerTests: XCTestCase {
    /// The window as the host leaves it after launch: `viewport-fit=cover`'s full-size
    /// content, then the saved frame put back.
    func window(_ frame: NSRect) -> NSWindow {
        let window = NSWindow(contentRect: frame, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.styleMask.insert(.fullSizeContentView)
        window.titlebarAppearsTransparent = true
        window.setFrame(frame, display: false)
        return window
    }

    func testTheTitleRowKeepsTheRestoredFrame() {
        _ = NSApplication.shared
        let saved = NSRect(x: 120, y: 90, width: 1280, height: 818)
        let window = window(saved)
        XCTAssertEqual(window.frame, saved)
        let toolbar = T3WindowChrome.titleRow(window)
        XCTAssertTrue(window.toolbar === toolbar)
        XCTAssertEqual(window.toolbarStyle, .unified)
        XCTAssertEqual(window.frame, saved, "the title row neither grows nor shrinks the frame the host restored")
        XCTAssertEqual(window.contentView?.frame.height, saved.height, "the content still fills the window under the title row")
        window.close()
    }

    func testTheTitleRowKeepsAResizedFrame() {
        _ = NSApplication.shared
        let window = window(NSRect(x: 300, y: 200, width: 1280, height: 840))
        T3WindowChrome.titleRow(window)
        let moved = NSRect(x: 200, y: 150, width: 1100, height: 760)
        window.setFrame(moved, display: false)
        // A second composer mount (a new window session) installs the row again.
        T3WindowChrome.titleRow(window)
        XCTAssertEqual(window.frame, moved)
        window.close()
    }
}

let suite = XCTestSuite(forTestCaseClass: R8PointerTests.self)
suite.run()
let run = suite.testRun!
print("R8Pointer tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
