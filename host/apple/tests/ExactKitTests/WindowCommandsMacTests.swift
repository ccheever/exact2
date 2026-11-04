#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1069.010: a quit the app holds fades its window and takes it out of
/// full screen first, `setWindowOpacity(opacity)` and `exitFullscreen()`.
@MainActor
final class WindowCommandsMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    private func shown() -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 320, height: 240), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.orderFrontRegardless()
        self.window = window
        return window
    }

    func testAnOpacityFadesTheWindowAndARefusalLeavesIt() {
        let window = shown()
        var said: [String] = []
        XCTAssertTrue(ExactWindowCommands.perform("setWindowOpacity", [NSNumber(value: 0)], on: window) { said.append($0) })
        XCTAssertEqual(window.alphaValue, 0)
        XCTAssertTrue(window.isVisible, "an unseen window is still shown, and still hears keys")
        XCTAssertTrue(ExactWindowCommands.perform("setWindowOpacity", [NSNumber(value: 0.5)], on: window) { said.append($0) })
        XCTAssertEqual(window.alphaValue, 0.5, accuracy: 0.001)
        XCTAssertTrue(ExactWindowCommands.perform("setWindowOpacity", [NSNumber(value: 3)], on: window) { said.append($0) })
        XCTAssertEqual(window.alphaValue, 1, "clamped to 0–1")
        XCTAssertTrue(ExactWindowCommands.perform("setWindowOpacity", ["0"], on: window) { said.append($0) })
        XCTAssertEqual(window.alphaValue, 1)
        XCTAssertEqual(said, ["setWindowOpacity refused: the opacity is a number from 0 to 1"])
        XCTAssertFalse(ExactWindowCommands.perform("close", [], on: window) { said.append($0) }, "not a window command of these")
    }

    func testLeavingFullScreenLeavesAWindowThatIsNotFullScreenAsItIs() {
        let window = shown()
        let frame = window.frame
        XCTAssertTrue(ExactWindowCommands.perform("exitFullscreen", [], on: window) { XCTFail($0) })
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.3))
        XCTAssertFalse(window.styleMask.contains(.fullScreen))
        XCTAssertEqual(window.frame, frame)
    }

    func testAFullScreenWindowLeavesItsSpace() throws {
        let window = shown()
        window.collectionBehavior = [.fullScreenPrimary]
        // A toggle while AppKit animates into the space is ignored, so the
        // command waits for the space to be entered.
        let entered = expectation(forNotification: NSWindow.didEnterFullScreenNotification, object: window)
        window.toggleFullScreen(nil)
        guard XCTWaiter().wait(for: [entered], timeout: 10) == .completed else { throw XCTSkip("this session gives no window a full-screen space") }
        let left = expectation(forNotification: NSWindow.didExitFullScreenNotification, object: window)
        XCTAssertTrue(ExactWindowCommands.perform("exitFullscreen", [], on: window) { XCTFail($0) })
        wait(for: [left], timeout: 10)
        XCTAssertFalse(window.styleMask.contains(.fullScreen))
    }
}
#endif
