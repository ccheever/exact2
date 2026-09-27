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
        // A table name drawn as a stand-in is silent; the agent's state shows it.
        let standIn = try XCTUnwrap(rows.first { $0[column].hasPrefix("~") }?[0])
        XCTAssertNotNil(Materials.resolve(standIn) { logged.append($0) })
        XCTAssertEqual(logged.count, 1, "a stand-in is not logged (Charlie: \"ok, (b)\")")
        XCTAssertEqual(Materials.agentMaterial(standIn)["standIn"] as? Bool, true)
        XCTAssertEqual(Materials.agentMaterial("not-a-material")["unknown"] as? Bool, true)
    }
}

#if os(macOS)
final class MaterialTableTests: XCTestCase {
    func testEveryMaterialIsAnAppKitOne() throws {
        try MaterialTable.check(2) { Materials.material($0) != nil }
    }
}
#endif
