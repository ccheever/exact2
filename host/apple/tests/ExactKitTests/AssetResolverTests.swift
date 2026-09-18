import XCTest
@testable import ExactKit

final class AssetResolverTests: XCTestCase {
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
