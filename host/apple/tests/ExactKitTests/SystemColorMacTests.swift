#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
import ExactCore

extension NSColor {
    /// Named like a colour, returning none: what the lookup must not `perform`.
    @objc class func exactTestNumberColor() -> Int { 3 }
    /// A colour by a class method Apple does not document: what the lookup must not reach.
    @objc class func exactTestUndocumentedColor() -> NSColor { .systemRed }
}

/// LLP 1095 D5 on AppKit: a system colour change (the accent) makes every
/// resolution again, and what was resolved into a symbol with it.
final class SystemColorMacTests: XCTestCase {
    func testAColourNamedMethodReturningNoObjectIsTheFallback() {
        XCTAssertEqual(SystemColor.channels("exactTestNumberColor", dark: false, fallback: [1, 2, 3, 255]), [1, 2, 3, 255])
    }

    /// Only a documented class colour resolves (`SystemColorNames`): one by
    /// any other class method, a colour though it is, is the fallback.
    func testAnUndocumentedColourIsTheFallback() {
        XCTAssertEqual(SystemColor.channels("exactTestUndocumentedColor", dark: false, fallback: [1, 2, 3, 255]), [1, 2, 3, 255])
        XCTAssertNil(SystemColor.native(.object(["sys": .string("exactTestUndocumentedColor")])))
        XCTAssertNotEqual(SystemColor.channels("systemRedColor", dark: false, fallback: [1, 2, 3, 255]), [1, 2, 3, 255])
    }

    /// Every class colour the role table names for this platform, and those
    /// the repo's apps and docs write in `-exact-platform-color()`, is on
    /// the documented list.
    func testEveryColourTheRepoNamesIsDocumented() throws {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../kernel/tables/schema.json").standardized
        let schema = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        let rows = try XCTUnwrap(schema?["colors"] as? [[String]])
        let named = rows.map { String($0[3].split(separator: "/")[0]) }.filter { !$0.hasPrefix("@") && $0 != "exactGroupedCardColor" }
        XCTAssertGreaterThan(named.count, 20)
        for name in named + ["systemTealColor", "systemMintColor", "systemRedColor", "systemBlueColor", "labelColor", "secondaryLabelColor"] { XCTAssertNotNil(SystemColorNames.color(name), name) }
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
        XCTAssertEqual(n.symbolLookKey, key, "invalidating paint does not replace a symbol")
    }
}
#endif
