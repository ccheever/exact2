import XCTest
@testable import ExactKit

/// LLP 1053.000 D4: every row of the schema's `materials` table reaches this
/// platform as the name the table gives, and that name is one this platform
/// draws (a glass effect, or a `UIBlurEffect.Style` / `NSVisualEffectView.Material`).
enum MaterialTable {
    static func rows() throws -> [[String]] {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../kernel/tables/schema.json").standardized
        let schema = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        return try XCTUnwrap(schema?["materials"] as? [[String]])
    }

    static func check(_ column: Int, draws: (String) -> Bool) throws {
        let rows = try rows()
        XCTAssertGreaterThan(rows.count, 30)
        for row in rows {
            let (apple, standIn) = try XCTUnwrap(Materials.platform(row[0]), row[0])
            XCTAssertEqual((standIn ? "~" : "") + apple, row[column], row[0])
            XCTAssertTrue(apple == "glass" || apple == "glassClear" || draws(apple), "\(row[0]): \(apple)")
        }
        XCTAssertNil(Materials.platform("not-a-material"))
        var logged: [String] = []
        XCTAssertEqual(Materials.resolve("not-a-material-\(column)") { logged.append($0) }, Materials.platform("ultra-thin")!.apple)
        XCTAssertEqual(Materials.resolve("not-a-material-\(column)") { logged.append($0) }, Materials.platform("ultra-thin")!.apple)
        XCTAssertEqual(logged.count, 1, "said once")
    }
}

#if os(macOS)
final class MaterialTableTests: XCTestCase {
    func testEveryMaterialIsAnAppKitOne() throws {
        try MaterialTable.check(2) { Materials.material($0) != nil }
    }
}
#endif
