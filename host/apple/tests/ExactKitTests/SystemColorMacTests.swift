#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

extension NSColor {
    /// Named like a colour, returning none: what the lookup must not `perform`.
    @objc class func exactTestNumberColor() -> Int { 3 }
}

/// LLP 1095 D5 on AppKit: a system colour change (the accent) makes every
/// resolution again, and what was resolved into a symbol with it.
final class SystemColorMacTests: XCTestCase {
    func testAColourNamedMethodReturningNoObjectIsTheFallback() {
        XCTAssertEqual(SystemColor.channels("exactTestNumberColor", dark: false, fallback: [1, 2, 3, 255]), [1, 2, 3, 255])
    }

    func testASystemColourChangeMakesResolutionsAgain() {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "system-colours-mac")
        defer { session.destroy() }
        let n = NodeView(id: 1, kind: "view", presenter: session.presenter)
        let key = n.symbolLookKey
        let before = SystemColor.generation
        NotificationCenter.default.post(name: NSColor.systemColorsDidChangeNotification, object: nil)
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertGreaterThan(SystemColor.generation, before, "the accent's cached channels are gone")
        XCTAssertNotEqual(n.symbolLookKey, key, "a palette resolved with the old accent is made again")
    }
}
#endif
