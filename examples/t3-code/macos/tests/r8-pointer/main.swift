import AppKit
import XCTest

// Lane r8-pointer with real AppKit: the window frame record (D14) survives the
// host's launch order (frame restored, then full-size content inserted, which
// keeps the content rect and drops the 32 pt titlebar from the frame).
final class R8PointerTests: XCTestCase {
    let screens = { [NSRect(x: 0, y: 0, width: 3000, height: 2000)] }

    func window(_ frame: NSRect) -> NSWindow {
        let window = NSWindow(contentRect: frame, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.setFrame(frame, display: false)
        return window
    }

    func testHostStyleChangeNoLongerShrinksTheWindow() {
        _ = NSApplication.shared
        var saved: String? = NSStringFromRect(NSRect(x: 120, y: 90, width: 1280, height: 818))
        let keeper = R8WindowFrame(load: { saved }, store: { saved = $0 }, screens: screens)
        // The host restored its autosave (818) …
        let window = window(NSRect(x: 120, y: 90, width: 1280, height: 818))
        var settle: (() -> Void)?
        keeper.attach(window) { settle = $0 }
        // … then viewport-fit=cover inserts full-size content: AppKit keeps the content rect.
        window.styleMask.insert(.fullSizeContentView)
        XCTAssertLessThan(window.frame.height, 818, "AppKit drops the titlebar from the frame (the bug's cause)")
        XCTAssertFalse(keeper.tracking, "nothing is recorded before the launch settles")
        XCTAssertEqual(saved, NSStringFromRect(NSRect(x: 120, y: 90, width: 1280, height: 818)))
        settle?()
        XCTAssertEqual(window.frame, NSRect(x: 120, y: 90, width: 1280, height: 818))
        XCTAssertTrue(keeper.tracking)
        // Moves and resizes are recorded; the next launch starts from them.
        window.setFrame(NSRect(x: 200, y: 150, width: 1100, height: 760), display: false)
        XCTAssertEqual(saved, NSStringFromRect(NSRect(x: 200, y: 150, width: 1100, height: 760)))
        keeper.detach()
        window.setFrame(NSRect(x: 10, y: 10, width: 900, height: 700), display: false)
        XCTAssertEqual(saved, NSStringFromRect(NSRect(x: 200, y: 150, width: 1100, height: 760)), "a detached keeper records nothing")
        window.close()
    }

    func testFirstLaunchRecordsTheHostFrame() {
        _ = NSApplication.shared
        var saved: String?
        let keeper = R8WindowFrame(load: { saved }, store: { saved = $0 }, screens: screens)
        let window = window(NSRect(x: 300, y: 200, width: 1280, height: 840))
        keeper.attach(window) { $0() }
        XCTAssertEqual(window.frame, NSRect(x: 300, y: 200, width: 1280, height: 840))
        XCTAssertEqual(saved, NSStringFromRect(window.frame))
        window.close()
    }

    func testUnusableRecordsAreIgnored() {
        let screen = [NSRect(x: 0, y: 0, width: 1440, height: 900)]
        XCTAssertNil(R8WindowFrame.usable(nil, screens: screen))
        XCTAssertNil(R8WindowFrame.usable("", screens: screen))
        XCTAssertNil(R8WindowFrame.usable(NSStringFromRect(NSRect(x: 5000, y: 5000, width: 1280, height: 818)), screens: screen), "off every screen")
        XCTAssertNil(R8WindowFrame.usable(NSStringFromRect(NSRect(x: 0, y: 0, width: 120, height: 818)), screens: screen), "too small")
        XCTAssertNotNil(R8WindowFrame.usable(NSStringFromRect(NSRect(x: 100, y: 40, width: 1280, height: 818)), screens: screen))
    }
}

let suite = XCTestSuite(forTestCaseClass: R8PointerTests.self)
suite.run()
let run = suite.testRun!
print("R8Pointer tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
