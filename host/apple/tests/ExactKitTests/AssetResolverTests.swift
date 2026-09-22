import XCTest
@testable import ExactKit

final class AssetResolverTests: XCTestCase {
    func testPrefixNamesMaterializeReloadAndCleanUpIndependently() throws {
        var resolver: AssetResolver? = try AssetResolver(root: URL(fileURLWithPath: "/unused"), verified: ["assets/a.tex": Data([1]), "assets/a.tex/x.tex": Data([2])])
        let a = try XCTUnwrap(resolver?.url("assets/a.tex")), b = try XCTUnwrap(resolver?.url("assets/a.tex/x.tex"))
        let encoded = Data("assets/a.tex".utf8).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        XCTAssertEqual(a.lastPathComponent, encoded)
        XCTAssertEqual(a.deletingLastPathComponent(), b.deletingLastPathComponent())
        for _ in 0..<2 {
            XCTAssertEqual(try resolver?.delivery("assets/a.tex"), Data([1]))
            XCTAssertEqual(try resolver?.delivery("assets/a.tex/x.tex"), Data([2]))
        }
        resolver = nil
        XCTAssertFalse(FileManager.default.fileExists(atPath: a.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: b.path))
    }

    func testDeliveredTexturesAreConsumableButFontsStayCached() {
        var reads: [String: Int] = [:]
        let resolver = AssetResolver(root: URL(fileURLWithPath: "/unused"), names: ["assets/model.tex", "assets/font.ttf"]) { name in
            reads[name, default: 0] += 1
            return Data([1, 2, 3])
        }
        for _ in 0..<2 {
            XCTAssertEqual(resolver.bytes("assets/model.tex"), Data([1, 2, 3]))
            XCTAssertEqual(resolver.bytes("assets/font.ttf"), Data([1, 2, 3]))
        }
        XCTAssertEqual(reads["assets/model.tex"], 2)
        XCTAssertEqual(reads["assets/font.ttf"], 1)
    }
}
