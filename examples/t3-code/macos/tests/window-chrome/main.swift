import AppKit
import XCTest

// The tall title row is the host's (app.json `display_override`): T3WindowChrome installs no toolbar and keeps
// the traffic lights at the reference's x = 16 through a resize and a return from full screen, which lay them out
// again; the window's own notifications drive it.
final class WindowChromeTests: XCTestCase {
    func window() -> NSWindow {
        _ = NSApplication.shared
        return NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1280, height: 840),
                        styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                        backing: .buffered, defer: true)
    }
    var closeX: (NSWindow) -> CGFloat = { $0.standardWindowButton(.closeButton)?.frame.minX ?? -1 }

    /// Posts `name` for `window` and runs the main queue until `done` holds (observers use `queue: .main`).
    func post(_ name: Notification.Name, _ window: NSWindow, until done: () -> Bool) {
        NotificationCenter.default.post(name: name, object: window)
        let limit = Date().addingTimeInterval(5)
        while !done() && Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
    }
    func moveLights(_ window: NSWindow, to x: CGFloat) {
        for kind: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
            guard let button = window.standardWindowButton(kind) else { continue }
            button.setFrameOrigin(NSPoint(x: button.frame.minX + x - closeX(window), y: button.frame.minY))
        }
    }

    func testTheLightsKeepTheReferencesInsetAndNoToolbarIsInstalled() {
        let window = window(), chrome = T3WindowChrome()
        chrome.attach(window)
        XCTAssertNil(window.toolbar, "the host's toolbar, not the module's")
        moveLights(window, to: 20)
        post(NSWindow.didResizeNotification, window) { closeX(window) == 16 }
        XCTAssertEqual(closeX(window), 16, accuracy: 0.5, "a resize")
        moveLights(window, to: 20)
        post(NSWindow.didExitFullScreenNotification, window) { closeX(window) == 16 }
        XCTAssertEqual(closeX(window), 16, accuracy: 0.5, "back from full screen")
        chrome.destroy()
    }

    func testDestroyStopsFollowingTheWindow() {
        let window = window(), chrome = T3WindowChrome()
        chrome.attach(window)
        chrome.destroy()
        moveLights(window, to: 20)
        NotificationCenter.default.post(name: NSWindow.didResizeNotification, object: window)
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        XCTAssertEqual(closeX(window), 20, accuracy: 0.5)
    }
}

let suite = XCTestSuite(forTestCaseClass: WindowChromeTests.self)
suite.run()
let run = suite.testRun!
print("Window chrome tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
