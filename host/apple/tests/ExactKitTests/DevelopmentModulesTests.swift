import Foundation
import XCTest
@testable import ExactKit

final class DevelopmentModulesTests: XCTestCase {
    func testEverySessionStartsPreparationEvenWhenEarlierSessionsWait() {
        var asked: [Int] = []
        let result = prepareGeneration([1, 2, 3]) { session -> String? in
            asked.append(session)
            return session == 2 ? "ready" : nil
        }
        XCTAssertNil(result)
        XCTAssertEqual(asked, [1, 2, 3])
        let ready = prepareGeneration([1, 2, 3]) { String($0) }
        XCTAssertEqual(ready?.map { $0.1 }, ["1", "2", "3"])
    }

    func testMixedReceiptsKeepTheirIdentityAcrossPendingRetries() throws {
        let javascript = ExactModule(receipt: Data(#"{"version":1,"module":{"hash":"js","bytes":1}}"#.utf8), bytecode: Data([1]))
        let rust = ExactModule(receipt: Data(#"{"version":1,"module":{"hash":"rust","bytes":1}}"#.utf8), bytecode: Data([2]))
        let combined = try XCTUnwrap(DevelopmentModules.combine(javascript: javascript, rust: rust))
        let expected = Data(#"{"javascript":{"module":{"bytes":1,"hash":"js"},"version":1},"javascriptBytes":1,"kind":"mixed","rust":{"module":{"bytes":1,"hash":"rust"},"version":1},"version":1}"#.utf8)
        XCTAssertEqual(combined.receipt, expected)
        for _ in 0..<20 {
            XCTAssertEqual(try DevelopmentModules.combine(javascript: javascript, rust: rust)?.receipt, combined.receipt)
        }
        XCTAssertEqual(combined.bytecode, Data([1, 2]))
    }
}
