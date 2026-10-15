import AppKit
import XCTest

// In real full screen AppKit drew T3WindowChrome's 52-pt toolbar as an opaque
// band over the title row, as a person's real full screen showed. The toolbar leaves as the window enters full screen and
// comes back when it is windowed again; the window's own notifications drive it.
final class WindowChromeFullScreenTests: XCTestCase {
    func window() -> NSWindow {
        _ = NSApplication.shared
        return NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1280, height: 840),
                        styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                        backing: .buffered, defer: true)
    }

    /// Posts `name` for `window` and runs the main queue until `done` holds (observers use `queue: .main`).
    func post(_ name: Notification.Name, _ window: NSWindow, until done: () -> Bool) {
        NotificationCenter.default.post(name: name, object: window)
        let limit = Date().addingTimeInterval(5)
        while !done() && Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
    }

    func testToolbarLeavesForFullScreenAndReturnsWindowed() {
        let window = window(), chrome = T3WindowChrome()
        chrome.attach(window)
        let toolbar = window.toolbar
        XCTAssertEqual(toolbar?.identifier, "com.exact.t3code.titlebar", "windowed: the title row's toolbar")
        XCTAssertEqual(window.toolbarStyle, .unified)

        post(NSWindow.willEnterFullScreenNotification, window) { window.toolbar == nil }
        XCTAssertNil(window.toolbar, "entering full screen: no toolbar band over the title row")

        post(NSWindow.didExitFullScreenNotification, window) { window.toolbar != nil }
        XCTAssertTrue(window.toolbar === toolbar, "windowed again: the same toolbar is back")
        XCTAssertEqual(window.standardWindowButton(.closeButton)?.frame.minX, 16, "the traffic lights keep their inset")
        chrome.destroy()
    }

    func testDestroyInFullScreenStopsFollowingTheWindow() {
        let window = window(), chrome = T3WindowChrome()
        chrome.attach(window)
        post(NSWindow.willEnterFullScreenNotification, window) { window.toolbar == nil }
        chrome.destroy()
        NotificationCenter.default.post(name: NSWindow.didExitFullScreenNotification, object: window)
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        XCTAssertNil(window.toolbar, "a destroyed chrome puts nothing back")
    }

    func testAttachingTheSameWindowAgainKeepsOneToolbar() {
        let window = window(), chrome = T3WindowChrome()
        chrome.attach(window)
        let toolbar = window.toolbar
        chrome.attach(window)
        XCTAssertTrue(window.toolbar === toolbar)
        chrome.destroy()
        XCTAssertNil(window.toolbar)
    }
}

let suite = XCTestSuite(forTestCaseClass: WindowChromeFullScreenTests.self)
suite.run()
let run = suite.testRun!
print("Window chrome full-screen tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
