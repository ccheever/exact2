#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 D7.9: a window the adapter makes opens with AppKit's own
/// animation (it was told `.none`), and window tabs are for an app whose
/// documents have windows of their own: a one-window app offers none, where
/// every window used to carry a tabbing identifier and AppKit's menus
/// offered Show Tab Bar.
final class WindowMannersMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    override func tearDown() {
        windows.forEach { $0.close() }
        windows = []
        NSWindow.allowsAutomaticWindowTabbing = true // AppKit's own, for the tests after this
    }
    private func window() -> NSWindow {
        _ = NSApplication.shared
        let made = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        made.isReleasedWhenClosed = false
        windows.append(made)
        return made
    }

    func testAWindowOpensAsAppKitOpensOneAndAOneWindowAppOffersNoTabs() {
        let appKit = window(), one = window(), documents = window()
        ExactDocuments.furnish(one, tabs: false)
        XCTAssertEqual(one.animationBehavior, appKit.animationBehavior, "AppKit's own opening animation")
        XCTAssertNotEqual(one.animationBehavior, .none)
        XCTAssertFalse(NSWindow.allowsAutomaticWindowTabbing, "no Show Tab Bar in a one-window app's menus")
        XCTAssertEqual(one.tabbingMode, .disallowed)
        XCTAssertEqual(one.tabbingIdentifier, appKit.tabbingIdentifier, "and no identifier of the app's to gather windows under")
        // An app whose documents have their own windows keeps AppKit's tabs.
        ExactDocuments.furnish(documents, tabs: true)
        XCTAssertTrue(NSWindow.allowsAutomaticWindowTabbing)
        XCTAssertEqual(documents.tabbingMode, .automatic)
        XCTAssertEqual(documents.tabbingIdentifier, ExactEnv.appMetadata["CFBundleIdentifier"] as? String ?? ExactEnv.appName)
        XCTAssertEqual(documents.animationBehavior, appKit.animationBehavior)
        // With nothing said, the manifest's launch mode decides; a test has none, so one window.
        XCTAssertNotEqual(ExactDocuments.launchMode, "navigate-new")
        ExactDocuments.furnish(documents)
        XCTAssertFalse(NSWindow.allowsAutomaticWindowTabbing)
        XCTAssertEqual(documents.tabbingMode, .disallowed)
    }
}
#endif
